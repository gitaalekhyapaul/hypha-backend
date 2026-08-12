//! Generated Protobuf contracts shared by Raziel Rust components.
//!
//! Run `pnpm generate` before compiling this crate. Generated Protobuf bytes are
//! transport artifacts, not canonical commitment inputs.

#![forbid(unsafe_code)]
#![cfg_attr(feature = "services", allow(clippy::empty_docs))]

pub mod raziel {
    pub mod v1 {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../gen/rust/raziel/v1/raziel.v1.rs"
        ));

        #[cfg(feature = "services")]
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../gen/rust-services/raziel/v1/raziel.v1.tonic.rs"
        ));
    }
}

#[cfg(all(test, feature = "services"))]
mod tests {
    use std::any::TypeId;

    use super::raziel::v1::{
        capability_service_client::CapabilityServiceClient,
        capability_service_server::CapabilityServiceServer,
        snapshot_service_client::SnapshotServiceClient,
        snapshot_service_server::SnapshotServiceServer,
        verification_service_client::VerificationServiceClient,
        verification_service_server::VerificationServiceServer,
    };

    struct ContractOnly;

    #[test]
    fn generated_service_client_and_server_types_compile() {
        let exported_types = [
            TypeId::of::<SnapshotServiceClient<tonic::transport::Channel>>(),
            TypeId::of::<SnapshotServiceServer<ContractOnly>>(),
            TypeId::of::<CapabilityServiceClient<tonic::transport::Channel>>(),
            TypeId::of::<CapabilityServiceServer<ContractOnly>>(),
            TypeId::of::<VerificationServiceClient<tonic::transport::Channel>>(),
            TypeId::of::<VerificationServiceServer<ContractOnly>>(),
        ];

        assert_eq!(exported_types.len(), 6);
    }
}
