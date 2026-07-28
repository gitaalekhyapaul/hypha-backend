use std::{
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use sp1_sdk::SP1_CIRCUIT_VERSION;

use crate::ProverError;

pub const SP1_PLONK_CIRCUIT_PATH_ENV: &str = "SP1_PLONK_CIRCUIT_PATH";

const PINNED_ARTIFACT_MANIFEST: &str = include_str!("../../../tools/sp1-plonk-artifact.env");
const PINNED_FILE_HASHES: &str = include_str!("../../../tools/sp1-plonk-files.sha256");
const PINNED_FILE_SIZES: &str = include_str!("../../../tools/sp1-plonk-files.size");
const INSTALLED_ARTIFACT_MANIFEST: &str = "RAZIEL-ARTIFACT-MANIFEST";
const INSTALLED_FILE_HASHES: &str = "RAZIEL-FILE-SHA256";
const INSTALLED_FILE_SIZES: &str = "RAZIEL-FILE-SIZE";
const REQUIRED_PLONK_FILES: &[&str] = &[
    "constraints.json",
    "plonk_circuit.bin",
    "plonk_pk.bin",
    "plonk_vk.bin",
];

pub fn require_local_plonk_artifacts() -> Result<(), ProverError> {
    let base = std::env::var_os(SP1_PLONK_CIRCUIT_PATH_ENV)
        .map(PathBuf::from)
        .ok_or(ProverError::PlonkArtifactsUnavailable)?;
    validate_local_plonk_artifacts(&base)
}

fn validate_local_plonk_artifacts(base: &Path) -> Result<(), ProverError> {
    validate_local_plonk_artifacts_against(
        base,
        PINNED_ARTIFACT_MANIFEST,
        PINNED_FILE_HASHES,
        PINNED_FILE_SIZES,
    )
}

fn validate_local_plonk_artifacts_against(
    base: &Path,
    pinned_manifest: &str,
    pinned_file_hashes: &str,
    pinned_file_sizes: &str,
) -> Result<(), ProverError> {
    if !base.is_absolute() {
        return Err(ProverError::PlonkArtifactsUnavailable);
    }

    let version_directory = base.join(SP1_CIRCUIT_VERSION);
    if has_symlink_component(&version_directory) || !is_real_directory(&version_directory) {
        return Err(ProverError::PlonkArtifactsUnavailable);
    }

    for file in REQUIRED_PLONK_FILES {
        if !is_real_file(&version_directory.join(file)) {
            return Err(ProverError::PlonkArtifactsUnavailable);
        }
    }

    let manifest = std::fs::read_to_string(version_directory.join(INSTALLED_ARTIFACT_MANIFEST))
        .map_err(|_| ProverError::PlonkArtifactsUnavailable)?;
    let installed_file_hashes =
        std::fs::read_to_string(version_directory.join(INSTALLED_FILE_HASHES))
            .map_err(|_| ProverError::PlonkArtifactsUnavailable)?;
    let installed_file_sizes =
        std::fs::read_to_string(version_directory.join(INSTALLED_FILE_SIZES))
            .map_err(|_| ProverError::PlonkArtifactsUnavailable)?;
    if manifest != pinned_manifest
        || installed_file_hashes != pinned_file_hashes
        || installed_file_sizes != pinned_file_sizes
    {
        return Err(ProverError::PlonkArtifactsUnavailable);
    }

    verify_files(&version_directory, pinned_file_hashes, pinned_file_sizes)?;
    Ok(())
}

fn verify_files(
    directory: &Path,
    pinned_file_hashes: &str,
    pinned_file_sizes: &str,
) -> Result<(), ProverError> {
    let mut hash_lines = pinned_file_hashes.lines();
    let mut size_lines = pinned_file_sizes.lines();
    for expected_file in REQUIRED_PLONK_FILES {
        let (expected_hash, hash_file_name) = hash_lines
            .next()
            .and_then(|line| line.split_once("  "))
            .ok_or(ProverError::PlonkArtifactsUnavailable)?;
        let (expected_size, size_file_name) = size_lines
            .next()
            .and_then(|line| line.split_once("  "))
            .ok_or(ProverError::PlonkArtifactsUnavailable)?;
        if hash_file_name != *expected_file || size_file_name != *expected_file {
            return Err(ProverError::PlonkArtifactsUnavailable);
        }

        let expected_hash = decode_sha256(expected_hash)?;
        let expected_size = expected_size
            .parse::<u64>()
            .map_err(|_| ProverError::PlonkArtifactsUnavailable)?;
        let file = File::open(directory.join(expected_file))
            .map_err(|_| ProverError::PlonkArtifactsUnavailable)?;
        if file
            .metadata()
            .map_err(|_| ProverError::PlonkArtifactsUnavailable)?
            .len()
            != expected_size
        {
            return Err(ProverError::PlonkArtifactsUnavailable);
        }
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let bytes_read = reader
                .read(&mut buffer)
                .map_err(|_| ProverError::PlonkArtifactsUnavailable)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }
        if <[u8; 32]>::from(hasher.finalize()) != expected_hash {
            return Err(ProverError::PlonkArtifactsUnavailable);
        }
    }
    if hash_lines.next().is_some() || size_lines.next().is_some() {
        return Err(ProverError::PlonkArtifactsUnavailable);
    }
    Ok(())
}

fn decode_sha256(value: &str) -> Result<[u8; 32], ProverError> {
    if value.len() != 64 {
        return Err(ProverError::PlonkArtifactsUnavailable);
    }

    let mut digest = [0_u8; 32];
    for (output, encoded) in digest.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
        let high = decode_lowercase_hex(encoded[0])?;
        let low = decode_lowercase_hex(encoded[1])?;
        *output = (high << 4) | low;
    }
    Ok(digest)
}

const fn decode_lowercase_hex(value: u8) -> Result<u8, ProverError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(ProverError::PlonkArtifactsUnavailable),
    }
}

fn is_real_directory(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.file_type().is_dir() && !metadata.file_type().is_symlink())
}

fn has_symlink_component(path: &Path) -> bool {
    let mut current = PathBuf::new();
    path.components().any(|component| {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => metadata.file_type().is_symlink(),
            Err(_) => true,
        }
    })
}

fn is_real_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| {
        metadata.file_type().is_file() && !metadata.file_type().is_symlink() && metadata.len() > 0
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;

    static NEXT_DIRECTORY_ID: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn pinned_artifact_directory_requires_all_files_and_exact_manifest() {
        let directory = test_directory();
        let version_directory = directory.join(SP1_CIRCUIT_VERSION);
        fs::create_dir_all(&version_directory).unwrap();
        let fixture_hashes = fixture_hashes();
        let fixture_sizes = fixture_sizes();

        assert_eq!(
            validate_local_plonk_artifacts_against(
                &directory,
                PINNED_ARTIFACT_MANIFEST,
                &fixture_hashes,
                &fixture_sizes,
            ),
            Err(ProverError::PlonkArtifactsUnavailable)
        );

        for file in REQUIRED_PLONK_FILES {
            fs::write(version_directory.join(file), b"fixture").unwrap();
        }
        fs::write(
            version_directory.join(INSTALLED_ARTIFACT_MANIFEST),
            PINNED_ARTIFACT_MANIFEST,
        )
        .unwrap();
        fs::write(
            version_directory.join(INSTALLED_FILE_HASHES),
            &fixture_hashes,
        )
        .unwrap();
        fs::write(version_directory.join(INSTALLED_FILE_SIZES), &fixture_sizes).unwrap();
        assert_eq!(
            validate_local_plonk_artifacts_against(
                &directory,
                PINNED_ARTIFACT_MANIFEST,
                &fixture_hashes,
                &fixture_sizes,
            ),
            Ok(())
        );

        fs::write(version_directory.join(REQUIRED_PLONK_FILES[0]), b"Fixture").unwrap();
        assert_eq!(
            validate_local_plonk_artifacts_against(
                &directory,
                PINNED_ARTIFACT_MANIFEST,
                &fixture_hashes,
                &fixture_sizes,
            ),
            Err(ProverError::PlonkArtifactsUnavailable)
        );
        fs::write(version_directory.join(REQUIRED_PLONK_FILES[0]), b"fixture!").unwrap();
        assert_eq!(
            validate_local_plonk_artifacts_against(
                &directory,
                PINNED_ARTIFACT_MANIFEST,
                &fixture_hashes,
                &fixture_sizes,
            ),
            Err(ProverError::PlonkArtifactsUnavailable)
        );
        fs::write(version_directory.join(REQUIRED_PLONK_FILES[0]), b"fixture").unwrap();

        fs::write(
            version_directory.join(INSTALLED_ARTIFACT_MANIFEST),
            b"wrong manifest",
        )
        .unwrap();
        assert_eq!(
            validate_local_plonk_artifacts_against(
                &directory,
                PINNED_ARTIFACT_MANIFEST,
                &fixture_hashes,
                &fixture_sizes,
            ),
            Err(ProverError::PlonkArtifactsUnavailable)
        );

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn pinned_artifact_directory_must_be_absolute() {
        assert_eq!(
            validate_local_plonk_artifacts(Path::new("relative")),
            Err(ProverError::PlonkArtifactsUnavailable)
        );
    }

    #[cfg(unix)]
    #[test]
    fn pinned_artifact_directory_rejects_linked_files() {
        use std::os::unix::fs::symlink;

        let directory = test_directory();
        let version_directory = directory.join(SP1_CIRCUIT_VERSION);
        fs::create_dir_all(&version_directory).unwrap();
        let fixture_hashes = fixture_hashes();
        let fixture_sizes = fixture_sizes();
        for file in REQUIRED_PLONK_FILES {
            fs::write(version_directory.join(file), b"fixture").unwrap();
        }
        fs::write(
            version_directory.join(INSTALLED_ARTIFACT_MANIFEST),
            PINNED_ARTIFACT_MANIFEST,
        )
        .unwrap();
        fs::write(
            version_directory.join(INSTALLED_FILE_HASHES),
            &fixture_hashes,
        )
        .unwrap();
        fs::write(version_directory.join(INSTALLED_FILE_SIZES), &fixture_sizes).unwrap();

        let linked_file = version_directory.join(REQUIRED_PLONK_FILES[0]);
        fs::remove_file(&linked_file).unwrap();
        symlink(version_directory.join(REQUIRED_PLONK_FILES[1]), linked_file).unwrap();
        assert_eq!(
            validate_local_plonk_artifacts_against(
                &directory,
                PINNED_ARTIFACT_MANIFEST,
                &fixture_hashes,
                &fixture_sizes,
            ),
            Err(ProverError::PlonkArtifactsUnavailable)
        );

        fs::remove_dir_all(directory).unwrap();
    }

    fn test_directory() -> PathBuf {
        std::env::temp_dir().canonicalize().unwrap().join(format!(
            "raziel-plonk-artifacts-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn fixture_hashes() -> String {
        let digest = Sha256::digest(b"fixture");
        let encoded = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        REQUIRED_PLONK_FILES
            .iter()
            .map(|file| format!("{encoded}  {file}\n"))
            .collect()
    }

    fn fixture_sizes() -> String {
        REQUIRED_PLONK_FILES
            .iter()
            .map(|file| format!("7  {file}\n"))
            .collect()
    }
}
