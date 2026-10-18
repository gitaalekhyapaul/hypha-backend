use std::path::Path;
use std::time::Duration;

use api::permissions_service_client::PermissionsServiceClient;
use prost::Message;
use spicedb_rs_proto::authzed::api::v1 as api;
use tonic::transport::Channel;

use super::super::network::{SchemaApi, SchemaClient};
use super::{ProvisioningError, owner_error};
use crate::policy_activation::store::StoreActivationError;

pub(in crate::policy_publication) struct Observed {
    pub token: String,
    pub relationships: Vec<api::Relationship>,
}

#[tonic::async_trait]
pub(in crate::policy_publication) trait GrantApi: SchemaApi {
    async fn snapshot(&mut self, types: &[String]) -> Result<Observed, ProvisioningError>;
    async fn touch(
        &mut self,
        updates: &[api::RelationshipUpdate],
    ) -> Result<String, ProvisioningError>;
}

pub(in crate::policy_publication) struct GrantClient {
    schema: SchemaClient,
    permissions: PermissionsServiceClient<Channel>,
}
impl GrantClient {
    pub async fn connect(endpoint: &str, token_path: &Path) -> Result<Self, ProvisioningError> {
        let schema = SchemaClient::connect(endpoint, token_path)
            .await
            .map_err(owner_error)?;
        let permissions = PermissionsServiceClient::new(schema.channel())
            .max_decoding_message_size(2 * 1024 * 1024)
            .max_encoding_message_size(2 * 1024 * 1024);
        Ok(Self {
            schema,
            permissions,
        })
    }
}

#[tonic::async_trait]
impl SchemaApi for GrantClient {
    async fn read(&mut self) -> Result<Vec<u8>, StoreActivationError> {
        self.schema.read().await
    }
    async fn write(&mut self, _schema: &[u8]) -> Result<(), StoreActivationError> {
        Err(StoreActivationError::InvalidRequest)
    }
}

#[tonic::async_trait]
impl GrantApi for GrantClient {
    async fn snapshot(&mut self, types: &[String]) -> Result<Observed, ProvisioningError> {
        if types.is_empty() || types.len() > 16 {
            return Err(ProvisioningError::Invalid);
        }
        let reflection = self.schema.current_revision().await.map_err(owner_error)?;
        let token = revision(reflection.read_at)?;
        let mut relationships = Vec::new();
        let mut bytes = 0;
        for resource_type in types {
            let request = self.schema.request(api::ReadRelationshipsRequest {
                consistency: Some(api::Consistency {
                    requirement: Some(api::consistency::Requirement::AtExactSnapshot(
                        api::ZedToken {
                            token: token.clone(),
                        },
                    )),
                }),
                relationship_filter: Some(api::RelationshipFilter {
                    resource_type: resource_type.clone(),
                    optional_resource_id: String::new(),
                    optional_relation: String::new(),
                    optional_subject_filter: None,
                    optional_resource_id_prefix: String::new(),
                }),
                optional_limit: 257 - relationships.len() as u32,
                optional_cursor: None,
            });
            let mut stream = self
                .permissions
                .read_relationships(request)
                .await
                .map_err(|_| ProvisioningError::Unavailable)?
                .into_inner();
            loop {
                let response = tokio::time::timeout(Duration::from_secs(15), stream.message())
                    .await
                    .map_err(|_| ProvisioningError::Unavailable)?
                    .map_err(|_| ProvisioningError::Unavailable)?;
                let Some(response) = response else {
                    break;
                };
                if revision(response.read_at)? != token {
                    return Err(ProvisioningError::Conflict);
                }
                let relationship = response
                    .relationship
                    .ok_or(ProvisioningError::Unavailable)?;
                if relationship
                    .resource
                    .as_ref()
                    .is_none_or(|r| r.object_type != *resource_type)
                {
                    return Err(ProvisioningError::Conflict);
                }
                bytes += relationship.encoded_len();
                if bytes > 1024 * 1024 || relationships.len() >= 256 {
                    return Err(ProvisioningError::Invalid);
                }
                relationships.push(relationship);
            }
        }
        Ok(Observed {
            token,
            relationships,
        })
    }
    async fn touch(
        &mut self,
        updates: &[api::RelationshipUpdate],
    ) -> Result<String, ProvisioningError> {
        if updates.is_empty() || updates.len() > 256 {
            return Err(ProvisioningError::Invalid);
        }
        let request = self.schema.request(api::WriteRelationshipsRequest {
            updates: updates.into(),
            optional_preconditions: Vec::new(),
            optional_transaction_metadata: None,
        });
        let response = self
            .permissions
            .write_relationships(request)
            .await
            .map_err(|_| ProvisioningError::Unavailable)?
            .into_inner();
        revision(response.written_at)
    }
}

pub(super) fn revision(value: Option<api::ZedToken>) -> Result<String, ProvisioningError> {
    value
        .map(|value| value.token)
        .filter(|token| {
            !token.is_empty()
                && token.len() <= 4096
                && token.is_ascii()
                && !token.bytes().any(|b| b.is_ascii_control())
        })
        .ok_or(ProvisioningError::Unavailable)
}
