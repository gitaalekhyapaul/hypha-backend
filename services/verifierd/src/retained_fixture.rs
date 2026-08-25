use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Debug, Formatter};
use std::fs;
use std::path::Path;

use raziel_authorization_core::{AuthorizationPublicValuesV1, decode_public_values};
use raziel_prover::{MAX_PROOF_ARTIFACT_BYTES, TrustedProgramKey};
use sha2::{Digest, Sha256};

const PAYLOAD_FILES: [&str; 4] = [
    "plonk-proof.bin",
    "public-values.bin",
    "sp1-vkey-hash.txt",
    "verification-key-id.txt",
];
const ALL_FILES: [&str; 6] = [
    "PROVENANCE",
    "SHA256SUMS",
    "plonk-proof.bin",
    "public-values.bin",
    "sp1-vkey-hash.txt",
    "verification-key-id.txt",
];
const PROVENANCE_FIELDS: [&str; 22] = [
    "format_version",
    "classification",
    "fixture_id",
    "proof_system",
    "source_git_commit",
    "source_git_tree",
    "source_manifest_sha256",
    "run_bundle_manifest_sha256",
    "toolchain_image_id",
    "benchmark_profile",
    "proof_generated_utc",
    "prove_ms",
    "baseline_verify_ms",
    "proof_artifact_bytes",
    "proof_artifact_sha256",
    "public_values_bytes",
    "public_values_sha256",
    "verification_key_id",
    "sp1_vkey_hash",
    "proof_memory_peak_bytes",
    "proof_swap_peak_bytes",
    "private_witness_retained",
];
const DEBUG_TRACE_FIELD: &str = "debug_trace_retained";
const MAX_PUBLIC_VALUES_BYTES: usize = 4 * 1024;

pub(crate) struct RetainedPlonkFixture {
    proof_artifact: Vec<u8>,
    public_values: AuthorizationPublicValuesV1,
    trusted_program_key: TrustedProgramKey,
    proof_sha256: [u8; 32],
}

impl RetainedPlonkFixture {
    pub(crate) fn load(path: &Path) -> Result<Self, RetainedFixtureError> {
        require_exact_files(path)?;
        let proof_artifact =
            read_bounded_file(&path.join("plonk-proof.bin"), 1, MAX_PROOF_ARTIFACT_BYTES)?;
        let public_values_bytes =
            read_bounded_file(&path.join("public-values.bin"), 1, MAX_PUBLIC_VALUES_BYTES)?;
        let public_values =
            decode_public_values(&public_values_bytes).map_err(|_| RetainedFixtureError)?;
        let verification_key_id = read_single_line_value(&path.join("verification-key-id.txt"))?;
        let sp1_vkey_hash = read_single_line_value(&path.join("sp1-vkey-hash.txt"))?;
        let trusted_program_key =
            TrustedProgramKey::from_registry_record(&verification_key_id, &sp1_vkey_hash)
                .map_err(|_| RetainedFixtureError)?;
        let checksums = parse_checksums(path)?;
        verify_payload_checksums(path, &checksums)?;
        let provenance = parse_provenance(path)?;
        verify_provenance(
            &provenance,
            &proof_artifact,
            &public_values_bytes,
            &trusted_program_key,
        )?;

        Ok(Self {
            proof_sha256: Sha256::digest(&proof_artifact).into(),
            proof_artifact,
            public_values,
            trusted_program_key,
        })
    }

    pub(crate) fn proof_artifact(&self) -> &[u8] {
        &self.proof_artifact
    }

    pub(crate) const fn public_values(&self) -> &AuthorizationPublicValuesV1 {
        &self.public_values
    }

    pub(crate) const fn trusted_program_key(&self) -> &TrustedProgramKey {
        &self.trusted_program_key
    }

    pub(crate) const fn proof_sha256(&self) -> &[u8; 32] {
        &self.proof_sha256
    }
}

impl Debug for RetainedPlonkFixture {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedPlonkFixture")
            .field("public_fixture", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RetainedFixtureError;

impl fmt::Display for RetainedFixtureError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("retained public proof fixture is invalid")
    }
}

impl Error for RetainedFixtureError {}

fn require_exact_files(path: &Path) -> Result<(), RetainedFixtureError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| RetainedFixtureError)?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(RetainedFixtureError);
    }
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(path).map_err(|_| RetainedFixtureError)? {
        let entry = entry.map_err(|_| RetainedFixtureError)?;
        let file_type = entry.file_type().map_err(|_| RetainedFixtureError)?;
        if !file_type.is_file() || file_type.is_symlink() {
            return Err(RetainedFixtureError);
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| RetainedFixtureError)?;
        if !names.insert(name) {
            return Err(RetainedFixtureError);
        }
    }
    if names != ALL_FILES.into_iter().map(str::to_owned).collect() {
        return Err(RetainedFixtureError);
    }
    Ok(())
}

fn read_bounded_file(
    path: &Path,
    minimum: usize,
    maximum: usize,
) -> Result<Vec<u8>, RetainedFixtureError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| RetainedFixtureError)?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(RetainedFixtureError);
    }
    let length = usize::try_from(metadata.len()).map_err(|_| RetainedFixtureError)?;
    if !(minimum..=maximum).contains(&length) {
        return Err(RetainedFixtureError);
    }
    let bytes = fs::read(path).map_err(|_| RetainedFixtureError)?;
    if bytes.len() != length {
        return Err(RetainedFixtureError);
    }
    Ok(bytes)
}

fn read_single_line_value(path: &Path) -> Result<String, RetainedFixtureError> {
    let bytes = read_bounded_file(path, 1, 128)?;
    if bytes.iter().any(|byte| matches!(byte, b'\n' | b'\r' | 0)) {
        return Err(RetainedFixtureError);
    }
    String::from_utf8(bytes).map_err(|_| RetainedFixtureError)
}

fn parse_checksums(path: &Path) -> Result<BTreeMap<String, String>, RetainedFixtureError> {
    let text = read_manifest_text(&path.join("SHA256SUMS"), 1024)?;
    let mut values = BTreeMap::new();
    for (line, expected_name) in text.lines().zip(PAYLOAD_FILES) {
        let (digest, name) = line.split_once("  ").ok_or(RetainedFixtureError)?;
        if name != expected_name || !is_lower_hex(digest, 64) {
            return Err(RetainedFixtureError);
        }
        values.insert(name.to_owned(), digest.to_owned());
    }
    if values.len() != PAYLOAD_FILES.len() || text.lines().count() != PAYLOAD_FILES.len() {
        return Err(RetainedFixtureError);
    }
    Ok(values)
}

fn verify_payload_checksums(
    path: &Path,
    checksums: &BTreeMap<String, String>,
) -> Result<(), RetainedFixtureError> {
    for name in PAYLOAD_FILES {
        let bytes = fs::read(path.join(name)).map_err(|_| RetainedFixtureError)?;
        let actual = lowercase_hex(&Sha256::digest(bytes));
        if checksums.get(name).map(String::as_str) != Some(actual.as_str()) {
            return Err(RetainedFixtureError);
        }
    }
    Ok(())
}

fn parse_provenance(path: &Path) -> Result<BTreeMap<String, String>, RetainedFixtureError> {
    let text = read_manifest_text(&path.join("PROVENANCE"), 8 * 1024)?;
    let mut values = BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line.split_once('=').ok_or(RetainedFixtureError)?;
        if key.is_empty()
            || value.is_empty()
            || values.insert(key.to_owned(), value.to_owned()).is_some()
        {
            return Err(RetainedFixtureError);
        }
    }
    let mut expected: BTreeSet<_> = PROVENANCE_FIELDS.into_iter().map(str::to_owned).collect();
    expected.insert(DEBUG_TRACE_FIELD.to_owned());
    if values.keys().cloned().collect::<BTreeSet<_>>() != expected {
        return Err(RetainedFixtureError);
    }
    Ok(values)
}

fn verify_provenance(
    values: &BTreeMap<String, String>,
    proof: &[u8],
    public_values: &[u8],
    trusted_key: &TrustedProgramKey,
) -> Result<(), RetainedFixtureError> {
    require_value(values, "format_version", "1")?;
    require_value(
        values,
        "classification",
        "public-zero-knowledge-test-fixture",
    )?;
    require_value(values, "fixture_id", "direct")?;
    require_value(values, "proof_system", "sp1-plonk")?;
    require_value(values, "benchmark_profile", "cluster-56g-8cpu")?;
    require_value(values, "proof_swap_peak_bytes", "0")?;
    require_value(values, "private_witness_retained", "false")?;
    require_value(values, DEBUG_TRACE_FIELD, "false")?;
    require_value(
        values,
        "verification_key_id",
        trusted_key.verification_key_id(),
    )?;
    require_value(values, "sp1_vkey_hash", trusted_key.sp1_vkey_hash())?;

    for key in [
        "source_manifest_sha256",
        "run_bundle_manifest_sha256",
        "proof_artifact_sha256",
        "public_values_sha256",
    ] {
        if !is_lower_hex(required(values, key)?, 64) {
            return Err(RetainedFixtureError);
        }
    }
    for key in ["source_git_commit", "source_git_tree"] {
        let value = required(values, key)?;
        if !matches!(value.len(), 40 | 64) || !is_lower_hex(value, value.len()) {
            return Err(RetainedFixtureError);
        }
    }
    let toolchain = required(values, "toolchain_image_id")?
        .strip_prefix("sha256:")
        .ok_or(RetainedFixtureError)?;
    if !is_lower_hex(toolchain, 64) {
        return Err(RetainedFixtureError);
    }
    for key in ["prove_ms", "baseline_verify_ms", "proof_memory_peak_bytes"] {
        if required(values, key)?
            .parse::<u64>()
            .ok()
            .filter(|value| *value > 0)
            .is_none()
        {
            return Err(RetainedFixtureError);
        }
    }
    require_value(values, "proof_artifact_bytes", &proof.len().to_string())?;
    require_value(
        values,
        "public_values_bytes",
        &public_values.len().to_string(),
    )?;
    require_value(
        values,
        "proof_artifact_sha256",
        &lowercase_hex(&Sha256::digest(proof)),
    )?;
    require_value(
        values,
        "public_values_sha256",
        &lowercase_hex(&Sha256::digest(public_values)),
    )?;
    let generated = required(values, "proof_generated_utc")?;
    if generated.len() != 20
        || generated.as_bytes().get(4) != Some(&b'-')
        || generated.as_bytes().get(7) != Some(&b'-')
        || generated.as_bytes().get(10) != Some(&b'T')
        || generated.as_bytes().get(13) != Some(&b':')
        || generated.as_bytes().get(16) != Some(&b':')
        || generated.as_bytes().get(19) != Some(&b'Z')
    {
        return Err(RetainedFixtureError);
    }
    Ok(())
}

fn read_manifest_text(path: &Path, maximum: usize) -> Result<String, RetainedFixtureError> {
    let bytes = read_bounded_file(path, 2, maximum)?;
    if bytes.last() != Some(&b'\n') || bytes.iter().any(|byte| *byte == 0 || *byte == b'\r') {
        return Err(RetainedFixtureError);
    }
    String::from_utf8(bytes).map_err(|_| RetainedFixtureError)
}

fn required<'a>(
    values: &'a BTreeMap<String, String>,
    key: &str,
) -> Result<&'a str, RetainedFixtureError> {
    values
        .get(key)
        .map(String::as_str)
        .ok_or(RetainedFixtureError)
}

fn require_value(
    values: &BTreeMap<String, String>,
    key: &str,
    expected: &str,
) -> Result<(), RetainedFixtureError> {
    if required(values, key)? == expected {
        Ok(())
    } else {
        Err(RetainedFixtureError)
    }
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use raziel_authorization_core::{
        AUTHORIZATION_PUBLIC_VALUES_VERSION, AuthorizationPublicValuesV1, encode_public_values,
    };

    use super::*;

    const KEY_ID: &str =
        "sp1-vk-v1:e96d8ab7a16a315c10b7adb0ddc15007c59cf5cf36d671a36bb75f1b0ce698b9";
    const SP1_KEY_HASH: &str = "0x00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    #[test]
    fn strict_public_fixture_loads() {
        let fixture = FixtureDirectory::new("valid");
        fixture.write_valid();
        let loaded = RetainedPlonkFixture::load(&fixture.path).unwrap();
        assert_eq!(loaded.proof_artifact(), b"public-proof");
        assert_eq!(loaded.public_values().policy_hash, [0x11; 32]);
        assert_eq!(loaded.trusted_program_key().verification_key_id(), KEY_ID);
    }

    #[test]
    fn extra_files_and_digest_mutations_fail_closed() {
        let extra = FixtureDirectory::new("extra");
        extra.write_valid();
        fs::write(extra.path.join("witness.bin"), b"private").unwrap();
        assert!(RetainedPlonkFixture::load(&extra.path).is_err());

        let changed = FixtureDirectory::new("changed");
        changed.write_valid();
        fs::write(changed.path.join("plonk-proof.bin"), b"changed").unwrap();
        assert!(RetainedPlonkFixture::load(&changed.path).is_err());
    }

    struct FixtureDirectory {
        path: std::path::PathBuf,
    }

    impl FixtureDirectory {
        fn new(label: &str) -> Self {
            static SEQUENCE: AtomicU64 = AtomicU64::new(0);
            let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "verifierd-retained-fixture-{label}-{}-{sequence}-{timestamp}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self { path }
        }

        fn write_valid(&self) {
            let proof = b"public-proof";
            let public_values = encode_public_values(&AuthorizationPublicValuesV1 {
                format_version: AUTHORIZATION_PUBLIC_VALUES_VERSION,
                policy_hash: [0x11; 32],
                relationship_snapshot_root: [0x22; 32],
                capability_request_commitment: [0x33; 32],
                capability_scope_hash: [0x44; 32],
                issued_at_unix_seconds: 1_000,
                expires_at_unix_seconds: 1_800,
                issuance_nullifier: [0x55; 32],
            })
            .unwrap();
            fs::write(self.path.join("plonk-proof.bin"), proof).unwrap();
            fs::write(self.path.join("public-values.bin"), &public_values).unwrap();
            fs::write(self.path.join("verification-key-id.txt"), KEY_ID).unwrap();
            fs::write(self.path.join("sp1-vkey-hash.txt"), SP1_KEY_HASH).unwrap();

            let payloads = [
                ("plonk-proof.bin", proof.as_slice()),
                ("public-values.bin", public_values.as_slice()),
                ("sp1-vkey-hash.txt", SP1_KEY_HASH.as_bytes()),
                ("verification-key-id.txt", KEY_ID.as_bytes()),
            ];
            let checksums = payloads
                .iter()
                .map(|(name, bytes)| format!("{}  {name}\n", lowercase_hex(&Sha256::digest(bytes))))
                .collect::<String>();
            fs::write(self.path.join("SHA256SUMS"), checksums).unwrap();
            let proof_hash = lowercase_hex(&Sha256::digest(proof));
            let public_hash = lowercase_hex(&Sha256::digest(&public_values));
            let provenance = format!(
                concat!(
                    "format_version=1\n",
                    "classification=public-zero-knowledge-test-fixture\n",
                    "fixture_id=direct\n",
                    "proof_system=sp1-plonk\n",
                    "source_git_commit={source}\n",
                    "source_git_tree={tree}\n",
                    "source_manifest_sha256={manifest}\n",
                    "run_bundle_manifest_sha256={bundle}\n",
                    "toolchain_image_id=sha256:{image}\n",
                    "benchmark_profile=cluster-56g-8cpu\n",
                    "proof_generated_utc=2026-08-25T00:00:00Z\n",
                    "prove_ms=100\n",
                    "baseline_verify_ms=10\n",
                    "proof_artifact_bytes={proof_bytes}\n",
                    "proof_artifact_sha256={proof_hash}\n",
                    "public_values_bytes={public_bytes}\n",
                    "public_values_sha256={public_hash}\n",
                    "verification_key_id={KEY_ID}\n",
                    "sp1_vkey_hash={SP1_KEY_HASH}\n",
                    "proof_memory_peak_bytes=1024\n",
                    "proof_swap_peak_bytes=0\n",
                    "private_witness_retained=false\n",
                    "debug_trace_retained=false\n",
                ),
                source = "a".repeat(40),
                tree = "b".repeat(40),
                manifest = "c".repeat(64),
                bundle = "d".repeat(64),
                image = "e".repeat(64),
                proof_bytes = proof.len(),
                public_bytes = public_values.len(),
                proof_hash = proof_hash,
                public_hash = public_hash,
                KEY_ID = KEY_ID,
                SP1_KEY_HASH = SP1_KEY_HASH,
            );
            fs::write(self.path.join("PROVENANCE"), provenance).unwrap();
        }
    }

    impl Drop for FixtureDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
