use std::convert::Infallible;
use std::sync::Arc;
use std::task::{Context, Poll};

use prost::bytes::Buf;
use raziel_proto::raziel::v1::verification_service_server::VerificationService;
use raziel_proto::raziel::v1::{VerifyCapabilityRequest, VerifyCapabilityResponse};
use tonic::codec::{Codec, DecodeBuf, Decoder};
use tonic::codegen::{Body, BoxFuture, Service, StdError, http};
use tonic::server::{Grpc, NamedService, UnaryService};
use tonic::{Request, Response, Status};
use tonic_prost::ProstEncoder;

use crate::admission::{
    MAX_VERIFICATION_SERVICE_MESSAGE_BYTES, decode_verify_capability_request_strict,
};

#[derive(Clone, Copy, Debug, Default)]
struct StrictVerificationCodec;

impl Codec for StrictVerificationCodec {
    type Encode = VerifyCapabilityResponse;
    type Decode = VerifyCapabilityRequest;
    type Encoder = ProstEncoder<VerifyCapabilityResponse>;
    type Decoder = StrictVerificationDecoder;

    fn encoder(&mut self) -> Self::Encoder {
        ProstEncoder::default()
    }

    fn decoder(&mut self) -> Self::Decoder {
        StrictVerificationDecoder
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct StrictVerificationDecoder;

impl Decoder for StrictVerificationDecoder {
    type Item = VerifyCapabilityRequest;
    type Error = Status;

    fn decode(&mut self, src: &mut DecodeBuf<'_>) -> Result<Option<Self::Item>, Self::Error> {
        if src.remaining() == 0 {
            return Ok(None);
        }
        if src.remaining() > MAX_VERIFICATION_SERVICE_MESSAGE_BYTES {
            return Err(Status::resource_exhausted(
                "verification request is too large",
            ));
        }
        let bytes = src.copy_to_bytes(src.remaining());
        decode_strict(&bytes).map(Some)
    }
}

fn decode_strict(bytes: &[u8]) -> Result<VerifyCapabilityRequest, Status> {
    decode_verify_capability_request_strict(bytes)
        .map_err(|error| Status::invalid_argument(error.to_string()))
}

#[derive(Debug)]
pub struct StrictVerificationServiceServer<T> {
    inner: Arc<T>,
}

impl<T> StrictVerificationServiceServer<T> {
    #[must_use]
    pub fn new(inner: T) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }
}

impl<T> Clone for StrictVerificationServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T, B> Service<http::Request<B>> for StrictVerificationServiceServer<T>
where
    T: VerificationService,
    B: Body + Send + 'static,
    B::Error: Into<StdError> + Send + 'static,
{
    type Response = http::Response<tonic::body::Body>;
    type Error = Infallible;
    type Future = BoxFuture<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: http::Request<B>) -> Self::Future {
        if request.uri().path() != "/raziel.v1.VerificationService/VerifyCapability" {
            return Box::pin(async move {
                let mut response = http::Response::new(tonic::body::Body::default());
                response.headers_mut().insert(
                    tonic::Status::GRPC_STATUS,
                    (tonic::Code::Unimplemented as i32).into(),
                );
                response.headers_mut().insert(
                    http::header::CONTENT_TYPE,
                    tonic::metadata::GRPC_CONTENT_TYPE,
                );
                Ok(response)
            });
        }

        struct VerifyCapabilityMethod<T: VerificationService>(Arc<T>);

        impl<T: VerificationService> UnaryService<VerifyCapabilityRequest> for VerifyCapabilityMethod<T> {
            type Response = VerifyCapabilityResponse;
            type Future = BoxFuture<Response<Self::Response>, Status>;

            fn call(&mut self, request: Request<VerifyCapabilityRequest>) -> Self::Future {
                let inner = Arc::clone(&self.0);
                Box::pin(async move { inner.verify_capability(request).await })
            }
        }

        let method = VerifyCapabilityMethod(Arc::clone(&self.inner));
        Box::pin(async move {
            let mut grpc = Grpc::new(StrictVerificationCodec).apply_max_message_size_config(
                Some(MAX_VERIFICATION_SERVICE_MESSAGE_BYTES),
                Some(MAX_VERIFICATION_SERVICE_MESSAGE_BYTES),
            );
            Ok(grpc.unary(method, request).await)
        })
    }
}

impl<T> NamedService for StrictVerificationServiceServer<T> {
    const NAME: &'static str = "raziel.v1.VerificationService";
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use prost::Message;
    use prost::bytes::BufMut;
    use raziel_proto::raziel::v1::verification_service_server::VerificationService;
    use tonic::codec::{EncodeBuf, Encoder};
    use tonic::transport::{Endpoint, Server};
    use tonic_prost::ProstDecoder;

    use super::*;

    #[test]
    fn decoder_rejects_unknown_wire_fields_before_the_handler() {
        let mut bytes = VerifyCapabilityRequest::default().encode_to_vec();
        bytes.extend_from_slice(&[0x48, 0x01]);
        assert_eq!(
            decode_strict(&bytes).unwrap_err().code(),
            tonic::Code::InvalidArgument
        );
    }

    #[tokio::test]
    async fn live_server_rejects_noncanonical_bytes_before_the_handler() {
        let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = probe.local_addr().unwrap();
        drop(probe);
        let calls = Arc::new(AtomicUsize::new(0));
        let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
        let server_calls = Arc::clone(&calls);
        let server = tokio::spawn(async move {
            Server::builder()
                .add_service(StrictVerificationServiceServer::new(CountingService {
                    calls: server_calls,
                }))
                .serve_with_shutdown(address, async move {
                    let _ = shutdown_receiver.await;
                })
                .await
        });

        let endpoint = Endpoint::from_shared(format!("http://{address}"))
            .unwrap()
            .connect_timeout(std::time::Duration::from_secs(2));
        let channel = loop {
            match endpoint.clone().connect().await {
                Ok(channel) => break channel,
                Err(_) => tokio::time::sleep(std::time::Duration::from_millis(10)).await,
            }
        };
        let mut client = tonic::client::Grpc::new(channel);
        client.ready().await.unwrap();
        let mut bytes = VerifyCapabilityRequest::default().encode_to_vec();
        bytes.extend_from_slice(&[0x48, 0x01]);
        let result = client
            .unary(
                Request::new(RawRequest(bytes)),
                http::uri::PathAndQuery::from_static(
                    "/raziel.v1.VerificationService/VerifyCapability",
                ),
                RawRequestCodec,
            )
            .await;
        assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        shutdown_sender.send(()).unwrap();
        server.await.unwrap().unwrap();
    }

    struct CountingService {
        calls: Arc<AtomicUsize>,
    }

    #[tonic::async_trait]
    impl VerificationService for CountingService {
        async fn verify_capability(
            &self,
            _request: Request<VerifyCapabilityRequest>,
        ) -> Result<Response<VerifyCapabilityResponse>, Status> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(Response::new(VerifyCapabilityResponse::default()))
        }
    }

    struct RawRequest(Vec<u8>);

    #[derive(Clone, Copy, Debug, Default)]
    struct RawRequestCodec;

    impl Codec for RawRequestCodec {
        type Encode = RawRequest;
        type Decode = VerifyCapabilityResponse;
        type Encoder = RawRequestEncoder;
        type Decoder = ProstDecoder<VerifyCapabilityResponse>;

        fn encoder(&mut self) -> Self::Encoder {
            RawRequestEncoder
        }

        fn decoder(&mut self) -> Self::Decoder {
            ProstDecoder::default()
        }
    }

    #[derive(Clone, Copy, Debug, Default)]
    struct RawRequestEncoder;

    impl Encoder for RawRequestEncoder {
        type Item = RawRequest;
        type Error = Status;

        fn encode(
            &mut self,
            item: Self::Item,
            destination: &mut EncodeBuf<'_>,
        ) -> Result<(), Self::Error> {
            destination.put_slice(&item.0);
            Ok(())
        }
    }
}
