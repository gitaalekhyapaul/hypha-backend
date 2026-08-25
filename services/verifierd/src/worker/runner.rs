use std::fs;
use std::os::unix::net::UnixStream;
use std::path::Path;

use raziel_prover::Sp1Verifier;

use super::{
    MAX_VERIFICATION_WORKER_REQUEST_BYTES, MAX_VERIFICATION_WORKER_RESULT_BYTES,
    VERIFICATION_MEMORY_LIMIT_BYTES, VerificationWorkerFailure, VerificationWorkerFailureClass,
    VerificationWorkerProtocolError, VerificationWorkerResult, VerificationWorkerSuccess,
    decode_request, encode_result, read_frame_to_eof, write_frame,
};

const CHILD_SOCKET_ENV: &str = "RAZIEL_VERIFIER_ONE_SHOT_SOCKET_PATH";
const CGROUP_ROOT: &str = "/sys/fs/cgroup";

pub fn run_child_from_env() -> Result<(), VerificationWorkerProtocolError> {
    let socket_path = std::env::var_os(CHILD_SOCKET_ENV)
        .filter(|value| !value.is_empty())
        .ok_or(VerificationWorkerProtocolError::Configuration)?;
    let mut stream =
        UnixStream::connect(socket_path).map_err(|_| VerificationWorkerProtocolError::FrameRead)?;
    let request_bytes = read_frame_to_eof(&mut stream, MAX_VERIFICATION_WORKER_REQUEST_BYTES)?;
    let request = decode_request(&request_bytes)?;
    let correlation_id = request.correlation_id().to_owned();

    let result = if validate_runtime_profile(Path::new(CGROUP_ROOT)).is_err() {
        VerificationWorkerResult::Failure(VerificationWorkerFailure::new(
            VerificationWorkerFailureClass::ResourceProfile,
            correlation_id,
        )?)
    } else {
        let verifier = Sp1Verifier::from_trusted_key(request.trusted_program_key().clone());
        match verifier.verify(
            request.proof_artifact(),
            request.trusted_program_key().verification_key_id(),
            request.expected_public_values(),
        ) {
            Ok(report)
                if report.public_values == request.expected_public_values()
                    && report.verification_key_id
                        == request.trusted_program_key().verification_key_id() =>
            {
                VerificationWorkerResult::Success(VerificationWorkerSuccess::new(
                    correlation_id,
                    report.verification_key_id,
                )?)
            }
            Ok(_) | Err(_) => VerificationWorkerResult::Failure(VerificationWorkerFailure::new(
                VerificationWorkerFailureClass::ProofRejected,
                correlation_id,
            )?),
        }
    };
    let response = encode_result(&result)?;
    write_frame(&mut stream, &response, MAX_VERIFICATION_WORKER_RESULT_BYTES)
}

fn validate_runtime_profile(root: &Path) -> Result<(), VerificationWorkerProtocolError> {
    let memory = read_limit(&root.join("memory.max"))?;
    let swap = read_limit(&root.join("memory.swap.max"))?;
    if memory != VERIFICATION_MEMORY_LIMIT_BYTES || swap != 0 {
        return Err(VerificationWorkerProtocolError::ResourceProfile);
    }
    Ok(())
}

fn read_limit(path: &Path) -> Result<u64, VerificationWorkerProtocolError> {
    fs::read_to_string(path)
        .map_err(|_| VerificationWorkerProtocolError::ResourceProfile)?
        .trim()
        .parse()
        .map_err(|_| VerificationWorkerProtocolError::ResourceProfile)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn exact_memory_and_zero_swap_are_required() {
        let root = std::env::temp_dir().join(format!(
            "rz-verifier-profile-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(
            root.join("memory.max"),
            VERIFICATION_MEMORY_LIMIT_BYTES.to_string(),
        )
        .unwrap();
        fs::write(root.join("memory.swap.max"), "0").unwrap();
        assert_eq!(validate_runtime_profile(&root), Ok(()));
        fs::write(root.join("memory.swap.max"), "1").unwrap();
        assert_eq!(
            validate_runtime_profile(&root),
            Err(VerificationWorkerProtocolError::ResourceProfile)
        );
        fs::remove_dir_all(root).unwrap();
    }
}
