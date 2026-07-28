use std::sync::{Mutex, MutexGuard};

use sp1_sdk::{
    Elf, ProvingKey, StatusCode,
    blocking::{CpuProver, ProveRequest, Prover, ProverClient, SP1Stdin},
};

use crate::{
    MAX_EXECUTION_CYCLE_LIMIT, PreparedGuestInput, ProverError, encode_proof_artifact,
    local_core_options, require_local_plonk_artifacts, require_safe_sp1_environment,
    verification_key_id,
};

static PROOF_JOB: Mutex<()> = Mutex::new(());

pub struct Sp1Prover {
    cycle_limit: u64,
}

impl Sp1Prover {
    #[must_use]
    pub const fn local() -> Self {
        Self {
            cycle_limit: MAX_EXECUTION_CYCLE_LIMIT,
        }
    }

    pub fn with_cycle_limit(mut self, cycle_limit: u64) -> Result<Self, ProverError> {
        if cycle_limit == 0 || cycle_limit > MAX_EXECUTION_CYCLE_LIMIT {
            return Err(ProverError::InvalidCycleLimit);
        }
        self.cycle_limit = cycle_limit;
        Ok(self)
    }

    pub fn prove(
        &self,
        guest_elf: &[u8],
        input: &PreparedGuestInput,
    ) -> Result<Sp1ProofReport, ProverError> {
        require_safe_sp1_environment()?;
        let _proof_job = acquire_proof_job()?;
        require_local_plonk_artifacts()?;

        let prover: CpuProver = ProverClient::builder()
            .cpu()
            .core_opts(local_core_options())
            .build();
        let proving_key = prover
            .setup(Elf::from(guest_elf))
            .map_err(|_| ProverError::ProverSetupFailed)?;
        let verification_key_id = verification_key_id(proving_key.verifying_key())?;

        let mut stdin = SP1Stdin::new();
        stdin.write(input.input());
        let proof = prover
            .prove(&proving_key, stdin)
            .plonk()
            .cycle_limit(self.cycle_limit)
            .expected_exit_code(StatusCode::SUCCESS)
            .deferred_proof_verification(true)
            .with_proof_nonce(proof_nonce(
                &input.expected_public_values().issuance_nullifier,
            ))
            .run()
            .map_err(|_| ProverError::ProofGenerationFailed)?;

        if proof.public_values.as_slice() != input.expected_public_values_bytes() {
            return Err(ProverError::PublicValuesMismatch);
        }
        let artifact = encode_proof_artifact(&proof)?;

        Ok(Sp1ProofReport {
            artifact,
            verification_key_id,
            public_values: proof.public_values.to_vec(),
        })
    }
}

fn acquire_proof_job() -> Result<MutexGuard<'static, ()>, ProverError> {
    PROOF_JOB
        .try_lock()
        .map_err(|_| ProverError::ProofJobUnavailable)
}

impl Default for Sp1Prover {
    fn default() -> Self {
        Self::local()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sp1ProofReport {
    pub artifact: Vec<u8>,
    pub verification_key_id: String,
    pub public_values: Vec<u8>,
}

fn proof_nonce(issuance_nullifier: &[u8; 32]) -> [u32; 4] {
    const KOALA_BEAR_MODULUS: u32 = 0x7f00_0001;

    std::array::from_fn(|index| {
        let start = index * 4;
        u32::from_be_bytes(
            issuance_nullifier[start..start + 4]
                .try_into()
                .expect("four-byte proof nonce chunk"),
        ) % KOALA_BEAR_MODULUS
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_nonce_uses_the_first_sixteen_nullifier_bytes_in_big_endian_order() {
        let nullifier = std::array::from_fn(|index| index as u8);
        assert_eq!(
            proof_nonce(&nullifier),
            [0x0001_0203, 0x0405_0607, 0x0809_0a0b, 0x0c0d_0e0f]
        );
    }

    #[test]
    fn proof_nonce_reduces_each_word_into_the_koala_bear_field() {
        assert_eq!(proof_nonce(&[u8::MAX; 32]), [u32::MAX % 0x7f00_0001; 4]);
    }

    #[test]
    fn a_second_local_proof_job_fails_closed() {
        let first = acquire_proof_job().unwrap();
        assert!(matches!(
            acquire_proof_job(),
            Err(ProverError::ProofJobUnavailable)
        ));
        drop(first);
        assert!(acquire_proof_job().is_ok());
    }
}
