//! Local SP1 execution boundary for the Raziel authorization guest.

#![forbid(unsafe_code)]

#[cfg(not(feature = "verification"))]
compile_error!("raziel-prover requires the `verification` or `proving` feature");

mod artifact;
#[cfg(feature = "proving")]
mod dump_guard;
mod error;
#[cfg(feature = "proving")]
mod execute;
#[cfg(feature = "proving")]
mod input;
mod key;
#[cfg(feature = "proving")]
mod plonk_artifacts;
#[cfg(feature = "proving")]
mod profile;
#[cfg(feature = "proving")]
mod prove;
mod verify;
mod version;

pub use artifact::MAX_PROOF_ARTIFACT_BYTES;
#[cfg(feature = "proving")]
pub use artifact::{decode_proof_artifact, encode_proof_artifact};
#[cfg(feature = "proving")]
pub use dump_guard::{FORBIDDEN_SP1_ENV_VARS, require_safe_sp1_environment};
pub use error::ProverError;
#[cfg(feature = "proving")]
pub use execute::{MAX_EXECUTION_CYCLE_LIMIT, Sp1ExecuteReport, Sp1Executor};
#[cfg(feature = "proving")]
pub use input::{PreparedGuestInput, prepare_guest_input};
pub use key::TrustedProgramKey;
#[cfg(feature = "proving")]
pub use key::verification_key_id;
#[cfg(feature = "proving")]
pub use plonk_artifacts::{SP1_PLONK_CIRCUIT_PATH_ENV, require_local_plonk_artifacts};
#[cfg(feature = "proving")]
pub use profile::{
    LOCAL_PROVER_ELEMENT_THRESHOLD, LOCAL_PROVER_GAS_TRACE_CHUNK_SLOTS,
    LOCAL_PROVER_HEIGHT_THRESHOLD, LOCAL_PROVER_MEMORY_LIMIT, LOCAL_PROVER_SHARD_SIZE,
    LOCAL_PROVER_TRACE_CHUNK_SLOTS, local_core_options,
};
#[cfg(feature = "proving")]
pub use prove::{Sp1ProofReport, Sp1Prover};
pub use verify::{Sp1VerificationReport, Sp1Verifier};
pub use version::SP1_SDK_VERSION;
