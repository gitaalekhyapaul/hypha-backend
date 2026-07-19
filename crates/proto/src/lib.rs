//! Generated Protobuf contracts shared by Raziel Rust components.
//!
//! Run `pnpm generate` before compiling this crate. Generated Protobuf bytes are
//! transport artifacts, not canonical commitment inputs.

#![forbid(unsafe_code)]

pub mod raziel {
    pub mod v1 {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../gen/rust/raziel/v1/raziel.v1.rs"
        ));
    }
}
