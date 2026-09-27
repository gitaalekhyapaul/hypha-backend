//! Request-triggered snapshot preparation. Queue dispatch and agent signing are
//! separate operations. This module never runs SP1 or releases protected rows.

use raziel_admin::request_provisioning::{ProvisioningConfig, ProvisioningError, RequestRevision};
use raziel_authority::{AuthorityRole, AuthoritySigner, SnapshotRole};
use raziel_proto::raziel::v1 as proto;
use raziel_verifier::{LocalRegistry, RegistryLoader, default_registry_config};

use crate::access_request::{AccessCoordinator, AccessError, ApprovedPreparation};
use crate::service::{SnapshotCoordinator, SnapshotServiceConfig, SystemClock, TrustedClock};
use crate::spicedb::RequestSnapshotSource;

const ISSUANCE_HORIZON_SECONDS: i64 = 4800;

/// Safe scope and signed snapshot for the later issuance exchange. It contains no
/// intent context, private grant, PolicyIR, relationship, salt, or Merkle opening.
pub struct PreparedSnapshotProposal {
    pub request_generation: u64,
    pub proposal_generation: u64,
    pub agent_public_key: Vec<u8>,
    pub task_id: String,
    pub purpose: String,
    pub action: String,
    pub policy_binding: proto::AccessPolicyBinding,
    pub snapshot: proto::RelationshipSnapshotDescriptor,
}

pub struct RequestPreparation {
    provisioning: ProvisioningConfig,
    snapshot_config: SnapshotServiceConfig,
    snapshots: SnapshotCoordinator,
}

impl RequestPreparation {
    pub fn open(
        provisioning: ProvisioningConfig,
        snapshot_config: SnapshotServiceConfig,
    ) -> Result<Self, AccessError> {
        if provisioning.registry_root != snapshot_config.registry_path()
            || provisioning.trust_domain_id != snapshot_config.trust_domain_id()
            || provisioning.endpoint != snapshot_config.spicedb_endpoint()
            || provisioning.token_path != snapshot_config.spicedb_auth_token_path()
            || snapshot_config.request_snapshot_root().is_none()
        {
            return Err(AccessError::Configuration);
        }
        let snapshots = SnapshotCoordinator::open(snapshot_config.clone())
            .map_err(|_| AccessError::ServiceUnavailable)?;
        Ok(Self {
            provisioning,
            snapshot_config,
            snapshots,
        })
    }

    /// Invoke only after the caller has reserved proof capacity. B-0066 owns that
    /// durable queue reservation and publication of AWAITING_SIGNATURE. Approval
    /// alone never invokes this function. Public inbox state is unchanged here.
    pub async fn prepare(
        &self,
        owner: &mut AccessCoordinator,
        agent: &str,
        request: &str,
        request_generation: u64,
        proposal_generation: u64,
    ) -> Result<PreparedSnapshotProposal, AccessError> {
        if proposal_generation == 0 {
            return Err(AccessError::InvalidRequest);
        }
        let input = owner.approved_preparation(agent, request, request_generation, now()?)?;
        let config = self.provisioning.clone();
        let command = input.approval.clone();
        let original = input.original.clone();
        let revision = tokio::task::spawn_blocking(move || {
            config
                .prepare(&command, &original, proposal_generation, now()?)
                .map_err(|error| match error {
                    ProvisioningError::Invalid => AccessError::InvalidRequest,
                    ProvisioningError::Conflict => AccessError::Conflict,
                    ProvisioningError::Unavailable => AccessError::ServiceUnavailable,
                })
        })
        .await
        .map_err(|_| AccessError::ServiceUnavailable)??;
        owner.confirm_preparation(&input, now()?)?;
        let source = snapshot_source(&input, &revision)?;
        let config = self.snapshot_config.clone();
        let binding = input.binding.clone();
        let deadline = input.deadline;
        let request_ref = format!("{}:{}:{}", agent, request, proposal_generation);
        let build = tokio::task::spawn_blocking(move || {
            build_request(&config, &binding, deadline, request_ref, now()?)
        })
        .await
        .map_err(|_| AccessError::ServiceUnavailable)??;
        let snapshot = self
            .snapshots
            .build_from_request(build, source)
            .await
            .map_err(|_| AccessError::ServiceUnavailable)?
            .snapshot
            .ok_or(AccessError::ServiceUnavailable)?;
        owner.confirm_preparation(&input, now()?)?;
        let owner_key = input
            .original
            .owner
            .as_ref()
            .ok_or(AccessError::ServiceUnavailable)?;
        let intent = input
            .original
            .intent
            .as_ref()
            .ok_or(AccessError::ServiceUnavailable)?;
        let minimum_expiry = now()?
            .checked_add(ISSUANCE_HORIZON_SECONDS)
            .ok_or(AccessError::Expired)?;
        if snapshot.policy_hash != input.binding.policy_hash
            || snapshot.policy_record_id != input.binding.policy_record_id
            || snapshot
                .expires_at
                .as_ref()
                .is_none_or(|expiry| expiry.nanos != 0 || expiry.seconds < minimum_expiry)
        {
            return Err(AccessError::Expired);
        }
        Ok(PreparedSnapshotProposal {
            request_generation: input.request_generation,
            proposal_generation,
            agent_public_key: owner_key.agent_public_key.clone(),
            task_id: intent.task_id.clone(),
            purpose: intent.purpose.clone(),
            action: intent.action.clone(),
            policy_binding: input.binding,
            snapshot,
        })
    }
}

fn snapshot_source(
    input: &ApprovedPreparation,
    revision: &RequestRevision,
) -> Result<RequestSnapshotSource, AccessError> {
    if revision.policy_record_id() != input.binding.policy_record_id
        || revision.policy_hash() != input.binding.policy_hash
    {
        return Err(AccessError::Conflict);
    }
    Ok(RequestSnapshotSource {
        receipt_key: format!("{}:{}", input.approval.agent_id, input.approval.request_id),
        proposal_generation: revision.proposal_generation(),
        policy_record_id: revision.policy_record_id().into(),
        zed_token: revision.zed_token().into(),
        source_schema_digest: revision
            .source_schema_digest()
            .try_into()
            .map_err(|_| AccessError::ServiceUnavailable)?,
        accepted_composition_schema: revision.accepted_composition_schema().into(),
    })
}

fn build_request(
    config: &SnapshotServiceConfig,
    binding: &proto::AccessPolicyBinding,
    deadline: i64,
    reference: String,
    now: i64,
) -> Result<proto::BuildSnapshotRequest, AccessError> {
    let registry_config = default_registry_config(config.trust_domain_id())
        .map_err(|_| AccessError::Configuration)?;
    let current = RegistryLoader::new(registry_config.clone())
        .load_current(config.registry_path(), now)
        .map_err(|_| AccessError::ServiceUnavailable)?;
    let mut registry = LocalRegistry::new(registry_config);
    registry
        .install_generation(current.clone())
        .map_err(|_| AccessError::ServiceUnavailable)?;
    let hash = binding
        .policy_hash
        .as_slice()
        .try_into()
        .map_err(|_| AccessError::ServiceUnavailable)?;
    let policy = registry
        .resolve_policy_exact(&binding.policy_record_id, &hash, now)
        .map_err(|_| AccessError::Conflict)?;
    let signer =
        AuthoritySigner::<SnapshotRole>::from_read_only_pkcs8_file(config.snapshot_key_path())
            .map_err(|_| AccessError::ServiceUnavailable)?;
    let authority = current
        .authority_keys
        .iter()
        .find(|key| key.key_id() == signer.key_id() && key.role() == AuthorityRole::Snapshot)
        .ok_or(AccessError::ServiceUnavailable)?;
    let expiry = deadline
        .min(policy.claims.claims().valid_until_unix_seconds)
        .min(authority.record().valid_until_unix_seconds);
    if expiry
        < now
            .checked_add(ISSUANCE_HORIZON_SECONDS)
            .ok_or(AccessError::Expired)?
    {
        return Err(AccessError::Expired);
    }
    let previous_epoch = current
        .accepted_roots
        .iter()
        .filter(|root| root.claims.claims().policy_hash == hash)
        .map(|root| root.claims.claims().epoch)
        .max()
        .unwrap_or(0);
    Ok(proto::BuildSnapshotRequest {
        spicedb_endpoint_ref: config.spicedb_endpoint_ref().into(),
        zed_token_secret_ref: reference,
        policy_record_id: binding.policy_record_id.clone(),
        policy_hash: binding.policy_hash.clone(),
        expires_at: Some(prost_types::Timestamp {
            seconds: expiry,
            nanos: 0,
        }),
        expected_previous_epoch: previous_epoch,
        authority_key_id: signer.key_id().into(),
    })
}

fn now() -> Result<i64, AccessError> {
    SystemClock
        .now_unix_seconds()
        .map_err(|_| AccessError::ServiceUnavailable)
}
