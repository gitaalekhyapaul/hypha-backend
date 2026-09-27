use prost::Message;
use raziel_proto::raziel::v1::{
    AccessRequest, AccessRequestState as State, SubmitAccessIssuanceRequest,
};

use super::issuance_model::StoredAttempt;
use super::model::{AccessError, MAX_INTENT_BYTES, Record, identity};
use super::service::AccessCoordinator;

impl AccessCoordinator {
    pub fn submit_issuance(
        &mut self,
        input: SubmitAccessIssuanceRequest,
        now: i64,
    ) -> Result<AccessRequest, AccessError> {
        if input.encoded_len() > MAX_INTENT_BYTES {
            return Err(AccessError::InvalidRequest);
        }
        let owner = input.owner.as_ref().ok_or(AccessError::InvalidRequest)?;
        let id = identity(&owner.agent_id, &input.request_id)?;
        let submission = input
            .submission
            .as_ref()
            .ok_or(AccessError::InvalidRequest)?;
        if submission.format_version != 1
            || submission.generation == 0
            || submission.request.is_none()
        {
            return Err(AccessError::InvalidRequest);
        }
        let _lock = self.private.lock()?;
        let mut connection = self.redis.connect()?;
        let (record, raw) = self.visible(&mut connection, &id, now)?;
        let original = self.validated_intent(&record)?;
        if original.0.owner.as_ref() != Some(owner) {
            return Err(AccessError::Conflict);
        }
        let mut record = self.expire(&mut connection, record, raw, now)?;
        // Accepted input wins over an expired signing window or stale public generation.
        if let Some(attempt) = self
            .private
            .read_attempt(&record.private_ref, &record, &original)?
        {
            if attempt.submission(&record, &original)? != *submission {
                return Err(AccessError::Conflict);
            }
            return self.public_result(&mut connection, &record, true);
        }
        if record.state()? != State::AwaitingSignature {
            return Err(AccessError::Conflict);
        }
        let attempt = StoredAttempt::accept(submission, &record, &original, now)?;
        self.private
            .persist_attempt(&record.private_ref, &record, &original, &attempt)?;
        // The durable private write must survive an ambiguous Redis CAS or process exit.
        let raw = record.encode()?;
        accept_record(&mut record, &attempt, now)?;
        self.redis.cas(&mut connection, &id, &raw, Some(&record))?;
        Ok(record.public())
    }

    pub(super) fn recover_issuance(
        &self,
        connection: &mut redis::Connection,
        record: &mut Record,
        raw: &mut String,
        now: i64,
    ) -> Result<(), AccessError> {
        let original = self.validated_intent(record)?;
        let retained = self
            .private
            .read_attempt(&record.private_ref, record, &original)?;
        match (retained, record.attempt.as_ref()) {
            (None, None) => Ok(()),
            (Some(attempt), Some(public)) if attempt.matches_public(public) => Ok(()),
            (Some(attempt), None) if record.state()? == State::AwaitingSignature => {
                accept_record(record, &attempt, now)?;
                self.redis
                    .cas(connection, &record.identity(), raw, Some(record))?;
                *raw = record.encode()?;
                Ok(())
            }
            _ => Err(AccessError::ServiceUnavailable),
        }
    }
}

fn accept_record(
    record: &mut Record,
    attempt: &StoredAttempt,
    now: i64,
) -> Result<(), AccessError> {
    if now < record.updated || now < attempt.accepted_at {
        return Err(AccessError::ServiceUnavailable);
    }
    record.attempt = Some(attempt.public());
    record.state = State::Submitting as i32;
    record.generation = record
        .generation
        .checked_add(1)
        .ok_or(AccessError::ServiceUnavailable)?;
    record.updated = now;
    record.due = record.deadline;
    Ok(())
}
