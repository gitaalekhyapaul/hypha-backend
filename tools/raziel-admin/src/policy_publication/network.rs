use std::path::Path;
use std::time::Duration;

use api::schema_service_client::SchemaServiceClient;
use spicedb_rs_proto::authzed::api::v1 as api;
use tonic::metadata::{Ascii, MetadataValue};
use tonic::transport::{Channel, Endpoint};

use super::files;
use crate::policy_activation::store::StoreActivationError as Error;

#[tonic::async_trait]
pub(super) trait SchemaApi {
    async fn read(&mut self) -> Result<Vec<u8>, Error>;
    async fn write(&mut self, schema: &[u8]) -> Result<(), Error>;
}

pub(super) struct SchemaClient {
    client: SchemaServiceClient<Channel>,
    channel: Channel,
    authorization: MetadataValue<Ascii>,
}

impl SchemaClient {
    pub async fn connect(endpoint: &str, token_path: &Path) -> Result<Self, Error> {
        if !endpoint.starts_with("http://") || endpoint.trim() != endpoint {
            return Err(Error::InvalidRequest);
        }
        let parent = token_path.parent().ok_or(Error::InvalidRequest)?;
        let name = token_path
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or(Error::InvalidRequest)?;
        let token = files::read_secret(parent, name, 4096)?;
        let token = std::str::from_utf8(&token)
            .map_err(|_| Error::InvalidRequest)?
            .trim();
        if token.is_empty() || token.bytes().any(|b| b.is_ascii_control()) {
            return Err(Error::InvalidRequest);
        }
        let mut authorization = MetadataValue::try_from(format!("Bearer {token}"))
            .map_err(|_| Error::InvalidRequest)?;
        authorization.set_sensitive(true);
        let channel = Endpoint::from_shared(endpoint.to_string())
            .map_err(|_| Error::InvalidRequest)?
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .connect()
            .await
            .map_err(|_| Error::Unavailable)?;
        Ok(Self {
            client: SchemaServiceClient::new(channel.clone())
                .max_decoding_message_size(2 * files::MAX_SCHEMA as usize)
                .max_encoding_message_size(2 * files::MAX_SCHEMA as usize),
            authorization,
            channel,
        })
    }
    pub(super) fn channel(&self) -> Channel {
        self.channel.clone()
    }

    pub(super) async fn current_revision(&mut self) -> Result<api::ReflectSchemaResponse, Error> {
        let request = self.request(api::ReflectSchemaRequest {
            consistency: Some(api::Consistency {
                requirement: Some(api::consistency::Requirement::FullyConsistent(true)),
            }),
            optional_filters: Vec::new(),
        });
        self.client
            .reflect_schema(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(|_| Error::Unavailable)
    }

    pub(super) fn request<T>(&self, value: T) -> tonic::Request<T> {
        let mut request = tonic::Request::new(value);
        request
            .metadata_mut()
            .insert("authorization", self.authorization.clone());
        request
    }
}

#[tonic::async_trait]
impl SchemaApi for SchemaClient {
    async fn read(&mut self) -> Result<Vec<u8>, Error> {
        let request = self.request(api::ReadSchemaRequest {});
        match self.client.read_schema(request).await {
            Ok(response) => {
                let bytes = response.into_inner().schema_text.into_bytes();
                if bytes.len() > files::MAX_SCHEMA as usize {
                    return Err(Error::CandidateRejected);
                }
                Ok(bytes)
            }
            Err(status) if status.code() == tonic::Code::NotFound => Ok(Vec::new()),
            Err(_) => Err(Error::Unavailable),
        }
    }
    async fn write(&mut self, schema: &[u8]) -> Result<(), Error> {
        let schema = std::str::from_utf8(schema).map_err(|_| Error::CandidateRejected)?;
        let request = self.request(api::WriteSchemaRequest {
            schema: schema.to_owned(),
        });
        let response = self
            .client
            .write_schema(request)
            .await
            .map_err(|_| Error::Unavailable)?
            .into_inner();
        if response
            .written_at
            .is_none_or(|t| t.token.is_empty() || t.token.len() > 4096)
        {
            return Err(Error::Unavailable);
        }
        Ok(())
    }
}
