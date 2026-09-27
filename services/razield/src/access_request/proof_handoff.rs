use prost::Message;
use raziel_proto::raziel::v1::{AccessRequest, ProofJob, PublicErrorCode};
#[cfg(feature = "access-dispatch")]
use raziel_proto::raziel::v1::{
    AccessRequestState as State, CreateCapabilityRequest, ProofJobState,
};

#[cfg(feature = "access-dispatch")]
use super::dispatch_model::DispatchReference;
use super::model::{AccessError, Record};
use super::proof_result;
use super::service::AccessCoordinator;

// Private input never implements Debug or enters a BullMQ payload.
#[cfg(feature = "access-dispatch")]
pub(crate) struct PendingProof {
    pub job_id: String,
    pub request: CreateCapabilityRequest,
    pub cancelling: bool,
    pub started: bool,
    pub observed: bool,
}

#[cfg(feature = "access-dispatch")]
pub(crate) struct SubmissionStart {
    pub sequence: u64,
    pub first: bool,
}

#[cfg(feature = "access-dispatch")]
impl AccessCoordinator {
    pub(crate) fn pending_proof(
        &mut self,
        reference: &DispatchReference,
        now: i64,
    ) -> Result<PendingProof, AccessError> {
        let _lock = self.private.lock()?;
        let mut connection = self.redis.connect()?;
        let record = self.handoff_record(&mut connection, reference, now)?;
        let original = self.validated_intent(&record)?;
        let stored = self
            .private
            .read_attempt(&record.private_ref, &record, &original)?
            .ok_or(AccessError::ServiceUnavailable)?;
        let public = record
            .attempt
            .as_ref()
            .ok_or(AccessError::ServiceUnavailable)?;
        Ok(PendingProof {
            job_id: public.job_id.clone(),
            request: stored
                .submission(&record, &original)?
                .request
                .ok_or(AccessError::ServiceUnavailable)?,
            cancelling: record.state()? == State::Cancelling,
            started: public.submission_started_at.is_some(),
            observed: public.proof_state != ProofJobState::Unspecified as i32,
        })
    }

    /// This marker precedes the RPC. Cancellation after this point must reconcile
    /// an outstanding publication, including after a process restart.
    pub(crate) fn begin_submission(
        &mut self,
        reference: &DispatchReference,
        now: i64,
    ) -> Result<SubmissionStart, AccessError> {
        let _lock = self.private.lock()?;
        let mut connection = self.redis.connect()?;
        let mut record = self.handoff_record(&mut connection, reference, now)?;
        if record.state()? != State::Submitting {
            return Err(AccessError::Conflict);
        }
        let raw = record.encode()?;
        let attempt = record
            .attempt
            .as_mut()
            .ok_or(AccessError::ServiceUnavailable)?;
        if attempt.proof_state != ProofJobState::Unspecified as i32 {
            return Err(AccessError::ServiceUnavailable);
        }
        let first = attempt.submission_started_at.is_none();
        attempt.submission_started_at.get_or_insert(now);
        attempt.submission_count = attempt
            .submission_count
            .checked_add(1)
            .ok_or(AccessError::ServiceUnavailable)?;
        let sequence = attempt.submission_count;
        advance(&mut record, now)?;
        self.redis
            .cas(&mut connection, &record.identity(), &raw, Some(&record))?;
        Ok(SubmissionStart { first, sequence })
    }

    /// Only a definite rejection of the first RPC can establish no publication.
    /// An older ambiguous RPC cannot be cleared by a later rejected retry.
    pub(crate) fn reject_first_submission(
        &mut self,
        reference: &DispatchReference,
        sequence: u64,
        now: i64,
    ) -> Result<(), AccessError> {
        let _lock = self.private.lock()?;
        let mut connection = self.redis.connect()?;
        let mut record = self.handoff_record(&mut connection, reference, now)?;
        let raw = record.encode()?;
        let attempt = record
            .attempt
            .as_mut()
            .ok_or(AccessError::ServiceUnavailable)?;
        if attempt.proof_state != 0
            || attempt.submission_started_at.is_none()
            || attempt.submission_count != sequence
        {
            return Err(AccessError::Conflict);
        }
        attempt.submission_started_at = None;
        advance(&mut record, now)?;
        self.redis
            .cas(&mut connection, &record.identity(), &raw, Some(&record))
    }

    pub(crate) fn finish_unsubmitted(
        &mut self,
        reference: &DispatchReference,
        now: i64,
    ) -> Result<(), AccessError> {
        let _lock = self.private.lock()?;
        let mut connection = self.redis.connect()?;
        let mut record = self.handoff_record(&mut connection, reference, now)?;
        let attempt = record
            .attempt
            .as_ref()
            .ok_or(AccessError::ServiceUnavailable)?;
        if attempt.submission_started_at.is_some() || attempt.proof_state != 0 {
            return Err(AccessError::Conflict);
        }
        let canceled = record.state()? == State::Cancelling;
        let job = ProofJob {
            job_id: attempt.job_id.clone(),
            state: if canceled {
                ProofJobState::Canceled
            } else {
                ProofJobState::Failed
            } as i32,
            submitted_at: Some(prost_types::Timestamp {
                seconds: attempt.accepted_at,
                nanos: 0,
            }),
            updated_at: Some(prost_types::Timestamp {
                seconds: now,
                nanos: 0,
            }),
            public_error_code: if canceled {
                PublicErrorCode::Unspecified
            } else {
                PublicErrorCode::InvalidRequest
            } as i32,
            ..Default::default()
        };
        self.apply_proof_result(&mut connection, &mut record, job, now)
    }

    pub(crate) fn record_proof_job(
        &mut self,
        reference: &DispatchReference,
        job: ProofJob,
        now: i64,
    ) -> Result<(), AccessError> {
        let _lock = self.private.lock()?;
        let mut connection = self.redis.connect()?;
        let mut record = self.handoff_record(&mut connection, reference, now)?;
        if record
            .attempt
            .as_ref()
            .is_none_or(|attempt| attempt.submission_started_at.is_none())
        {
            return Err(AccessError::ServiceUnavailable);
        }
        self.apply_proof_result(&mut connection, &mut record, job, now)
    }

    fn apply_proof_result(
        &self,
        connection: &mut redis::Connection,
        record: &mut Record,
        job: ProofJob,
        now: i64,
    ) -> Result<(), AccessError> {
        let target = proof_result::validate(record, &job)?;
        let terminal = matches!(target, State::Ready | State::Failed | State::Canceled);
        let old_state = record
            .attempt
            .as_ref()
            .ok_or(AccessError::ServiceUnavailable)?
            .proof_state;
        if !terminal && job.state < old_state {
            return Err(AccessError::ServiceUnavailable);
        }
        if !terminal && job.state == old_state {
            return Ok(());
        }
        let raw = record.encode()?;
        record.attempt.as_mut().unwrap().proof_state = job.state;
        if terminal {
            record.finish(target, now)?;
            // Public result and slot release commit together, independently of proof retention.
            self.redis
                .cas_terminal(connection, &raw, record, &job.encode_to_vec())
        } else {
            if record.state()? != State::Cancelling {
                record.state = target as i32;
            }
            advance(record, now)?;
            self.redis
                .cas(connection, &record.identity(), &raw, Some(record))
        }
    }

    fn handoff_record(
        &self,
        connection: &mut redis::Connection,
        reference: &DispatchReference,
        now: i64,
    ) -> Result<Record, AccessError> {
        reference.validate()?;
        let (record, raw) = self.visible(connection, &reference.identity(), now)?;
        let record = self.expire(connection, record, raw, now)?;
        if !reference.matches(&record) || record.terminal()? || record.attempt.is_none() {
            return Err(AccessError::Conflict);
        }
        Ok(record)
    }
}

impl AccessCoordinator {
    pub(super) fn public_result(
        &self,
        connection: &mut redis::Connection,
        record: &Record,
        include_capability: bool,
    ) -> Result<AccessRequest, AccessError> {
        let mut public = record.public();
        if record.terminal()? && record.attempt.is_some() {
            let bytes = self
                .redis
                .result(connection, &record.identity())?
                .ok_or(AccessError::ServiceUnavailable)?;
            let job =
                ProofJob::decode(bytes.as_slice()).map_err(|_| AccessError::ServiceUnavailable)?;
            if job.encode_to_vec() != bytes
                || proof_result::validate(record, &job)? != record.state()?
                || job.state != public.proof_job_state
            {
                return Err(AccessError::ServiceUnavailable);
            }
            public.metrics = job.metrics;
            public.error_code = match PublicErrorCode::try_from(job.public_error_code)
                .map_err(|_| AccessError::ServiceUnavailable)?
            {
                PublicErrorCode::Unspecified => None,
                PublicErrorCode::InvalidRequest => Some("INVALID_REQUEST".into()),
                PublicErrorCode::ServiceUnavailable => Some("SERVICE_UNAVAILABLE".into()),
                _ => return Err(AccessError::ServiceUnavailable),
            };
            if include_capability {
                public.capability = job
                    .capability
                    .map(|value| value.encode_to_vec())
                    .unwrap_or_default();
            }
        }
        Ok(public)
    }
}

#[cfg(feature = "access-dispatch")]
fn advance(record: &mut Record, now: i64) -> Result<(), AccessError> {
    if now < record.updated {
        return Err(AccessError::ServiceUnavailable);
    }
    record.generation = record
        .generation
        .checked_add(1)
        .ok_or(AccessError::ServiceUnavailable)?;
    record.updated = now;
    Ok(())
}
