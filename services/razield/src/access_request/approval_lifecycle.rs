use raziel_proto::raziel::v1::{
    AccessRequest, AccessRequestState as State, ApproveAccessRequestRequest,
};

use super::AccessCoordinator;
use super::approval::{PublicBinding, StoredApproval, normalize_command};
use super::model::{AccessError, identity};

impl AccessCoordinator {
    /// Approval stores a private decision. It does not provision or start a proof.
    pub fn approve(
        &mut self,
        command: ApproveAccessRequestRequest,
        now: i64,
    ) -> Result<AccessRequest, AccessError> {
        let command = normalize_command(command)?;
        let id = identity(&command.agent_id, &command.request_id)?;
        let _lock = self.private.lock()?;
        let mut connection = self.redis.connect()?;
        let (record, raw) = self.visible(&mut connection, &id, now)?;
        let mut record = self.expire(&mut connection, record, raw, now)?;
        match record.state()? {
            State::Expired => return Err(AccessError::Expired),
            State::Canceled | State::Rejected | State::Failed => return Err(AccessError::Conflict),
            _ => {}
        }
        let original = self.private.read(&record.private_ref)?;
        let approval = match self.private.read_approval(&record.private_ref, &original)? {
            Some(retained) => {
                if !retained.matches_retry(&command) {
                    return Err(AccessError::Conflict);
                }
                if record.policy_binding.is_some() {
                    // Exact repeats need neither the catalogue nor the dataset service.
                    return Ok(record.public());
                }
                retained
            }
            None => {
                if record.state()? != State::PendingReview
                    || record.generation != command.expected_generation
                {
                    return Err(AccessError::Conflict);
                }
                if command.dataset.is_none() {
                    return Err(AccessError::NotFound);
                }
                if command.dataset != original.0.dataset {
                    return Err(AccessError::Conflict);
                }
                let binding = self
                    .approval
                    .as_ref()
                    .ok_or(AccessError::ServiceUnavailable)?
                    .prepare(&command, &original.0, now)?;
                let approval = StoredApproval {
                    command,
                    binding,
                    approved_at: now,
                };
                // Retain the exact input before a potentially ambiguous Redis write.
                self.private
                    .persist_approval(&record.private_ref, &original, &approval)?;
                approval
            }
        };
        if record.state()? != State::PendingReview
            || record.generation != approval.command.expected_generation
            || approval.approved_at < record.updated
            || approval.approved_at >= record.deadline
        {
            return Err(AccessError::Conflict);
        }
        let raw = record.encode()?;
        record.policy_binding = Some(PublicBinding::from_proto(&approval.binding)?);
        record.state = State::ApprovedWaiting as i32;
        record.generation += 1;
        record.updated = approval.approved_at;
        record.due = record.deadline;
        self.redis.cas(&mut connection, &id, &raw, Some(&record))?;
        Ok(record.public())
    }
}
