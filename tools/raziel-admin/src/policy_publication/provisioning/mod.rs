//! Single-owner relationship provisioning and private exact-revision handoff.

#[cfg(test)]
mod live;
pub(super) mod network;
mod storage;
#[cfg(test)]
mod tests;

use std::fmt;
use std::fs::File;
use std::path::PathBuf;
use std::time::Duration;

use prost::Message;
use raziel_proto::raziel::v1 as proto;
use spicedb_rs_proto::authzed::api::v1 as api;

use crate::policy_activation::store::StoreActivationError;
use crate::policy_package::{PolicyPackageKey, PolicyPackageStore};
use crate::request_approval::{ApprovalConfig, ApprovalError, build_grant};

use super::{compiler::Compiler, files, installation::Installation};
use network::{GrantApi, GrantClient};
use storage::{RequestStore, StoredRequest};

#[derive(Clone)]
pub struct ProvisioningConfig {
    pub publication_root: PathBuf,
    pub package_root: PathBuf,
    pub registry_root: PathBuf,
    pub binding_root: PathBuf,
    pub compiler: PathBuf,
    pub token_path: PathBuf,
    pub endpoint: String,
    pub trust_domain_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProvisioningError {
    Invalid,
    Conflict,
    Unavailable,
}

impl fmt::Display for ProvisioningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Invalid => "invalid request provisioning input",
            Self::Conflict => "request provisioning conflicts with current owner state",
            Self::Unavailable => "request provisioning is unavailable",
        })
    }
}
impl std::error::Error for ProvisioningError {}

/// Private handoff. Do not expose this object through agent or browser responses.
#[derive(Clone)]
pub struct RequestRevision {
    stored: StoredRequest,
}

impl RequestRevision {
    pub fn policy_record_id(&self) -> &str {
        &self.stored.policy_record_id
    }
    pub fn policy_hash(&self) -> &[u8] {
        &self.stored.policy_hash
    }
    pub fn source_schema_digest(&self) -> &[u8] {
        &self.stored.source_schema_digest
    }
    pub fn zed_token(&self) -> &str {
        self.stored.zed_token.as_deref().unwrap_or_default()
    }
    pub fn accepted_composition_schema(&self) -> &str {
        &self.stored.accepted_schema
    }
    /// JSON reflection emitted by the pinned composition compiler.
    pub fn expected_reflection_json(&self) -> &[u8] {
        &self.stored.accepted_reflection
    }
    pub fn accepted_relationship_count(&self) -> u32 {
        self.stored.relationships.len() as u32
    }
    pub fn proposal_generation(&self) -> u64 {
        self.stored.proposal_generation
    }
}

impl fmt::Debug for RequestRevision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestRevision")
            .field("private_fields", &"<redacted>")
            .finish()
    }
}

impl ProvisioningConfig {
    /// Blocking owner operation. Run from spawn_blocking, never a Tokio executor thread.
    /// The publication lock ends before any snapshot construction or proof execution.
    pub fn prepare(
        &self,
        command: &proto::ApproveAccessRequestRequest,
        original: &proto::CreateAccessRequestRequest,
        proposal_generation: u64,
        now: i64,
    ) -> Result<RequestRevision, ProvisioningError> {
        if proposal_generation == 0 || original.encoded_len() > 16 * 1024 {
            return Err(ProvisioningError::Invalid);
        }
        ApprovalConfig {
            publication_root: self.publication_root.clone(),
            package_root: self.package_root.clone(),
            registry_root: self.registry_root.clone(),
            binding_root: self.binding_root.clone(),
            trust_domain_id: self.trust_domain_id.clone(),
        }
        .prepare(command, original, now)
        .map_err(approval_error)?;
        files::private_directory(&self.publication_root).map_err(owner_error)?;
        let lock =
            File::open(&self.publication_root).map_err(|_| ProvisioningError::Unavailable)?;
        lock.try_lock()
            .map_err(|_| ProvisioningError::Unavailable)?;
        let compiler =
            Compiler::new(&self.compiler, &self.publication_root).map_err(owner_error)?;
        let mut packages = PolicyPackageStore::new(
            self.package_root.clone(),
            self.registry_root.clone(),
            &self.trust_domain_id,
        )
        .map_err(|_| ProvisioningError::Unavailable)?;
        let installed = Installation::load_for_request(
            &self.publication_root,
            &self.registry_root,
            &self.trust_domain_id,
            &mut packages,
            &compiler,
            now,
        )
        .map_err(owner_error)?;
        let key = PolicyPackageKey::new(command.policy_version_id.clone(), command.approval_epoch)
            .map_err(|_| ProvisioningError::Invalid)?;
        if !installed.contains(&key) {
            return Err(ProvisioningError::Conflict);
        }
        let package = packages
            .read_approved(&key, now)
            .map_err(|_| ProvisioningError::Conflict)?;
        if package.approval_record().record_id != command.policy_record_id {
            return Err(ProvisioningError::Conflict);
        }
        if super::roles::is_managed(&self.publication_root, &package).map_err(owner_error)? {
            return self.prepare_managed(
                command,
                original,
                proposal_generation,
                &package,
                &installed,
                &compiler,
            );
        }
        let grant = command.grant.as_ref().ok_or(ProvisioningError::Invalid)?;
        let relationships = build_grant(&package, original, grant).map_err(approval_error)?;
        let updates = relationships
            .iter()
            .map(crate::spicedb_candidate::relationship_update)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ProvisioningError::Invalid)?;
        let composition = installed.composition();
        let proposed = StoredRequest::new(
            command,
            original,
            proposal_generation,
            &package,
            &updates,
            composition,
        )?;
        let store = RequestStore::open(&self.publication_root, command, proposal_generation)?;
        let retained = store.read()?;
        if retained
            .as_ref()
            .is_some_and(|record| !record.same_input(&proposed))
        {
            return Err(ProvisioningError::Conflict);
        }
        let types = package
            .policy()
            .program()
            .definitions
            .iter()
            .map(|d| d.name.clone())
            .collect::<Vec<_>>();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| ProvisioningError::Unavailable)?;
        runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(90), async {
                let mut client = GrantClient::connect(&self.endpoint, &self.token_path).await?;
                installed
                    .check_live(&mut client, &compiler)
                    .await
                    .map_err(owner_error)?;
                if let Some(retained) = retained.as_ref().filter(|r| r.zed_token.is_some()) {
                    return Ok(RequestRevision {
                        stored: retained.clone(),
                    });
                }
                // Durability precedes every possible mutation, including a retry.
                store.persist(&proposed)?;
                let token = write_or_reconcile(&mut client, &types, &updates).await?;
                let mut complete = proposed;
                complete.zed_token = Some(token);
                store.persist(&complete)?;
                Ok(RequestRevision { stored: complete })
            })
            .await
            .map_err(|_| ProvisioningError::Unavailable)?
        })
    }

    fn prepare_managed(
        &self,
        command: &proto::ApproveAccessRequestRequest,
        original: &proto::CreateAccessRequestRequest,
        generation: u64,
        package: &crate::policy_package::ApprovedPolicyPackage,
        installed: &Installation,
        compiler: &Compiler,
    ) -> Result<RequestRevision, ProvisioningError> {
        let store = RequestStore::open(&self.publication_root, command, generation)?;
        if let Some(record) = store.read()? {
            if record.format_version != 2
                || record.zed_token.is_none()
                || !record.matches_request(command, original, generation, package)
            {
                return Err(ProvisioningError::Conflict);
            }
            return Ok(RequestRevision { stored: record });
        }
        let types = package
            .policy()
            .program()
            .definitions
            .iter()
            .map(|d| d.name.clone())
            .collect::<Vec<_>>();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| ProvisioningError::Unavailable)?;
        runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(90), async {
                let mut client = GrantClient::connect(&self.endpoint, &self.token_path).await?;
                installed
                    .check_live(&mut client, compiler)
                    .await
                    .map_err(owner_error)?;
                // This path is read-only. Later role edits cannot replace this exact revision.
                let observed = client.snapshot(&types).await?;
                let record = StoredRequest::from_observed(
                    command,
                    original,
                    generation,
                    package,
                    &observed,
                    installed.composition(),
                )?;
                store.persist(&record)?;
                Ok(RequestRevision { stored: record })
            })
            .await
            .map_err(|_| ProvisioningError::Unavailable)?
        })
    }
}

async fn write_or_reconcile(
    api: &mut impl GrantApi,
    types: &[String],
    updates: &[api::RelationshipUpdate],
) -> Result<String, ProvisioningError> {
    let observed = api.snapshot(types).await?;
    let mut missing = 0;
    for update in updates {
        let wanted = update
            .relationship
            .as_ref()
            .ok_or(ProvisioningError::Invalid)?;
        let matching = observed
            .relationships
            .iter()
            .filter(|actual| same_identity(actual, wanted))
            .collect::<Vec<_>>();
        match matching.as_slice() {
            [] => missing += 1,
            [actual] if same_values(actual, wanted) => {}
            _ => return Err(ProvisioningError::Conflict),
        }
    }
    if observed.relationships.len() + missing > 256 {
        return Err(ProvisioningError::Invalid);
    }
    if missing == 0 {
        return Ok(observed.token);
    }
    // Every existing wanted identity was read at one fully consistent revision
    // and matched exactly. The single owner lock prevents intervening owner writes.
    api.touch(updates).await
}

fn same_identity(a: &api::Relationship, b: &api::Relationship) -> bool {
    a.resource == b.resource && a.relation == b.relation && a.subject == b.subject
}
fn same_values(a: &api::Relationship, b: &api::Relationship) -> bool {
    let normalize = |input: &api::Relationship| {
        let mut value = input.clone();
        if let Some(caveat) = &mut value.optional_caveat
            && caveat
                .context
                .as_ref()
                .is_some_and(|context| context.fields.is_empty())
        {
            caveat.context = None;
        }
        value
    };
    normalize(a) == normalize(b)
}
fn approval_error(error: ApprovalError) -> ProvisioningError {
    match error {
        ApprovalError::Invalid => ProvisioningError::Invalid,
        ApprovalError::Conflict => ProvisioningError::Conflict,
        ApprovalError::Unavailable => ProvisioningError::Unavailable,
    }
}
fn owner_error(error: StoreActivationError) -> ProvisioningError {
    match error {
        StoreActivationError::Conflict => ProvisioningError::Conflict,
        StoreActivationError::InvalidRequest => ProvisioningError::Invalid,
        _ => ProvisioningError::Unavailable,
    }
}
