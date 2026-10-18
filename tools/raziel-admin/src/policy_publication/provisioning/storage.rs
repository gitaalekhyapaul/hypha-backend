use std::path::{Path, PathBuf};

use prost::Message;
use raziel_proto::raziel::v1 as proto;
use spicedb_rs_proto::authzed::api::v1 as api;

use super::super::{compiler::Composition, files};
use super::{ProvisioningError, owner_error};
use crate::policy_package::ApprovedPolicyPackage;

const MAX_RECORD: u64 = 4 * 1024 * 1024;

#[derive(Clone, PartialEq, Message)]
#[prost(skip_debug)]
pub(super) struct StoredRequest {
    #[prost(uint32, tag = "1")]
    pub format_version: u32,
    #[prost(string, tag = "2")]
    pub agent_id: String,
    #[prost(string, tag = "3")]
    pub request_id: String,
    #[prost(uint64, tag = "4")]
    pub proposal_generation: u64,
    #[prost(bytes = "vec", tag = "5")]
    pub command: Vec<u8>,
    #[prost(bytes = "vec", tag = "6")]
    pub original: Vec<u8>,
    #[prost(string, tag = "7")]
    pub policy_record_id: String,
    #[prost(bytes = "vec", tag = "8")]
    pub policy_hash: Vec<u8>,
    #[prost(bytes = "vec", tag = "9")]
    pub source_schema_digest: Vec<u8>,
    #[prost(bytes = "vec", repeated, tag = "10")]
    pub relationships: Vec<Vec<u8>>,
    #[prost(string, tag = "11")]
    pub accepted_schema: String,
    #[prost(bytes = "vec", tag = "12")]
    pub accepted_reflection: Vec<u8>,
    #[prost(string, optional, tag = "13")]
    pub zed_token: Option<String>,
}

impl std::fmt::Debug for StoredRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredRequest")
            .field("private_fields", &"<redacted>")
            .finish()
    }
}

impl StoredRequest {
    pub fn new(
        command: &proto::ApproveAccessRequestRequest,
        original: &proto::CreateAccessRequestRequest,
        generation: u64,
        package: &ApprovedPolicyPackage,
        updates: &[api::RelationshipUpdate],
        composition: &Composition,
    ) -> Result<Self, ProvisioningError> {
        let relationships = updates
            .iter()
            .map(|u| {
                u.relationship
                    .as_ref()
                    .map(Message::encode_to_vec)
                    .ok_or(ProvisioningError::Invalid)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            format_version: 1,
            agent_id: command.agent_id.clone(),
            request_id: command.request_id.clone(),
            proposal_generation: generation,
            command: command.encode_to_vec(),
            original: original.encode_to_vec(),
            policy_record_id: package.approval_record().record_id.clone(),
            policy_hash: package.policy_hash().into(),
            source_schema_digest: package.policy().program().source_schema_digest.to_vec(),
            relationships,
            accepted_schema: String::from_utf8(composition.schema.clone())
                .map_err(|_| ProvisioningError::Invalid)?,
            accepted_reflection: composition.reflection.clone(),
            zed_token: None,
        })
    }
    pub fn same_input(&self, other: &Self) -> bool {
        self.format_version == other.format_version
            && self.agent_id == other.agent_id
            && self.request_id == other.request_id
            && self.proposal_generation == other.proposal_generation
            && self.command == other.command
            && self.original == other.original
            && self.policy_record_id == other.policy_record_id
            && self.policy_hash == other.policy_hash
            && self.source_schema_digest == other.source_schema_digest
            && self.relationships == other.relationships
    }

    pub fn matches_request(
        &self,
        command: &proto::ApproveAccessRequestRequest,
        original: &proto::CreateAccessRequestRequest,
        generation: u64,
        package: &ApprovedPolicyPackage,
    ) -> bool {
        self.agent_id == command.agent_id
            && self.request_id == command.request_id
            && self.proposal_generation == generation
            && self.command == command.encode_to_vec()
            && self.original == original.encode_to_vec()
            && self.policy_record_id == package.approval_record().record_id
            && self.policy_hash == package.policy_hash()
            && self.source_schema_digest == package.policy().program().source_schema_digest
    }

    pub fn from_observed(
        command: &proto::ApproveAccessRequestRequest,
        original: &proto::CreateAccessRequestRequest,
        generation: u64,
        package: &ApprovedPolicyPackage,
        observed: &super::network::Observed,
        composition: &Composition,
    ) -> Result<Self, ProvisioningError> {
        let updates = observed
            .relationships
            .iter()
            .map(|r| api::RelationshipUpdate {
                operation: api::relationship_update::Operation::Touch as i32,
                relationship: Some(r.clone()),
            })
            .collect::<Vec<_>>();
        let mut record = Self::new(
            command,
            original,
            generation,
            package,
            &updates,
            composition,
        )?;
        record.format_version = 2;
        record.zed_token = Some(observed.token.clone());
        record.validate()?;
        Ok(record)
    }
    fn validate(&self) -> Result<(), ProvisioningError> {
        if !matches!(self.format_version, 1 | 2)
            || self.proposal_generation == 0
            || self.command.len() > 4096
            || self.original.len() > 16 * 1024
            || self.policy_hash.len() != 32
            || self.source_schema_digest.len() != 32
            || self.format_version == 1 && self.relationships.is_empty()
            || self.relationships.len() > 256
            || self.accepted_schema.is_empty()
            || self.accepted_schema.len() > 1024 * 1024
            || self.accepted_reflection.len() > 1024 * 1024
            || self.encoded_len() as u64 > MAX_RECORD
        {
            return Err(ProvisioningError::Unavailable);
        }
        if let Some(token) = &self.zed_token {
            super::network::revision(Some(api::ZedToken {
                token: token.clone(),
            }))?;
        }
        Ok(())
    }
}

pub(super) struct RequestStore {
    root: PathBuf,
    name: String,
    agent_id: String,
    request_id: String,
    generation: u64,
}
impl RequestStore {
    pub fn open(
        publication: &Path,
        command: &proto::ApproveAccessRequestRequest,
        generation: u64,
    ) -> Result<Self, ProvisioningError> {
        let owner = files::private_directory(publication).map_err(owner_error)?;
        let mut root = publication.to_owned();
        for part in ["requests", &command.agent_id, &command.request_id] {
            let parent = root.clone();
            root.push(part);
            files::create_private_directory(&root).map_err(owner_error)?;
            if files::private_directory(&root).map_err(owner_error)? != owner {
                return Err(ProvisioningError::Unavailable);
            }
            crate::io::sync_directory(&parent).map_err(|_| ProvisioningError::Unavailable)?;
        }
        Ok(Self {
            root,
            name: format!("{generation}.pb"),
            agent_id: command.agent_id.clone(),
            request_id: command.request_id.clone(),
            generation,
        })
    }
    pub fn read(&self) -> Result<Option<StoredRequest>, ProvisioningError> {
        match std::fs::symlink_metadata(self.root.join(&self.name)) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(ProvisioningError::Unavailable),
            Ok(_) => {}
        }
        let bytes = files::read(&self.root, &self.name, MAX_RECORD).map_err(owner_error)?;
        let record =
            StoredRequest::decode(bytes.as_slice()).map_err(|_| ProvisioningError::Unavailable)?;
        if record.encode_to_vec() != bytes {
            return Err(ProvisioningError::Unavailable);
        }
        self.check_identity(&record)?;
        record.validate()?;
        Ok(Some(record))
    }
    pub fn persist(&self, record: &StoredRequest) -> Result<(), ProvisioningError> {
        self.check_identity(record)?;
        record.validate()?;
        if let Some(existing) = self.read()? {
            if !existing.same_input(record) || existing.zed_token.is_some() && existing != *record {
                return Err(ProvisioningError::Conflict);
            }
            if existing == *record {
                return Ok(());
            }
            crate::io::atomic_write(&self.root.join(&self.name), &record.encode_to_vec())
                .map_err(|_| ProvisioningError::Unavailable)
        } else {
            files::write_new(&self.root, &self.name, &record.encode_to_vec()).map_err(owner_error)
        }
    }
    fn check_identity(&self, record: &StoredRequest) -> Result<(), ProvisioningError> {
        if record.agent_id != self.agent_id
            || record.request_id != self.request_id
            || record.proposal_generation != self.generation
        {
            return Err(ProvisioningError::Conflict);
        }
        Ok(())
    }
}
