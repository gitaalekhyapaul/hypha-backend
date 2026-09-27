use raziel_proto::raziel::v1::{AccessRequest, AccessRequestState as State};

use super::AccessCoordinator;
use super::approval::PublicBinding;
use super::dispatch_model::{
    DispatchReference, ISSUANCE_SECONDS, PublicProposal, SIGNATURE_SECONDS, hex,
};
use super::model::{AccessError, Phase, Record, identity};
use crate::request_preparation::PreparedSnapshotProposal;

impl AccessCoordinator {
    pub fn dispatch_request(
        &mut self,
        reference: &DispatchReference,
        now: i64,
    ) -> Result<Option<AccessRequest>, AccessError> {
        reference.validate()?;
        let _lock = self.private.lock()?;
        let mut connection = self.redis.connect()?;
        let (record, raw) = self.visible(&mut connection, &reference.identity(), now)?;
        let record = self.expire(&mut connection, record, raw, now)?;
        if reference.matches(&record)
            || (record.state()? == State::ApprovedWaiting
                && record.generation == reference.generation)
        {
            Ok(Some(record.public()))
        } else {
            Ok(None)
        }
    }

    /// Repairs missing BullMQ references from durable requests. Active workflows
    /// precede waiting requests, including after a dispatcher process restart.
    pub fn dispatch_candidates(&mut self, now: i64) -> Result<Vec<DispatchReference>, AccessError> {
        let _lock = self.private.lock()?;
        let mut connection = self.redis.connect()?;
        let mut active = Vec::new();
        let mut waiting = Vec::new();
        for id in self.redis.active(&mut connection)? {
            let (record, raw) = self.visible(&mut connection, &id, now)?;
            let record = self.expire(&mut connection, record, raw, now)?;
            if record.state()? == State::ApprovedWaiting {
                waiting.push((
                    record.created,
                    DispatchReference {
                        agent_id: record.agent_id,
                        request_id: record.request_id,
                        generation: record.generation,
                    },
                ));
            } else if holds_capacity(record.state()?) {
                active.push(DispatchReference {
                    agent_id: record.agent_id,
                    request_id: record.request_id,
                    generation: record
                        .dispatch_generation
                        .ok_or(AccessError::ServiceUnavailable)?,
                });
            }
        }
        if active.len() > 1 {
            return Err(AccessError::ServiceUnavailable);
        }
        waiting.sort_by_key(|(created, reference)| (*created, reference.job_id()));
        active.extend(waiting.into_iter().map(|(_, reference)| reference));
        Ok(active)
    }

    /// Called only by the BullMQ processor after checking the proof queue is free.
    /// The durable reservation survives a lost worker lock and fences other jobs.
    pub fn reserve_preparation(
        &mut self,
        reference: &DispatchReference,
        now: i64,
    ) -> Result<AccessRequest, AccessError> {
        reference.validate()?;
        let _lock = self.private.lock()?;
        let mut connection = self.redis.connect()?;
        let (record, raw) = self.visible(&mut connection, &reference.identity(), now)?;
        let mut record = self.expire(&mut connection, record, raw, now)?;
        if reference.matches(&record) && holds_capacity(record.state()?) {
            return Ok(record.public());
        }
        if record.terminal()?
            || record.state()? != State::ApprovedWaiting
            || record.generation != reference.generation
        {
            return Err(AccessError::Conflict);
        }
        for id in self.redis.active(&mut connection)? {
            if id == record.identity() {
                continue;
            }
            let (other, _) = self
                .redis
                .get(&mut connection, &id)?
                .ok_or(AccessError::ServiceUnavailable)?;
            if holds_capacity(other.state()?) {
                return Err(AccessError::CapacityExceeded);
            }
        }
        let raw = record.encode()?;
        record.dispatch_generation = Some(reference.generation);
        transition(&mut record, State::Preparing, now)?;
        self.redis
            .cas(&mut connection, &reference.identity(), &raw, Some(&record))?;
        Ok(record.public())
    }

    pub fn publish_proposal(
        &mut self,
        reference: &DispatchReference,
        prepared: PreparedSnapshotProposal,
        now: i64,
    ) -> Result<AccessRequest, AccessError> {
        reference.validate()?;
        let _lock = self.private.lock()?;
        let mut connection = self.redis.connect()?;
        let (record, raw) = self.visible(&mut connection, &reference.identity(), now)?;
        let mut record = self.expire(&mut connection, record, raw, now)?;
        if !reference.matches(&record)
            || record.state()? != State::Preparing
            || prepared.request_generation != record.generation
            || prepared.proposal_generation != reference.generation
        {
            return Err(AccessError::Conflict);
        }
        let original = self.private.read(&record.private_ref)?;
        let binding = PublicBinding::from_proto(&prepared.policy_binding)?;
        let snapshot = prepared.snapshot;
        let requested_expiry = now
            .checked_add(ISSUANCE_SECONDS)
            .ok_or(AccessError::Expired)?;
        if original
            .0
            .owner
            .as_ref()
            .is_none_or(|owner| owner.agent_public_key != prepared.agent_public_key)
            || record.policy_binding.as_ref() != Some(&binding)
            || prepared.task_id != record.task_id
            || prepared.purpose != record.purpose
            || prepared.action != record.action
            || snapshot.policy_hash != binding.policy_hash
            || snapshot.policy_record_id != binding.policy_record_id
            || snapshot
                .expires_at
                .as_ref()
                .is_none_or(|time| time.nanos != 0 || time.seconds < requested_expiry)
            || requested_expiry > record.deadline
        {
            return Err(AccessError::Conflict);
        }
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|_| AccessError::ServiceUnavailable)?;
        let raw = record.encode()?;
        transition(&mut record, State::AwaitingSignature, now)?;
        record.proposal = Some(PublicProposal {
            proposal_id: hex(&random),
            generation: record.generation,
            created: now,
            signature_deadline: now + SIGNATURE_SECONDS,
            agent_public_key: prepared.agent_public_key,
            snapshot_id: snapshot.snapshot_id,
            snapshot_root: snapshot.relationship_snapshot_root,
            requested_expiry,
        });
        record.proposal.as_ref().unwrap().validate(&record)?;
        record.due = now + SIGNATURE_SECONDS;
        self.redis
            .cas(&mut connection, &reference.identity(), &raw, Some(&record))?;
        Ok(record.public())
    }

    /// Explicit authenticated resume, never an automatic retry loop for offline agents.
    pub fn resume(
        &mut self,
        agent: &str,
        request: &str,
        generation: u64,
        now: i64,
    ) -> Result<AccessRequest, AccessError> {
        let id = identity(agent, request)?;
        if generation == 0 {
            return Err(AccessError::InvalidRequest);
        }
        let _lock = self.private.lock()?;
        let mut connection = self.redis.connect()?;
        let (record, raw) = self.visible(&mut connection, &id, now)?;
        let mut record = self.expire(&mut connection, record, raw, now)?;
        if record.state()? == State::Expired {
            return Err(AccessError::Expired);
        }
        if record.state()? == State::ApprovedWaiting
            && record.resumed_from_generation == Some(generation)
        {
            return Ok(record.public());
        }
        if record.state()? != State::WaitingForAgent || record.generation != generation {
            return Err(AccessError::Conflict);
        }
        let raw = record.encode()?;
        transition(&mut record, State::ApprovedWaiting, now)?;
        record.dispatch_generation = None;
        record.proposal = None;
        record.resumed_from_generation = Some(generation);
        self.redis.cas(&mut connection, &id, &raw, Some(&record))?;
        Ok(record.public())
    }

    /// Preparation has no proof attempt. Confirm cancellation only after its callback exits.
    pub fn finish_preparation(
        &mut self,
        reference: &DispatchReference,
        failed: bool,
        now: i64,
    ) -> Result<AccessRequest, AccessError> {
        reference.validate()?;
        let _lock = self.private.lock()?;
        let mut connection = self.redis.connect()?;
        let (record, raw) = self.visible(&mut connection, &reference.identity(), now)?;
        let mut record = self.expire(&mut connection, record, raw, now)?;
        if !reference.matches(&record) {
            return Err(AccessError::Conflict);
        }
        if record.terminal()? {
            return Ok(record.public());
        }
        let target = match record.state()? {
            State::Cancelling if record.proposal.is_none() => State::Canceled,
            State::Preparing if failed => State::Failed,
            _ => return Err(AccessError::Conflict),
        };
        let raw = record.encode()?;
        record.finish(target, now)?;
        self.redis
            .cas(&mut connection, &reference.identity(), &raw, Some(&record))?;
        Ok(record.public())
    }
}

pub(super) fn holds_capacity(state: State) -> bool {
    matches!(
        state,
        State::Preparing
            | State::AwaitingSignature
            | State::Submitting
            | State::Queued
            | State::Executing
            | State::Proving
            | State::Cancelling
    )
}

fn transition(record: &mut Record, next: State, now: i64) -> Result<(), AccessError> {
    if record.phase != Phase::Published || record.terminal()? || now < record.updated {
        return Err(AccessError::Conflict);
    }
    record.generation = record
        .generation
        .checked_add(1)
        .ok_or(AccessError::ServiceUnavailable)?;
    record.updated = now;
    record.state = next as i32;
    record.due = record.deadline;
    Ok(())
}
