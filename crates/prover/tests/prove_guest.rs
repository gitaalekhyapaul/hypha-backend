use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

use raziel_authorization_core::AuthorizationGuestInputV1;
use raziel_prover::{
    FORBIDDEN_SP1_ENV_VARS, MAX_PROOF_ARTIFACT_BYTES, ProverError, Sp1Executor, Sp1Prover,
    Sp1Verifier, decode_proof_artifact, encode_proof_artifact, prepare_guest_input,
};
use sha2::{Digest, Sha256};
use sp1_sdk::SP1Proof;

#[path = "support/authorization_fixture.rs"]
mod authorization_fixture;
mod support;

use authorization_fixture::authorization_inputs;
use support::{KEY_THUMBPRINT, guest_elf_path};

const AGENT_PUBLIC_KEY: [u8; 32] = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];

const DIRECT_FIXTURE_INSTRUCTIONS: u64 = 422_138;
const PROOF_DEADLINE: Duration = Duration::from_secs(60 * 60);
const VERIFICATION_DEADLINE: Duration = Duration::from_secs(60);
const UNSAFE_ENV_HELPER: &str = "RAZIEL_UNSAFE_SP1_ENV_HELPER";
const BENCHMARK_DIRECTORY_ENV: &str = "RAZIEL_SP1_BENCHMARK_DIRECTORY";
const PROOF_ARTIFACT_FILE: &str = "plonk-proof.bin";
const PUBLIC_VALUES_FILE: &str = "public-values.bin";
const SP1_VKEY_HASH_FILE: &str = "sp1-vkey-hash.txt";
const VERIFICATION_KEY_ID_FILE: &str = "verification-key-id.txt";

fn direct_authorization_input() -> AuthorizationGuestInputV1 {
    authorization_inputs()
        .into_iter()
        .find_map(|(case_id, input)| (case_id == "direct").then_some(input))
        .expect("direct authorization fixture must exist")
}

#[test]
fn direct_fixture_uses_the_m4_signable_agent_identity() {
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(AGENT_PUBLIC_KEY)),
        KEY_THUMBPRINT
    );
    assert_eq!(
        direct_authorization_input().request.agent_key_thumbprint,
        KEY_THUMBPRINT
    );
}

#[test]
fn prover_rejects_unsafe_sp1_environment_settings() {
    if let Some(forbidden) = std::env::var_os(UNSAFE_ENV_HELPER) {
        let prepared = prepare_guest_input(direct_authorization_input()).unwrap();
        assert_eq!(
            Sp1Prover::local().prove(&[], &prepared),
            Err(ProverError::UnsafeSp1Environment),
            "{} was not rejected",
            forbidden.to_string_lossy(),
        );
        assert!(matches!(
            Sp1Verifier::from_trusted_elf(&[]),
            Err(ProverError::UnsafeSp1Environment)
        ));
        return;
    }

    for forbidden in FORBIDDEN_SP1_ENV_VARS
        .iter()
        .copied()
        .chain(["SP1_WORKER_FUTURE_OPTION"])
    {
        let output = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("prover_rejects_unsafe_sp1_environment_settings")
            .arg("--nocapture")
            .env(UNSAFE_ENV_HELPER, forbidden)
            .env(forbidden, "1")
            .output()
            .expect("unsafe-environment subprocess must start");
        assert!(
            output.status.success(),
            "{forbidden} subprocess failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
#[ignore = "runs the ADR-0012 capped local Plonk proof benchmark"]
fn local_plonk_execute_records_the_adr_0012_fixture_metrics() {
    let guest_elf = fs::read(guest_elf_path()).expect("authorization guest ELF must exist");
    let input = direct_authorization_input();
    let prepared = prepare_guest_input(input).expect("direct authorization fixture must prepare");

    let execute_started = Instant::now();
    let execution = Sp1Executor::local()
        .execute(&guest_elf, &prepared)
        .expect("direct fixture must execute");
    let execute_elapsed = execute_started.elapsed();
    assert_eq!(
        execution.instruction_count, DIRECT_FIXTURE_INSTRUCTIONS,
        "direct fixture instruction count changed"
    );

    eprintln!(
        "adr_0012_execute: proof_mode=plonk, fixture_id=direct, instructions={}, execute_ms={}",
        execution.instruction_count,
        execute_elapsed.as_millis(),
    );
}

#[test]
#[ignore = "runs the ADR-0012 capped local Plonk proof benchmark"]
fn local_plonk_proof_passes_the_adr_0012_generation_gate() {
    let guest_elf = fs::read(guest_elf_path()).expect("authorization guest ELF must exist");
    let prepared = prepare_guest_input(direct_authorization_input())
        .expect("direct authorization fixture must prepare");

    let prove_started = Instant::now();
    let proof = Sp1Prover::local()
        .prove(&guest_elf, &prepared)
        .expect("local CpuProver must create a Plonk proof");
    let prove_elapsed = prove_started.elapsed();
    assert!(
        prove_elapsed <= PROOF_DEADLINE,
        "Plonk proof exceeded the ADR-0012 deadline"
    );
    assert!(
        proof.artifact.len() <= MAX_PROOF_ARTIFACT_BYTES,
        "Plonk proof exceeded the ADR-0012 artifact ceiling"
    );
    assert_eq!(
        proof.public_values,
        prepared.expected_public_values_bytes(),
        "prover returned the wrong public journal"
    );

    let benchmark_directory = benchmark_directory();
    fs::write(
        benchmark_directory.join(PROOF_ARTIFACT_FILE),
        &proof.artifact,
    )
    .expect("public proof artifact must be stored for verification");
    fs::write(
        benchmark_directory.join(VERIFICATION_KEY_ID_FILE),
        &proof.verification_key_id,
    )
    .expect("public verification-key ID must be stored for verification");
    fs::write(
        benchmark_directory.join(PUBLIC_VALUES_FILE),
        &proof.public_values,
    )
    .expect("public journal must be stored for independent verification");
    eprintln!(
        "adr_0012_prove: proof_mode=plonk, prove_ms={}, artifact_bytes={}, verification_key_id={}",
        prove_elapsed.as_millis(),
        proof.artifact.len(),
        proof.verification_key_id,
    );
}

#[test]
#[ignore = "runs the ADR-0012 capped local Plonk proof benchmark"]
fn local_plonk_verification_passes_the_adr_0012_gate() {
    let guest_elf = fs::read(guest_elf_path()).expect("authorization guest ELF must exist");
    let prepared = prepare_guest_input(direct_authorization_input())
        .expect("direct authorization fixture must prepare");
    let expected_journal = prepared.expected_public_values_bytes();
    let stored = stored_proof();
    let verifier = Sp1Verifier::from_trusted_elf(&guest_elf).expect("trusted guest key must load");
    let verify_started = Instant::now();
    let verified = verifier
        .verify(
            &stored.artifact,
            &stored.verification_key_id,
            expected_journal,
        )
        .expect("LightProver must verify the local Plonk proof");
    let verify_elapsed = verify_started.elapsed();
    assert!(
        verify_elapsed <= VERIFICATION_DEADLINE,
        "Plonk proof verification exceeded the ADR-0012 deadline"
    );
    assert_eq!(verified.verification_key_id, stored.verification_key_id);
    assert_eq!(verified.public_values, expected_journal);
    fs::write(
        benchmark_directory().join(SP1_VKEY_HASH_FILE),
        verifier.sp1_vkey_hash(),
    )
    .expect("public SP1 verification-key hash must be stored for independent verification");

    eprintln!(
        "adr_0012_verify: proof_mode=plonk, verify_ms={}, verification_key_id={}",
        verify_elapsed.as_millis(),
        verified.verification_key_id,
    );
}

#[test]
#[ignore = "reuses the ADR-0012 Plonk proof for mutation checks"]
fn local_plonk_proof_mutations_fail_closed() {
    let guest_elf = fs::read(guest_elf_path()).expect("authorization guest ELF must exist");
    let prepared = prepare_guest_input(direct_authorization_input())
        .expect("direct authorization fixture must prepare");
    let expected_journal = prepared.expected_public_values_bytes();
    let stored = stored_proof();

    let verifier = Sp1Verifier::from_trusted_elf(&guest_elf).expect("trusted guest key must load");
    let mut changed_bundle = decode_proof_artifact(&stored.artifact).unwrap();
    let SP1Proof::Plonk(changed_proof) = &mut changed_bundle.proof else {
        panic!("stored proof must use Plonk");
    };
    mutate_hex_tail(&mut changed_proof.encoded_proof);
    let changed_artifact = encode_proof_artifact(&changed_bundle).unwrap();
    assert_eq!(
        verifier.verify(
            &changed_artifact,
            &stored.verification_key_id,
            expected_journal,
        ),
        Err(ProverError::ProofVerificationFailed),
        "changed proof bytes did not reach cryptographic verification"
    );

    let mut wrong_key_id = stored.verification_key_id.clone().into_bytes();
    let last = wrong_key_id.last_mut().unwrap();
    *last = if *last == b'0' { b'1' } else { b'0' };
    let wrong_key_id = String::from_utf8(wrong_key_id).unwrap();
    assert_eq!(
        verifier.verify(&stored.artifact, &wrong_key_id, expected_journal,),
        Err(ProverError::VerificationKeyMismatch)
    );

    let alternate_guest_elf = alternate_program_elf(&guest_elf);
    let alternate_verifier = Sp1Verifier::from_trusted_elf(&alternate_guest_elf)
        .expect("alternate guest ELF must produce a verification key");
    let alternate_key_id = alternate_verifier.verification_key_id();
    assert_ne!(alternate_key_id, stored.verification_key_id);
    assert_eq!(
        alternate_verifier.verify(&stored.artifact, alternate_key_id, expected_journal,),
        Err(ProverError::ProofVerificationFailed),
        "a different program key verified the proof"
    );

    let mut changed_journal = expected_journal.to_vec();
    changed_journal[0] ^= 1;
    assert_public_journal_rejected(
        &verifier,
        &stored.artifact,
        &stored.verification_key_id,
        &changed_journal,
    );
    assert_public_journal_rejected(
        &verifier,
        &stored.artifact,
        &stored.verification_key_id,
        &expected_journal[..expected_journal.len() - 1],
    );
    let mut extended_journal = expected_journal.to_vec();
    extended_journal.push(0);
    assert_public_journal_rejected(
        &verifier,
        &stored.artifact,
        &stored.verification_key_id,
        &extended_journal,
    );

    let mut changed_journal_bundle = decode_proof_artifact(&stored.artifact).unwrap();
    changed_journal_bundle.public_values =
        sp1_sdk::SP1PublicValues::from(changed_journal.as_slice());
    let changed_journal_artifact = encode_proof_artifact(&changed_journal_bundle).unwrap();
    assert_eq!(
        verifier.verify(
            &changed_journal_artifact,
            &stored.verification_key_id,
            &changed_journal,
        ),
        Err(ProverError::ProofVerificationFailed),
        "changed artifact journal did not reach cryptographic verification"
    );

    for input_index in 0..5 {
        let mut changed_input_bundle = decode_proof_artifact(&stored.artifact).unwrap();
        let SP1Proof::Plonk(changed_input_proof) = &mut changed_input_bundle.proof else {
            panic!("stored proof must use Plonk");
        };
        let replacement = if changed_input_proof.public_inputs[input_index] == "0" {
            "1"
        } else {
            "0"
        };
        changed_input_proof.public_inputs[input_index] = replacement.to_owned();
        if input_index >= 2 {
            set_encoded_metadata_word(
                &mut changed_input_proof.encoded_proof,
                input_index - 2,
                replacement,
            );
        }
        let changed_input_artifact = encode_proof_artifact(&changed_input_bundle).unwrap();
        assert_eq!(
            verifier.verify(
                &changed_input_artifact,
                &stored.verification_key_id,
                expected_journal,
            ),
            Err(ProverError::ProofVerificationFailed),
            "changed Plonk public input {input_index} was accepted"
        );
    }

    let mut trailing_artifact = stored.artifact;
    trailing_artifact.push(0);
    assert_eq!(
        verifier.verify(
            &trailing_artifact,
            &stored.verification_key_id,
            expected_journal,
        ),
        Err(ProverError::ProofArtifactInvalid)
    );
}

fn mutate_hex_tail(value: &mut String) {
    let mut bytes = value.as_bytes().to_vec();
    let last = bytes.last_mut().expect("encoded proof must not be empty");
    *last = if *last == b'0' { b'1' } else { b'0' };
    *value = String::from_utf8(bytes).unwrap();
}

fn set_encoded_metadata_word(value: &mut String, word_index: usize, decimal: &str) {
    assert!(word_index < 3);
    assert!(matches!(decimal, "0" | "1"));
    let mut bytes = value.as_bytes().to_vec();
    let start = word_index * 64;
    bytes[start..start + 64].fill(b'0');
    if decimal == "1" {
        bytes[start + 63] = b'1';
    }
    *value = String::from_utf8(bytes).unwrap();
}

struct StoredProof {
    artifact: Vec<u8>,
    verification_key_id: String,
}

fn benchmark_directory() -> PathBuf {
    std::env::var_os(BENCHMARK_DIRECTORY_ENV)
        .map(PathBuf::from)
        .expect("benchmark directory environment variable must be set")
}

fn stored_proof() -> StoredProof {
    let benchmark_directory = benchmark_directory();
    StoredProof {
        artifact: fs::read(benchmark_directory.join(PROOF_ARTIFACT_FILE))
            .expect("public proof artifact must exist"),
        verification_key_id: fs::read_to_string(benchmark_directory.join(VERIFICATION_KEY_ID_FILE))
            .expect("public verification-key ID must exist"),
    }
}

fn alternate_program_elf(guest_elf: &[u8]) -> Vec<u8> {
    const ELF64_PROGRAM_HEADER_OFFSET: usize = 32;
    const ELF64_PROGRAM_HEADER_ENTRY_SIZE_OFFSET: usize = 54;
    const ELF64_PROGRAM_HEADER_COUNT_OFFSET: usize = 56;
    const PROGRAM_HEADER_TYPE_OFFSET: usize = 0;
    const PROGRAM_HEADER_FLAGS_OFFSET: usize = 4;
    const PROGRAM_HEADER_FILE_OFFSET: usize = 8;
    const PROGRAM_HEADER_FILE_SIZE_OFFSET: usize = 32;
    const PROGRAM_HEADER_MINIMUM_SIZE: usize = 40;
    const LOAD_SEGMENT: u32 = 1;
    const EXECUTABLE_FLAG: u32 = 1;

    assert_eq!(guest_elf.get(..4), Some(b"\x7fELF".as_slice()));
    assert_eq!(guest_elf.get(4), Some(&2), "guest must use ELF64");
    assert_eq!(guest_elf.get(5), Some(&1), "guest must use little endian");

    let program_header_offset = read_u64_le(guest_elf, ELF64_PROGRAM_HEADER_OFFSET) as usize;
    let entry_size = read_u16_le(guest_elf, ELF64_PROGRAM_HEADER_ENTRY_SIZE_OFFSET) as usize;
    let entry_count = read_u16_le(guest_elf, ELF64_PROGRAM_HEADER_COUNT_OFFSET) as usize;
    assert!(entry_size >= PROGRAM_HEADER_MINIMUM_SIZE);

    let mut alternate = guest_elf.to_vec();
    for index in 0..entry_count {
        let header_offset = program_header_offset
            .checked_add(
                index
                    .checked_mul(entry_size)
                    .expect("program-header offset"),
            )
            .expect("program-header offset");
        let segment_type = read_u32_le(guest_elf, header_offset + PROGRAM_HEADER_TYPE_OFFSET);
        let flags = read_u32_le(guest_elf, header_offset + PROGRAM_HEADER_FLAGS_OFFSET);
        if segment_type != LOAD_SEGMENT || flags & EXECUTABLE_FLAG == 0 {
            continue;
        }

        let file_offset =
            read_u64_le(guest_elf, header_offset + PROGRAM_HEADER_FILE_OFFSET) as usize;
        let file_size =
            read_u64_le(guest_elf, header_offset + PROGRAM_HEADER_FILE_SIZE_OFFSET) as usize;
        assert!(file_size >= 8, "executable segment is too small");
        let instruction_offset = (file_size / 2) & !3;
        let mutation_offset = file_offset
            .checked_add(instruction_offset)
            .and_then(|offset| offset.checked_add(3))
            .expect("executable-segment mutation offset");
        alternate
            .get_mut(mutation_offset)
            .expect("executable-segment mutation byte must exist");
        alternate[mutation_offset] ^= 0x80;
        return alternate;
    }

    panic!("guest ELF has no executable load segment");
}

fn read_u16_le(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .expect("ELF u16 field must exist")
            .try_into()
            .unwrap(),
    )
}

fn read_u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .expect("ELF u32 field must exist")
            .try_into()
            .unwrap(),
    )
}

fn read_u64_le(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes
            .get(offset..offset + 8)
            .expect("ELF u64 field must exist")
            .try_into()
            .unwrap(),
    )
}

fn assert_public_journal_rejected(
    verifier: &Sp1Verifier,
    artifact: &[u8],
    verification_key_id: &str,
    expected_public_values: &[u8],
) {
    assert_eq!(
        verifier.verify(artifact, verification_key_id, expected_public_values,),
        Err(ProverError::PublicValuesMismatch)
    );
}
