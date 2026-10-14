use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;

use prost::Message;

use super::wire::{
    PolicyActivationFailureWire, PolicyActivationRequestWire, PolicyActivationResponseWire,
    PolicyCatalogueEntryWire, PolicyCatalogueWire, PolicyOperationWire,
};
use super::{
    MAX_FRAME_BYTES, PROTOCOL_VERSION, PolicyActivationClientError, PolicyActivationProjection,
    PublicTimestamp,
};
use crate::policy_package::PolicyVersionIdentity;
use crate::policy_publication::catalogue::{
    CatalogueIdentity, CataloguePolicy, CatalogueProjection,
};
use crate::request_approval::GrantInputField;

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;

pub(super) fn send_request(
    socket_path: &Path,
    request: PolicyActivationRequestWire,
    timeout: std::time::Duration,
) -> Result<PolicyActivationProjection, PolicyActivationClientError> {
    response_result(send_operation(socket_path, request, timeout)?)
}

pub(super) fn send_catalogue_request(
    socket_path: &Path,
    timeout: std::time::Duration,
) -> Result<CatalogueProjection, PolicyActivationClientError> {
    let response = send_operation(
        socket_path,
        PolicyActivationRequestWire {
            format_version: PROTOCOL_VERSION,
            candidate_id: String::new(),
            approved_by: String::new(),
            operation: PolicyOperationWire::ListCatalogue as i32,
        },
        timeout,
    )?;
    catalogue_response_result(response)
}

fn catalogue_response_result(
    response: PolicyActivationResponseWire,
) -> Result<CatalogueProjection, PolicyActivationClientError> {
    response_failure(&response)?;
    if response.projection.is_some() {
        return Err(PolicyActivationClientError::Protocol);
    }
    catalogue_projection(
        response
            .catalogue
            .ok_or(PolicyActivationClientError::Protocol)?,
    )
}

fn send_operation(
    socket_path: &Path,
    request: PolicyActivationRequestWire,
    timeout: std::time::Duration,
) -> Result<PolicyActivationResponseWire, PolicyActivationClientError> {
    let mut stream =
        UnixStream::connect(socket_path).map_err(|_| PolicyActivationClientError::Unavailable)?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|_| PolicyActivationClientError::Unavailable)?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|_| PolicyActivationClientError::Unavailable)?;
    write_message(&mut stream, &request).map_err(|_| PolicyActivationClientError::Protocol)?;
    stream
        .shutdown(Shutdown::Write)
        .map_err(|_| PolicyActivationClientError::Protocol)?;
    let response: PolicyActivationResponseWire =
        read_message(&mut stream).map_err(|_| PolicyActivationClientError::Protocol)?;
    require_eof(&mut stream).map_err(|_| PolicyActivationClientError::Protocol)?;
    Ok(response)
}

pub(super) fn read_request(
    stream: &mut UnixStream,
) -> Result<PolicyActivationRequestWire, PolicyActivationClientError> {
    let request = read_message(stream).map_err(|_| PolicyActivationClientError::Protocol)?;
    require_eof(stream).map_err(|_| PolicyActivationClientError::Protocol)?;
    Ok(request)
}

pub(super) fn write_response(
    stream: &mut UnixStream,
    response: &PolicyActivationResponseWire,
) -> Result<(), PolicyActivationClientError> {
    write_message(stream, response).map_err(|_| PolicyActivationClientError::Protocol)
}

fn write_message<M: Message>(stream: &mut UnixStream, message: &M) -> Result<(), ()> {
    let bytes = message.encode_to_vec();
    if bytes.is_empty() || bytes.len() > MAX_FRAME_BYTES {
        return Err(());
    }
    let length = u32::try_from(bytes.len()).map_err(|_| ())?;
    stream.write_all(&length.to_be_bytes()).map_err(|_| ())?;
    stream.write_all(&bytes).map_err(|_| ())?;
    stream.flush().map_err(|_| ())
}

fn read_message<M: Message + Default>(stream: &mut UnixStream) -> Result<M, ()> {
    let mut prefix = [0_u8; 4];
    stream.read_exact(&mut prefix).map_err(|_| ())?;
    let length = usize::try_from(u32::from_be_bytes(prefix)).map_err(|_| ())?;
    if length == 0 || length > MAX_FRAME_BYTES {
        return Err(());
    }
    let mut bytes = vec![0_u8; length];
    stream.read_exact(&mut bytes).map_err(|_| ())?;
    let value = M::decode(bytes.as_slice()).map_err(|_| ())?;
    if value.encode_to_vec() != bytes {
        return Err(());
    }
    Ok(value)
}

fn require_eof(stream: &mut UnixStream) -> Result<(), ()> {
    let mut trailing = [0_u8; 1];
    match stream.read(&mut trailing) {
        Ok(0) => Ok(()),
        _ => Err(()),
    }
}

fn response_result(
    response: PolicyActivationResponseWire,
) -> Result<PolicyActivationProjection, PolicyActivationClientError> {
    response_failure(&response)?;
    if response.catalogue.is_some() {
        return Err(PolicyActivationClientError::Protocol);
    }
    let value = response
        .projection
        .ok_or(PolicyActivationClientError::Protocol)?;
    if value.record_id.is_empty()
        || value.policy_name.is_empty()
        || value.policy_version.is_empty()
        || value.policy_hash.len() != 64
        || value.approval_epoch == 0
        || value.registered_at_unix_seconds <= 0
        || value.valid_from_unix_seconds <= 0
        || value.valid_until_unix_seconds <= value.valid_from_unix_seconds
        || value.registry_generation == 0
    {
        return Err(PolicyActivationClientError::Protocol);
    }
    Ok(PolicyActivationProjection {
        record_id: value.record_id,
        policy_name: value.policy_name,
        policy_version: value.policy_version,
        policy_hash: value.policy_hash,
        approval_epoch: value.approval_epoch.to_string(),
        status: "APPROVED".into(),
        registered_at: timestamp(value.registered_at_unix_seconds),
        valid_from: timestamp(value.valid_from_unix_seconds),
        valid_until: timestamp(value.valid_until_unix_seconds),
        registry_generation: value.registry_generation.to_string(),
    })
}

fn response_failure(
    response: &PolicyActivationResponseWire,
) -> Result<(), PolicyActivationClientError> {
    if response.format_version != PROTOCOL_VERSION
        || response.failure != PolicyActivationFailureWire::Unspecified as i32
            && (response.projection.is_some() || response.catalogue.is_some())
    {
        return Err(PolicyActivationClientError::Protocol);
    }
    let failure = PolicyActivationFailureWire::try_from(response.failure)
        .map_err(|_| PolicyActivationClientError::Protocol)?;
    match failure {
        PolicyActivationFailureWire::Unspecified => Ok(()),
        PolicyActivationFailureWire::InvalidRequest => {
            Err(PolicyActivationClientError::InvalidRequest)
        }
        PolicyActivationFailureWire::Conflict => Err(PolicyActivationClientError::Conflict),
        PolicyActivationFailureWire::CandidateRejected => {
            Err(PolicyActivationClientError::CandidateRejected)
        }
        PolicyActivationFailureWire::Unavailable => Err(PolicyActivationClientError::Unavailable),
    }
}

pub(super) fn catalogue_wire(
    projection: CatalogueProjection,
) -> Result<PolicyCatalogueWire, PolicyActivationClientError> {
    if projection.format_version != 1 {
        return Err(PolicyActivationClientError::Protocol);
    }
    let policies = projection
        .policies
        .into_iter()
        .map(|policy| {
            Ok(PolicyCatalogueEntryWire {
                family_id: policy.identity.family_id,
                version_id: policy.identity.version_id,
                version: policy.identity.version,
                record_id: policy.record_id,
                policy_hash: policy.policy_hash,
                policy_name: policy.policy_name,
                policy_version: policy.policy_version,
                resource_type: policy.resource_type,
                permission: policy.permission.into(),
                approval_epoch: policy
                    .approval_epoch
                    .parse()
                    .map_err(|_| PolicyActivationClientError::Protocol)?,
                valid_from_unix_seconds: policy
                    .valid_from_unix_seconds
                    .parse()
                    .map_err(|_| PolicyActivationClientError::Protocol)?,
                valid_until_unix_seconds: policy
                    .valid_until_unix_seconds
                    .parse()
                    .map_err(|_| PolicyActivationClientError::Protocol)?,
                required_grant_fields: policy
                    .required_grant_fields
                    .into_iter()
                    .map(|field| field as i32)
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>, PolicyActivationClientError>>()?;
    let value = PolicyCatalogueWire {
        format_version: 2,
        policies,
    };
    catalogue_projection(value.clone())?;
    Ok(value)
}

fn catalogue_projection(
    value: PolicyCatalogueWire,
) -> Result<CatalogueProjection, PolicyActivationClientError> {
    use PolicyActivationClientError::Protocol;

    // Version 1 omits requirements and cannot distinguish absence from no inputs.
    if value.format_version != 2
        || value.policies.len() > 32
        || value
            .policies
            .windows(2)
            .any(|pair| pair[0].version_id >= pair[1].version_id)
    {
        return Err(Protocol);
    }
    let mut names = std::collections::BTreeSet::new();
    let mut records = std::collections::BTreeSet::new();
    let mut policies = Vec::with_capacity(value.policies.len());
    for policy in value.policies {
        if policy.required_grant_fields.len() > 6
            || policy
                .required_grant_fields
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != policy.required_grant_fields.len()
        {
            return Err(Protocol);
        }
        let required_grant_fields = policy
            .required_grant_fields
            .into_iter()
            .map(|field| GrantInputField::try_from(field).map_err(|_| Protocol))
            .collect::<Result<Vec<_>, _>>()?;
        let identity =
            PolicyVersionIdentity::new(policy.family_id, policy.version_id, policy.version)
                .map_err(|_| Protocol)?;
        if policy.policy_name != identity.signed_policy_name()
            || policy.policy_version != identity.version().to_string()
            || policy.resource_type != identity.resource_type()
            || policy.permission != "query"
            || policy.record_id.is_empty()
            || policy.record_id.len() > 512
            || !policy.record_id.bytes().all(|byte| byte.is_ascii_graphic())
            || policy.policy_hash.len() != 64
            || !policy
                .policy_hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || policy.approval_epoch == 0
            || policy.valid_from_unix_seconds < -62_135_596_800
            || policy.valid_until_unix_seconds > 253_402_300_799
            || policy.valid_until_unix_seconds <= policy.valid_from_unix_seconds
            || !names.insert(policy.policy_name.clone())
            || !records.insert(policy.record_id.clone())
        {
            return Err(Protocol);
        }
        policies.push(CataloguePolicy {
            identity: CatalogueIdentity {
                family_id: identity.family_id().into(),
                version_id: identity.version_id().into(),
                version: identity.version(),
            },
            record_id: policy.record_id,
            policy_hash: policy.policy_hash,
            policy_name: policy.policy_name,
            policy_version: policy.policy_version,
            resource_type: policy.resource_type,
            permission: "query",
            approval_epoch: policy.approval_epoch.to_string(),
            valid_from_unix_seconds: policy.valid_from_unix_seconds.to_string(),
            valid_until_unix_seconds: policy.valid_until_unix_seconds.to_string(),
            required_grant_fields,
        });
    }
    let projection = CatalogueProjection {
        format_version: 1,
        policies,
    };
    if serde_json::to_vec(&projection).map_err(|_| Protocol)?.len() > MAX_FRAME_BYTES {
        return Err(Protocol);
    }
    Ok(projection)
}

fn timestamp(seconds: i64) -> PublicTimestamp {
    PublicTimestamp {
        seconds: seconds.to_string(),
        nanos: 0,
    }
}
