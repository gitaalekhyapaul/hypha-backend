//! Owner-only approval preparation. This creates no grant, snapshot, or proof.

mod grant;
mod recipe;
#[cfg(test)]
mod tests;
mod validation;

use std::fmt;
use std::path::PathBuf;

use raziel_proto::raziel::v1 as proto;

use crate::dataset_binding::{
    BindingError, DatasetBinding, DatasetBindingStore, FinalizedDatasetRevision,
};
use crate::policy_package::PolicyPackageKey;
use crate::policy_publication::catalogue::{
    CatalogueConfig, InstalledApprovalError, resolve_installed,
};

pub use grant::build_grant;
pub use recipe::GrantInputField;
pub(crate) use recipe::required_grant_fields;
pub use validation::validate_command;

#[derive(Clone)]
pub struct ApprovalConfig {
    pub publication_root: PathBuf,
    pub package_root: PathBuf,
    pub registry_root: PathBuf,
    pub binding_root: PathBuf,
    pub trust_domain_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalError {
    Invalid,
    Conflict,
    Unavailable,
}

impl fmt::Display for ApprovalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Invalid => "invalid access approval",
            Self::Conflict => "access approval conflicts with current state",
            Self::Unavailable => "access approval is unavailable",
        })
    }
}

impl std::error::Error for ApprovalError {}

impl ApprovalConfig {
    /// Call under the request lifecycle lock after its generation/state check.
    /// Dataset metadata must come from the trusted finalized-dataset service.
    /// The returned binding is not an ALLOW decision or an authorization proof.
    pub fn prepare(
        &self,
        command: &proto::ApproveAccessRequestRequest,
        original: &proto::CreateAccessRequestRequest,
        now: i64,
    ) -> Result<proto::AccessPolicyBinding, ApprovalError> {
        validate_command(command)?;
        let dataset = validation::match_original(command, original)?;
        let key = PolicyPackageKey::new(command.policy_version_id.clone(), command.approval_epoch)
            .map_err(|_| ApprovalError::Invalid)?;
        let installed = resolve_installed(
            &CatalogueConfig {
                publication_root: self.publication_root.clone(),
                package_root: self.package_root.clone(),
                registry_root: self.registry_root.clone(),
                trust_domain_id: self.trust_domain_id.clone(),
            },
            &key,
            &command.policy_record_id,
            now,
        )
        .map_err(|error| match error {
            InstalledApprovalError::Conflict => ApprovalError::Conflict,
            InstalledApprovalError::Unavailable => ApprovalError::Unavailable,
        })?;
        recipe::validate_policy(&installed.package)?;
        let binding = DatasetBinding::for_approved_policy(
            &installed.package,
            dataset.dataset_id.clone(),
            FinalizedDatasetRevision {
                dataset_id: dataset.dataset_id.clone(),
                revision: dataset.revision,
                checksum: dataset
                    .checksum
                    .as_slice()
                    .try_into()
                    .map_err(|_| ApprovalError::Invalid)?,
            },
        )
        .map_err(binding_error)?;
        DatasetBindingStore::open(self.binding_root.clone())
            .and_then(|store| store.retain(&binding))
            .map_err(binding_error)?;
        Ok(proto::AccessPolicyBinding {
            policy_record_id: installed.package.approval_record().record_id.clone(),
            policy_hash: installed.package.policy_hash().into(),
            resource_type: binding.resource_type,
            resource_scope: binding.resource_scope,
        })
    }
}

fn binding_error(error: BindingError) -> ApprovalError {
    match error {
        BindingError::Conflict => ApprovalError::Conflict,
        BindingError::Invalid | BindingError::NotFound | BindingError::Unavailable => {
            ApprovalError::Unavailable
        }
    }
}
