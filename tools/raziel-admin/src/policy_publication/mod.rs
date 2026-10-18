//! Owner-only publication of coexisting policy versions. This never creates grants.

pub mod catalogue;
mod compiler;
mod files;
mod installation;
mod network;
pub mod provisioning;
pub mod roles;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use crate::policy_activation::store::{
    PolicyActivationStore, StoreActivationError, StoredActivationProjection,
};
use crate::policy_activation::wire::PolicyActivationRequestWire;
use crate::policy_package::{PolicyPackageInput, PolicyPackageStore};
use crate::policy_version::PolicyVersionStore;

use compiler::Compiler;
use files::Artifacts;
use installation::Installation;

pub(crate) struct PublicationConfig {
    pub root: PathBuf,
    pub package_root: PathBuf,
    pub version_root: PathBuf,
    pub candidate_root: PathBuf,
    pub registry_root: PathBuf,
    pub compiler: PathBuf,
    pub legacy_root: Option<PathBuf>,
    pub token_path: PathBuf,
    pub endpoint: String,
    pub trust_domain_id: String,
}

pub(crate) struct PolicyPublisher {
    config: PublicationConfig,
    activation: PolicyActivationStore,
}

impl PolicyPublisher {
    pub(crate) fn catalogue(
        &self,
        trusted_now: i64,
    ) -> Result<catalogue::CatalogueProjection, catalogue::CatalogueError> {
        catalogue::list_approved(
            &catalogue::CatalogueConfig {
                publication_root: self.config.root.clone(),
                package_root: self.config.package_root.clone(),
                registry_root: self.config.registry_root.clone(),
                trust_domain_id: self.config.trust_domain_id.clone(),
            },
            trusted_now,
        )
    }

    pub(crate) fn new(
        config: PublicationConfig,
        activation: PolicyActivationStore,
    ) -> Result<Self, StoreActivationError> {
        for path in [&config.root, &config.package_root, &config.version_root] {
            files::private_directory(path)?;
        }
        let roots = [
            &config.root,
            &config.package_root,
            &config.version_root,
            &config.candidate_root,
        ];
        let overlap = roots.iter().enumerate().any(|(i, root)| {
            roots[i + 1..]
                .iter()
                .any(|other| root.starts_with(other) || other.starts_with(root))
        });
        if overlap || !config.compiler.is_absolute() {
            return Err(StoreActivationError::InvalidRequest);
        }
        Ok(Self { config, activation })
    }

    pub(crate) fn publish(
        &self,
        request: PolicyActivationRequestWire,
    ) -> Result<StoredActivationProjection, StoreActivationError> {
        let c = &self.config;
        // Shared with subsequent grant provisioning, never held through proving.
        let lock = std::fs::File::open(&c.root).map_err(|_| StoreActivationError::Unavailable)?;
        lock.try_lock()
            .map_err(|_| StoreActivationError::Unavailable)?;
        let allocation = PolicyVersionStore::open_read_only(c.version_root.clone())
            .and_then(|store| store.read(&request.candidate_id))
            .map_err(|_| StoreActivationError::CandidateRejected)?;
        let candidate = Artifacts::load(&c.candidate_root.join(allocation.candidate_id()))?;
        candidate.check_identity(allocation.identity())?;
        let now = crate::policy_activation::store::trusted_now()?;
        let mut packages = PolicyPackageStore::new(
            c.package_root.clone(),
            c.registry_root.clone(),
            &c.trust_domain_id,
        )
        .map_err(|_| StoreActivationError::Unavailable)?;
        let compiler = Compiler::new(&c.compiler, &c.root)?;
        let mut installed = Installation::load(c, &mut packages, &compiler, now)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| StoreActivationError::Unavailable)?;
        let mut network =
            runtime.block_on(network::SchemaClient::connect(&c.endpoint, &c.token_path))?;
        // Reconcile a previously uncertain write before admitting a new publication.
        runtime.block_on(installed.reconcile(&mut network, &mut packages, &compiler, now))?;
        let target = installed.prepare(
            &candidate,
            allocation.identity(),
            &mut packages,
            &compiler,
            now,
        )?;
        runtime.block_on(installed.check_live(&mut network, &compiler))?;
        // The aggregate bounds and drift checks run before the authority signs.
        let projection = self.activation.activate_version(request)?;
        let approved = packages
            .retain(
                PolicyPackageInput {
                    identity: allocation.identity().clone(),
                    candidate_manifest: &candidate.manifest,
                    source_schema: &candidate.schema,
                    compiler_config: &candidate.config,
                    policy_pb: &candidate.policy,
                },
                crate::policy_activation::store::trusted_now()?,
            )
            .map_err(|_| StoreActivationError::CandidateRejected)?;
        if approved.key().version_id() != target.version_id
            || approved.key().approval_epoch() != target.approval_epoch
        {
            return Err(StoreActivationError::Conflict);
        }
        runtime.block_on(installed.install(target, &mut network, &compiler))?;
        Ok(projection)
    }
}
