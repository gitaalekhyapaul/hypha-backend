//! Local SP1 execution boundary for the Raziel authorization guest.

#![forbid(unsafe_code)]

mod artifact;
mod dump_guard;
mod error;
mod execute;
mod input;
mod key;
mod plonk_artifacts;
mod profile;
mod prove;
mod verify;

pub use artifact::{
    MAX_PROOF_ARTIFACT_BYTES, SP1_SDK_VERSION, decode_proof_artifact, encode_proof_artifact,
};
pub use dump_guard::{FORBIDDEN_SP1_ENV_VARS, require_safe_sp1_environment};
pub use error::ProverError;
pub use execute::{MAX_EXECUTION_CYCLE_LIMIT, Sp1ExecuteReport, Sp1Executor};
pub use input::{PreparedGuestInput, prepare_guest_input};
pub use key::verification_key_id;
pub use plonk_artifacts::{SP1_PLONK_CIRCUIT_PATH_ENV, require_local_plonk_artifacts};
pub use profile::{
    LOCAL_PROVER_ELEMENT_THRESHOLD, LOCAL_PROVER_GAS_TRACE_CHUNK_SLOTS,
    LOCAL_PROVER_HEIGHT_THRESHOLD, LOCAL_PROVER_MEMORY_LIMIT, LOCAL_PROVER_SHARD_SIZE,
    LOCAL_PROVER_TRACE_CHUNK_SLOTS, local_core_options,
};
pub use prove::{Sp1ProofReport, Sp1Prover};
pub use verify::{Sp1VerificationReport, Sp1Verifier};
