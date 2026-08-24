use std::fs::File;
use std::io::Read;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Instant;

use raziel_prover::{
    ProverError, Sp1Executor, Sp1Prover, prepare_guest_input, require_local_plonk_artifacts,
    require_safe_sp1_environment,
};
use raziel_verifier::{LocalRegistry, RegistryLoader, default_registry_config};

use super::config::{CGROUP_ROOT, MAX_GUEST_ELF_BYTES};
use super::{
    MAX_ONE_SHOT_REQUEST_BYTES, MAX_ONE_SHOT_RESULT_BYTES, OneShotFailure, OneShotFailureClass,
    OneShotMetrics, OneShotProtocolError, OneShotProverConfig, OneShotRequest, OneShotResult,
    OneShotSuccess, PROOF_MEMORY_LIMIT_BYTES, decode_request, encode_result, read_frame_to_eof,
    write_frame,
};
use crate::input_assembly::{InputAssemblyError, assemble_authorization_input};
use crate::private_snapshot::PrivateSnapshotStore;

/// Runs one fixed-profile child request and exits after one response.
pub fn run_child_from_env() -> Result<(), OneShotProtocolError> {
    let config = OneShotProverConfig::from_env()?;
    validate_runtime_profile(Path::new(CGROUP_ROOT))?;
    validate_owner_files(&config)?;
    require_safe_sp1_environment().map_err(|_| OneShotProtocolError::ResourceProfile)?;
    require_local_plonk_artifacts().map_err(|_| OneShotProtocolError::ResourceProfile)?;
    let guest_elf = read_guest_elf(config.guest_elf_path())?;

    let mut stream =
        UnixStream::connect(config.socket_path()).map_err(|_| OneShotProtocolError::FrameRead)?;
    let request_bytes = read_frame_to_eof(&mut stream, MAX_ONE_SHOT_REQUEST_BYTES)?;
    let request = decode_request(&request_bytes)?;
    let correlation_id = request.correlation_id().to_owned();
    let result = match execute_request(&config, request, &guest_elf) {
        Ok(success) => OneShotResult::Success(Box::new(success)),
        Err(class) => OneShotResult::Failure(
            OneShotFailure::new(class, correlation_id)
                .map_err(|_| OneShotProtocolError::InvalidResult)?,
        ),
    };
    let result_bytes = encode_result(&result)?;
    write_frame(&mut stream, &result_bytes, MAX_ONE_SHOT_RESULT_BYTES)
}

fn execute_request(
    config: &OneShotProverConfig,
    request: OneShotRequest,
    guest_elf: &[u8],
) -> Result<OneShotSuccess, OneShotFailureClass> {
    let registry_config = default_registry_config(config.trust_domain_id())
        .map_err(|_| OneShotFailureClass::TrustedState)?;
    let generation = RegistryLoader::new(registry_config.clone())
        .load_current(config.registry_path(), request.worker_start_unix_seconds())
        .map_err(|_| OneShotFailureClass::TrustedState)?;
    let mut registry = LocalRegistry::new(registry_config);
    registry
        .install_generation(generation)
        .map_err(|_| OneShotFailureClass::TrustedState)?;

    let resolved = registry
        .resolve_snapshot_exact(
            &request.envelope().request().snapshot_id,
            request.worker_start_unix_seconds(),
        )
        .map_err(|_| OneShotFailureClass::TrustedState)?;
    let root_claims = resolved.root_record.claims.claims();
    let snapshot = PrivateSnapshotStore::open(config.private_snapshot_root())
        .and_then(|store| {
            store.load(
                &resolved.policy_record.policy,
                &root_claims.snapshot_id,
                &root_claims.relationship_snapshot_root,
            )
        })
        .map_err(|_| OneShotFailureClass::TrustedState)?;
    let validated = assemble_authorization_input(
        &mut registry,
        request.envelope(),
        snapshot.into_selected(),
        request.worker_start_unix_seconds(),
    )
    .map_err(map_assembly_failure)?;
    let expected_public_values = validated.expected_public_values_bytes().to_vec();
    let (guest_input, _) = validated.into_parts();
    let prepared = prepare_guest_input(guest_input).map_err(map_prover_failure)?;
    if prepared.expected_public_values_bytes() != expected_public_values {
        return Err(OneShotFailureClass::JournalMismatch);
    }

    let execute_started = Instant::now();
    let execution = Sp1Executor::local()
        .execute(guest_elf, &prepared)
        .map_err(map_prover_failure)?;
    let execute_millis = elapsed_millis(execute_started)?;
    if execution.public_values != expected_public_values {
        return Err(OneShotFailureClass::JournalMismatch);
    }

    let prove_started = Instant::now();
    let proof = Sp1Prover::local()
        .prove(guest_elf, &prepared)
        .map_err(map_prover_failure)?;
    let prove_millis = elapsed_millis(prove_started)?;
    if proof.public_values != expected_public_values
        || proof.verification_key_id != config.expected_verification_key_id()
    {
        return Err(OneShotFailureClass::JournalMismatch);
    }
    let artifact_bytes =
        u64::try_from(proof.artifact.len()).map_err(|_| OneShotFailureClass::Internal)?;

    let (peak_memory_bytes, peak_swap_bytes) = read_cgroup_peaks(Path::new(CGROUP_ROOT))
        .map_err(|_| OneShotFailureClass::ResourceProfile)?;
    OneShotSuccess::new(
        proof.artifact,
        proof.public_values,
        proof.verification_key_id,
        OneShotMetrics {
            execute_millis,
            prove_millis,
            artifact_bytes,
            peak_memory_bytes,
            peak_swap_bytes,
        },
    )
    .map_err(|_| OneShotFailureClass::Internal)
}

fn elapsed_millis(started: Instant) -> Result<u64, OneShotFailureClass> {
    u64::try_from(started.elapsed().as_millis()).map_err(|_| OneShotFailureClass::Internal)
}

fn map_assembly_failure(error: InputAssemblyError) -> OneShotFailureClass {
    match error {
        InputAssemblyError::Witness(_) => OneShotFailureClass::WitnessRejected,
        InputAssemblyError::HostValidation(_)
        | InputAssemblyError::RequestMapping(_)
        | InputAssemblyError::EnvelopeCommitmentMismatch => OneShotFailureClass::HostRejected,
        InputAssemblyError::InvalidWorkerStart | InputAssemblyError::InsufficientValidityWindow => {
            OneShotFailureClass::InvalidRequest
        }
        InputAssemblyError::Registry(_)
        | InputAssemblyError::TrustedSelectionMismatch
        | InputAssemblyError::SnapshotSelectionMismatch => OneShotFailureClass::TrustedState,
    }
}

fn map_prover_failure(error: ProverError) -> OneShotFailureClass {
    match error {
        ProverError::PublicValuesMismatch => OneShotFailureClass::JournalMismatch,
        ProverError::GuestExecutionFailed | ProverError::InvalidInput(_) => {
            OneShotFailureClass::HostRejected
        }
        ProverError::UnsafeSp1Environment | ProverError::PlonkArtifactsUnavailable => {
            OneShotFailureClass::ResourceProfile
        }
        _ => OneShotFailureClass::ProofFailed,
    }
}

fn validate_owner_files(config: &OneShotProverConfig) -> Result<(), OneShotProtocolError> {
    validate_regular_file(config.guest_elf_path(), MAX_GUEST_ELF_BYTES)?;
    validate_directory(config.registry_path())?;
    let snapshot_store = PrivateSnapshotStore::open(config.private_snapshot_root())
        .map_err(|_| OneShotProtocolError::Configuration)?;
    drop(snapshot_store);
    validate_directory(config.plonk_setup_path())?;
    let snapshot_owner = std::fs::symlink_metadata(config.private_snapshot_root())
        .map_err(|_| OneShotProtocolError::Configuration)?
        .uid();
    validate_socket(config.socket_path(), snapshot_owner)
}

fn validate_socket(path: &Path, expected_owner_uid: u32) -> Result<(), OneShotProtocolError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| OneShotProtocolError::Configuration)?;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_socket()
        || metadata.permissions().mode() & 0o7777 != 0o600
        || metadata.uid() != expected_owner_uid
        || metadata.nlink() != 1
    {
        return Err(OneShotProtocolError::Configuration);
    }
    Ok(())
}

fn validate_directory(path: &Path) -> Result<(), OneShotProtocolError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| OneShotProtocolError::Configuration)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(OneShotProtocolError::Configuration);
    }
    Ok(())
}

fn validate_regular_file(path: &Path, maximum: u64) -> Result<u64, OneShotProtocolError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| OneShotProtocolError::Configuration)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > maximum
        || metadata.permissions().mode() & 0o022 != 0
        || metadata.nlink() != 1
    {
        return Err(OneShotProtocolError::Configuration);
    }
    Ok(metadata.len())
}

fn read_guest_elf(path: &Path) -> Result<Vec<u8>, OneShotProtocolError> {
    let expected_length = validate_regular_file(path, MAX_GUEST_ELF_BYTES)?;
    let mut bytes = Vec::with_capacity(
        usize::try_from(expected_length).map_err(|_| OneShotProtocolError::Configuration)?,
    );
    File::open(path)
        .map_err(|_| OneShotProtocolError::Configuration)?
        .take(MAX_GUEST_ELF_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| OneShotProtocolError::Configuration)?;
    if bytes.len() as u64 != expected_length {
        return Err(OneShotProtocolError::Configuration);
    }
    Ok(bytes)
}

fn validate_runtime_profile(cgroup_root: &Path) -> Result<(), OneShotProtocolError> {
    validate_cgroup(cgroup_root)?;
    if !matches!(std::thread::available_parallelism().map(usize::from), Ok(8))
        || std::env::var("RAYON_NUM_THREADS").as_deref() != Ok("8")
        || std::env::var("GOMAXPROCS").as_deref() != Ok("8")
        || std::env::var("GOMEMLIMIT").as_deref() != Ok("24GiB")
    {
        return Err(OneShotProtocolError::ResourceProfile);
    }
    Ok(())
}

pub(super) fn validate_cgroup(cgroup_root: &Path) -> Result<(), OneShotProtocolError> {
    if read_cgroup_number(cgroup_root, "memory.max")? != PROOF_MEMORY_LIMIT_BYTES
        || read_cgroup_number(cgroup_root, "memory.swap.max")? != 0
    {
        return Err(OneShotProtocolError::ResourceProfile);
    }
    Ok(())
}

fn read_cgroup_peaks(cgroup_root: &Path) -> Result<(u64, u64), OneShotProtocolError> {
    Ok((
        read_cgroup_number(cgroup_root, "memory.peak")?,
        read_cgroup_number(cgroup_root, "memory.swap.peak")?,
    ))
}

fn read_cgroup_number(cgroup_root: &Path, file_name: &str) -> Result<u64, OneShotProtocolError> {
    let value = std::fs::read_to_string(cgroup_root.join(file_name))
        .map_err(|_| OneShotProtocolError::ResourceProfile)?;
    let trimmed = value.trim();
    if trimmed.is_empty()
        || !trimmed.bytes().all(|byte| byte.is_ascii_digit())
        || (trimmed.len() > 1 && trimmed.starts_with('0'))
    {
        return Err(OneShotProtocolError::ResourceProfile);
    }
    trimmed
        .parse()
        .map_err(|_| OneShotProtocolError::ResourceProfile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cgroup_profile_requires_the_exact_memory_limit_and_zero_swap() {
        let root =
            std::env::temp_dir().join(format!("razield-one-shot-cgroup-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("memory.max"),
            format!("{PROOF_MEMORY_LIMIT_BYTES}\n"),
        )
        .unwrap();
        std::fs::write(root.join("memory.swap.max"), "0\n").unwrap();
        assert_eq!(validate_cgroup(&root), Ok(()));

        std::fs::write(root.join("memory.max"), "max\n").unwrap();
        assert_eq!(
            validate_cgroup(&root),
            Err(OneShotProtocolError::ResourceProfile)
        );
        std::fs::write(
            root.join("memory.max"),
            format!("{PROOF_MEMORY_LIMIT_BYTES}\n"),
        )
        .unwrap();
        std::fs::write(root.join("memory.swap.max"), "1\n").unwrap();
        assert_eq!(
            validate_cgroup(&root),
            Err(OneShotProtocolError::ResourceProfile)
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
