use raziel_proto::raziel::v1::{
    AccessPolicyBinding, AccessRequestState, ApproveAccessRequestRequest,
    CreateAccessRequestRequest,
};

use super::model::identity;
use super::{AccessCoordinator, AccessError};

/// Owner-private input to preparation, never a public API response or queue payload.
/// The dispatcher must recheck the request generation before publishing its result.
pub struct ApprovedPreparation {
    pub(crate) original: CreateAccessRequestRequest,
    pub(crate) approval: ApproveAccessRequestRequest,
    pub(crate) binding: AccessPolicyBinding,
    pub(crate) request_generation: u64,
    pub(crate) deadline: i64,
}

impl AccessCoordinator {
    /// Reads one durable approval without creating a proof job or reserving capacity.
    pub fn approved_preparation(
        &mut self,
        agent: &str,
        request: &str,
        expected_generation: u64,
        now: i64,
    ) -> Result<ApprovedPreparation, AccessError> {
        let id = identity(agent, request)?;
        let _lock = self.private.lock()?;
        let mut connection = self.redis.connect()?;
        let (record, raw) = self.visible(&mut connection, &id, now)?;
        let record = self.expire(&mut connection, record, raw, now)?;
        if record.state()? == AccessRequestState::Expired {
            return Err(AccessError::Expired);
        }
        if !matches!(
            record.state()?,
            AccessRequestState::ApprovedWaiting | AccessRequestState::Preparing
        ) || record.generation != expected_generation
        {
            return Err(AccessError::Conflict);
        }
        let original = self.private.read(&record.private_ref)?;
        let approval = self
            .private
            .read_approval(&record.private_ref, &original)?
            .ok_or(AccessError::ServiceUnavailable)?;
        if !approval.matches_published(&record) {
            return Err(AccessError::ServiceUnavailable);
        }
        Ok(ApprovedPreparation {
            original: original.0,
            approval: approval.command,
            binding: approval.binding,
            request_generation: record.generation,
            deadline: record.deadline,
        })
    }

    /// A cancellation, expiry, or generation change during network work fences the
    /// result. A produced snapshot alone can never revive the request.
    pub fn confirm_preparation(
        &mut self,
        input: &ApprovedPreparation,
        now: i64,
    ) -> Result<(), AccessError> {
        let current = self.approved_preparation(
            &input.approval.agent_id,
            &input.approval.request_id,
            input.request_generation,
            now,
        )?;
        if current.original != input.original
            || current.approval != input.approval
            || current.binding != input.binding
            || current.deadline != input.deadline
        {
            return Err(AccessError::Conflict);
        }
        Ok(())
    }
}
