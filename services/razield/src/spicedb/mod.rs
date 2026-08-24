//! Bounded runtime reads from one exact SpiceDB revision.

mod client;
mod conversion;

#[cfg(test)]
mod tests;

use std::error::Error;
use std::fmt;

use raziel_policy_ir::ValidatedPolicyProgram;
use raziel_snapshot::RelationshipRecord;
use spicedb_rs_proto::authzed::api::v1 as authzed;

pub use client::SpiceDbSnapshotReader;

const AGGREGATE_READ_LIMIT: u32 = 257;
const MAX_RELATIONSHIPS: usize = 256;
const MAX_SPICEDB_MESSAGE_BYTES: usize = 2 * 1024 * 1024;

#[tonic::async_trait]
trait SpiceDbReadApi: Send {
    async fn reflect_schema(
        &mut self,
        request: authzed::ReflectSchemaRequest,
    ) -> Result<authzed::ReflectSchemaResponse, SpiceDbSnapshotError>;

    async fn read_relationships(
        &mut self,
        request: authzed::ReadRelationshipsRequest,
    ) -> Result<Vec<authzed::ReadRelationshipsResponse>, SpiceDbSnapshotError>;
}

async fn read_exact<A>(
    api: &mut A,
    policy: &ValidatedPolicyProgram,
    zed_token: &str,
) -> Result<Vec<RelationshipRecord>, SpiceDbSnapshotError>
where
    A: SpiceDbReadApi,
{
    let policy = ValidatedPolicyProgram::new(
        policy.program().clone(),
        &raziel_authorization_core::TRUSTED_POLICY_CEILINGS,
    )
    .map_err(|_| SpiceDbSnapshotError::PolicyOutsideGuestLimits)?;
    validate_zed_token(zed_token)?;

    let reflected = api
        .reflect_schema(authzed::ReflectSchemaRequest {
            consistency: Some(exact_consistency(zed_token)),
            optional_filters: Vec::new(),
        })
        .await?;
    require_exact_token(reflected.read_at.as_ref(), zed_token)?;

    let mut relationships = Vec::new();
    for definition in &policy.program().definitions {
        let remaining = AGGREGATE_READ_LIMIT
            .checked_sub(
                u32::try_from(relationships.len())
                    .map_err(|_| SpiceDbSnapshotError::RelationshipLimitExceeded)?,
            )
            .ok_or(SpiceDbSnapshotError::RelationshipLimitExceeded)?;
        let responses = api
            .read_relationships(authzed::ReadRelationshipsRequest {
                consistency: Some(exact_consistency(zed_token)),
                relationship_filter: Some(authzed::RelationshipFilter {
                    resource_type: definition.name.clone(),
                    optional_resource_id: String::new(),
                    optional_relation: String::new(),
                    optional_subject_filter: None,
                    optional_resource_id_prefix: String::new(),
                }),
                optional_limit: remaining,
                optional_cursor: None,
            })
            .await?;
        for response in responses {
            require_exact_token(response.read_at.as_ref(), zed_token)?;
            let relationship = response
                .relationship
                .as_ref()
                .ok_or(SpiceDbSnapshotError::MalformedRelationship)?;
            relationships.push(conversion::relationship_from_api(relationship)?);
            if relationships.len() > MAX_RELATIONSHIPS {
                return Err(SpiceDbSnapshotError::RelationshipLimitExceeded);
            }
        }
    }

    relationships.sort_by(raziel_snapshot::compare_relationship_identity);
    if relationships
        .windows(2)
        .any(|pair| raziel_snapshot::compare_relationship_identity(&pair[0], &pair[1]).is_eq())
    {
        return Err(SpiceDbSnapshotError::DuplicateRelationship);
    }
    Ok(relationships)
}

fn exact_consistency(zed_token: &str) -> authzed::Consistency {
    authzed::Consistency {
        requirement: Some(authzed::consistency::Requirement::AtExactSnapshot(
            authzed::ZedToken {
                token: zed_token.to_owned(),
            },
        )),
    }
}

fn require_exact_token(
    token: Option<&authzed::ZedToken>,
    expected: &str,
) -> Result<(), SpiceDbSnapshotError> {
    match token {
        Some(token) if token.token == expected => Ok(()),
        _ => Err(SpiceDbSnapshotError::RevisionMismatch),
    }
}

fn validate_zed_token(value: &str) -> Result<(), SpiceDbSnapshotError> {
    if value.is_empty()
        || value.len() > 4_096
        || !value.is_ascii()
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(SpiceDbSnapshotError::InvalidRevision);
    }
    Ok(())
}

/// Redacted failures from exact-revision snapshot reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpiceDbSnapshotError {
    InvalidConfiguration,
    ConnectionFailed,
    AuthenticationFailed,
    PolicyOutsideGuestLimits,
    InvalidRevision,
    ReflectionFailed,
    ReadFailed,
    RevisionMismatch,
    RelationshipLimitExceeded,
    MalformedRelationship,
    UnsupportedWildcard,
    UnsupportedExpiration,
    UnsupportedCaveatValue,
    DuplicateRelationship,
}

impl fmt::Display for SpiceDbSnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidConfiguration => "invalid SpiceDB snapshot configuration",
            Self::ConnectionFailed => "SpiceDB connection failed",
            Self::AuthenticationFailed => "SpiceDB authentication configuration failed",
            Self::PolicyOutsideGuestLimits => "policy exceeds proof guest limits",
            Self::InvalidRevision => "invalid SpiceDB revision",
            Self::ReflectionFailed => "SpiceDB schema reflection failed",
            Self::ReadFailed => "SpiceDB relationship read failed",
            Self::RevisionMismatch => "SpiceDB revision evidence mismatch",
            Self::RelationshipLimitExceeded => "SpiceDB relationship limit exceeded",
            Self::MalformedRelationship => "SpiceDB returned a malformed relationship",
            Self::UnsupportedWildcard => "SpiceDB returned an unsupported wildcard subject",
            Self::UnsupportedExpiration => "SpiceDB returned an unsupported expiring relationship",
            Self::UnsupportedCaveatValue => "SpiceDB returned an unsupported stored caveat value",
            Self::DuplicateRelationship => "SpiceDB returned a duplicate relationship",
        })
    }
}

impl Error for SpiceDbSnapshotError {}
