use std::fmt;
use std::time::Duration;

use spicedb_rs_proto::authzed::api::v1 as authzed;
use spicedb_rs_proto::authzed::api::v1::permissions_service_client::PermissionsServiceClient;
use spicedb_rs_proto::authzed::api::v1::schema_service_client::SchemaServiceClient;
use tonic::metadata::{Ascii, MetadataValue};
use tonic::transport::{Channel, Endpoint};
use tonic::{Request, Status};

use super::{
    MAX_SPICEDB_MESSAGE_BYTES, RequestSnapshotSource, SpiceDbReadApi, SpiceDbSnapshotError,
    read_exact, read_exact_composition,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const RPC_TIMEOUT: Duration = Duration::from_secs(15);

/// Direct generated-client binding for one configured SpiceDB endpoint.
pub struct SpiceDbSnapshotReader {
    permissions: PermissionsServiceClient<Channel>,
    schema: SchemaServiceClient<Channel>,
    authorization: MetadataValue<Ascii>,
}

impl SpiceDbSnapshotReader {
    /// Connects to the owner-selected plaintext endpoint used by the PoC network.
    pub async fn connect(endpoint: &str, bearer_token: &str) -> Result<Self, SpiceDbSnapshotError> {
        if !endpoint.starts_with("http://") || endpoint.trim() != endpoint {
            return Err(SpiceDbSnapshotError::InvalidConfiguration);
        }
        let endpoint = Endpoint::from_shared(endpoint.to_owned())
            .map_err(|_| SpiceDbSnapshotError::InvalidConfiguration)?
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(RPC_TIMEOUT);
        let channel = endpoint
            .connect()
            .await
            .map_err(|_| SpiceDbSnapshotError::ConnectionFailed)?;
        let mut authorization: MetadataValue<Ascii> =
            format!("Bearer {bearer_token}")
                .try_into()
                .map_err(|_| SpiceDbSnapshotError::AuthenticationFailed)?;
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

    /// Reads and converts all policy resource types at one exact revision.
    pub async fn read_exact(
        &mut self,
        policy: &raziel_policy_ir::ValidatedPolicyProgram,
        zed_token: &str,
    ) -> Result<Vec<raziel_snapshot::RelationshipRecord>, SpiceDbSnapshotError> {
        read_exact(self, policy, zed_token).await
    }

    /// Checks the complete accepted composition before reading selected-policy tuples.
    pub async fn read_request_source(
        &mut self,
        policy: &raziel_policy_ir::ValidatedPolicyProgram,
        source: &RequestSnapshotSource,
    ) -> Result<Vec<raziel_snapshot::RelationshipRecord>, SpiceDbSnapshotError> {
        source.validate(policy)?;
        read_exact_composition(
            self,
            policy,
            &source.zed_token,
            Some(&source.accepted_composition_schema),
        )
        .await
    }

    fn request<T>(&self, message: T) -> Request<T> {
        let mut request = Request::new(message);
        request
            .metadata_mut()
            .insert("authorization", self.authorization.clone());
        request
    }
}

#[tonic::async_trait]
impl SpiceDbReadApi for SpiceDbSnapshotReader {
    async fn diff_schema(
        &mut self,
        request: authzed::DiffSchemaRequest,
    ) -> Result<authzed::DiffSchemaResponse, SpiceDbSnapshotError> {
        let request = self.request(request);
        self.schema
            .diff_schema(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(|status| {
                if status.code() == tonic::Code::Unauthenticated {
                    SpiceDbSnapshotError::AuthenticationFailed
                } else {
                    SpiceDbSnapshotError::SchemaComparisonFailed
                }
            })
    }
    async fn reflect_schema(
        &mut self,
        request: authzed::ReflectSchemaRequest,
    ) -> Result<authzed::ReflectSchemaResponse, SpiceDbSnapshotError> {
        let request = self.request(request);
        self.schema
            .reflect_schema(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(map_reflection_status)
    }

    async fn read_relationships(
        &mut self,
        request: authzed::ReadRelationshipsRequest,
    ) -> Result<Vec<authzed::ReadRelationshipsResponse>, SpiceDbSnapshotError> {
        let request = self.request(request);
        let mut stream = self
            .permissions
            .read_relationships(request)
            .await
            .map_err(map_read_status)?
            .into_inner();
        let mut responses = Vec::new();
        while let Some(response) = stream
            .message()
            .await
            .map_err(|_| SpiceDbSnapshotError::ReadFailed)?
        {
            responses.push(response);
            if responses.len() > super::AGGREGATE_READ_LIMIT as usize {
                return Err(SpiceDbSnapshotError::RelationshipLimitExceeded);
            }
        }
        Ok(responses)
    }
}

fn map_reflection_status(status: Status) -> SpiceDbSnapshotError {
    if status.code() == tonic::Code::Unauthenticated {
        SpiceDbSnapshotError::AuthenticationFailed
    } else {
        SpiceDbSnapshotError::ReflectionFailed
    }
}

fn map_read_status(status: Status) -> SpiceDbSnapshotError {
    if status.code() == tonic::Code::Unauthenticated {
        SpiceDbSnapshotError::AuthenticationFailed
    } else {
        SpiceDbSnapshotError::ReadFailed
    }
}

impl fmt::Debug for SpiceDbSnapshotReader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SpiceDbSnapshotReader")
            .field("configuration", &"<redacted>")
            .finish()
    }
}
