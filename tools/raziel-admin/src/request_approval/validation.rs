use prost::Message;
use raziel_proto::raziel::v1 as proto;

use super::ApprovalError;

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// Performs bounded structural checks without touching owner storage.
pub fn validate_command(command: &proto::ApproveAccessRequestRequest) -> Result<(), ApprovalError> {
    let grant = command.grant.as_ref().ok_or(ApprovalError::Invalid)?;
    if command.encoded_len() > 4096
        || !hex_id(&command.agent_id)
        || !request_id(&command.request_id)
        || command.expected_generation == 0
        || !safe_id(&command.policy_record_id)
        || !hex_id(&command.policy_version_id)
        || command.approval_epoch == 0
        || command.dataset.as_ref().is_some_and(|dataset| {
            !safe_id(&dataset.dataset_id)
                || dataset.revision == 0
                || dataset.revision > MAX_SAFE_INTEGER
                || dataset.checksum.len() != 32
        })
        || grant.format_version != 1
        || !safe_id(&grant.team_id)
        || !safe_id(&grant.organization_id)
        || !(1..=MAX_SAFE_INTEGER as i64).contains(&grant.approved_amount)
    {
        return Err(ApprovalError::Invalid);
    }
    Ok(())
}

pub(super) fn match_original<'a>(
    command: &proto::ApproveAccessRequestRequest,
    original: &'a proto::CreateAccessRequestRequest,
) -> Result<&'a proto::AccessDatasetRevision, ApprovalError> {
    let owner = original.owner.as_ref().ok_or(ApprovalError::Invalid)?;
    let intent = original.intent.as_ref().ok_or(ApprovalError::Invalid)?;
    let dataset = original.dataset.as_ref().ok_or(ApprovalError::Invalid)?;
    let current_dataset = command.dataset.as_ref().ok_or(ApprovalError::Invalid)?;
    if intent.format_version != 1 || intent.action != "query" || owner.agent_public_key.len() != 32
    {
        return Err(ApprovalError::Invalid);
    }
    if command.agent_id != owner.agent_id
        || command.request_id != intent.request_id
        || current_dataset != dataset
        || dataset.dataset_id != intent.dataset_id
        || dataset.revision != intent.dataset_revision
    {
        return Err(ApprovalError::Conflict);
    }
    Ok(dataset)
}

fn hex_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn request_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, b)| match index {
            8 | 13 | 18 | 23 => b == b'-',
            14 => b == b'4',
            19 => b"89ab".contains(&b),
            _ => b.is_ascii_digit() || (b'a'..=b'f').contains(&b),
        })
}

fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.starts_with(|c: char| c.is_ascii_alphanumeric())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
}
