use sp1_sdk::{
    Elf, StatusCode,
    blocking::{LightProver, Prover, ProverClient, SP1Stdin},
};

use crate::{PreparedGuestInput, ProverError};

pub const MAX_EXECUTION_CYCLE_LIMIT: u64 = 100_000_000;

pub struct Sp1Executor {
    prover: LightProver,
    cycle_limit: u64,
}

impl Sp1Executor {
    #[must_use]
    pub fn local() -> Self {
        Self {
            prover: ProverClient::builder().light().build(),
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

    pub fn execute(
        &self,
        guest_elf: &[u8],
        input: &PreparedGuestInput,
    ) -> Result<Sp1ExecuteReport, ProverError> {
        let mut stdin = SP1Stdin::new();
        stdin.write(input.input());
        let (public_values, report) = self
            .prover
            .execute(Elf::from(guest_elf), stdin)
            .cycle_limit(self.cycle_limit)
            .calculate_gas(true)
            .expected_exit_code(StatusCode::SUCCESS)
            .run()
            .map_err(|_| ProverError::GuestExecutionFailed)?;

        validate_execution_result(
            report.exit_code,
            public_values.as_slice(),
            input.expected_public_values_bytes(),
        )?;

        Ok(Sp1ExecuteReport {
            public_values: public_values.to_vec(),
            instruction_count: report.total_instruction_count(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sp1ExecuteReport {
    pub public_values: Vec<u8>,
    pub instruction_count: u64,
}

fn validate_execution_result(
    exit_code: u64,
    public_values: &[u8],
    expected_public_values: &[u8],
) -> Result<(), ProverError> {
    if exit_code != u64::from(StatusCode::SUCCESS.as_u32()) {
        return Err(ProverError::GuestExecutionFailed);
    }
    if public_values != expected_public_values {
        return Err(ProverError::PublicValuesMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_result_rejects_non_success_before_journal_comparison() {
        assert_eq!(
            validate_execution_result(
                u64::from(StatusCode::PANIC.as_u32()),
                b"expected",
                b"expected",
            ),
            Err(ProverError::GuestExecutionFailed)
        );
    }

    #[test]
    fn execution_result_rejects_every_public_journal_shape_mismatch() {
        let expected = b"expected";
        for public_values in [
            b"Expected".as_slice(),
            b"expecte".as_slice(),
            b"expected-extra".as_slice(),
        ] {
            assert_eq!(
                validate_execution_result(
                    u64::from(StatusCode::SUCCESS.as_u32()),
                    public_values,
                    expected,
                ),
                Err(ProverError::PublicValuesMismatch)
            );
        }
    }
}
