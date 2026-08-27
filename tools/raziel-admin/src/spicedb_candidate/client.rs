use std::fmt;
use std::time::Duration;

use spicedb_rs_proto::authzed::api::v1 as authzed;
use spicedb_rs_proto::authzed::api::v1::permissions_service_client::PermissionsServiceClient;
use spicedb_rs_proto::authzed::api::v1::schema_service_client::SchemaServiceClient;
use tonic::Request;
use tonic::metadata::{Ascii, MetadataValue};
use tonic::transport::{Channel, Endpoint};

use super::{CandidateWriteError, MAX_SPICEDB_MESSAGE_BYTES, SpiceDbWriteApi};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const RPC_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) struct SpiceDbCandidateWriter {
    permissions: PermissionsServiceClient<Channel>,
    schema: SchemaServiceClient<Channel>,
    authorization: MetadataValue<Ascii>,
}

impl SpiceDbCandidateWriter {
    pub(super) async fn connect(
        endpoint: &str,
        bearer_token: &str,
    ) -> Result<Self, CandidateWriteError> {
        if !endpoint.starts_with("http://") || endpoint.trim() != endpoint {
            return Err(CandidateWriteError::InvalidConfiguration);
        }
        let endpoint = Endpoint::from_shared(endpoint.to_owned())
            .map_err(|_| CandidateWriteError::InvalidConfiguration)?
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(RPC_TIMEOUT);
        let channel = endpoint
            .connect()
            .await
            .map_err(|_| CandidateWriteError::ConnectionFailed)?;
        let mut authorization: MetadataValue<Ascii> =
            format!("Bearer {bearer_token}")
                .try_into()
                .map_err(|_| CandidateWriteError::InvalidCredential)?;
        authorization.set_sensitive(true);
        Ok(Self {
            permissions: PermissionsServiceClient::new(channel.clone())
                .max_decoding_message_size(MAX_SPICEDB_MESSAGE_BYTES)
                .max_encoding_message_size(MAX_SPICEDB_MESSAGE_BYTES),
            schema: SchemaServiceClient::new(channel)
                .max_decoding_message_size(MAX_SPICEDB_MESSAGE_BYTES)
                .max_encoding_message_size(MAX_SPICEDB_MESSAGE_BYTES),
            authorization,
        })
    }

    fn request<T>(&self, value: T) -> Request<T> {
        let mut request = Request::new(value);
        request
            .metadata_mut()
            .insert("authorization", self.authorization.clone());
        request
    }
}

#[tonic::async_trait]
impl SpiceDbWriteApi for SpiceDbCandidateWriter {
    async fn write_schema(
        &mut self,
        request: authzed::WriteSchemaRequest,
    ) -> Result<authzed::WriteSchemaResponse, CandidateWriteError> {
        let request = self.request(request);
        self.schema
            .write_schema(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(|status| {
                if status.code() == tonic::Code::Unauthenticated {
                    CandidateWriteError::InvalidCredential
                } else {
                    CandidateWriteError::SchemaWriteFailed
                }
            })
    }

    async fn write_relationships(
        &mut self,
        request: authzed::WriteRelationshipsRequest,
    ) -> Result<authzed::WriteRelationshipsResponse, CandidateWriteError> {
        let request = self.request(request);
        self.permissions
            .write_relationships(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(|status| {
                if status.code() == tonic::Code::Unauthenticated {
                    CandidateWriteError::InvalidCredential
                } else {
                    CandidateWriteError::RelationshipWriteFailed
                }
            })
    }
}

impl fmt::Debug for SpiceDbCandidateWriter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SpiceDbCandidateWriter")
            .field("configuration", &"<redacted>")
            .finish()
    }
}
