use bincode::Options;
use serde::{Deserialize, Serialize};
#[cfg(feature = "proving")]
use sp1_sdk::SP1ProofWithPublicValues;
use sp1_verifier::{ProofFromNetwork, SP1Proof};

use crate::{
    ProverError,
    version::{SP1_CIRCUIT_VERSION, SP1_SDK_VERSION},
};

const ARTIFACT_MAGIC: &[u8; 8] = b"RZSP1PF1";
const PLONK_PROOF_MODE: u8 = 2;
const BN254_SCALAR_MODULUS_DECIMAL: &[u8] =
    b"21888242871839275222246405745257275088548364400416034343698204186575808495617";
const PLONK_ENCODED_PROOF_BYTES: usize = 96 + 0x300 + 0x60;
const PLONK_ENCODED_PROOF_HEX_CHARACTERS: usize = PLONK_ENCODED_PROOF_BYTES * 2;
const PINNED_PLONK_VKEY_SHA256: [u8; 32] = [
    0x5a, 0x09, 0x3a, 0x2f, 0xcb, 0x46, 0x39, 0x4f, 0x5c, 0xad, 0xfe, 0x55, 0xc4, 0x4d, 0x4d, 0x57,
    0x2f, 0xad, 0x9c, 0xec, 0x7a, 0xeb, 0x38, 0x02, 0x6b, 0x02, 0x78, 0x32, 0x2e, 0xf0, 0x7f, 0xac,
];

pub const MAX_PROOF_ARTIFACT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CompatibleProofPayload {
    network_proof: ProofFromNetwork,
    tee_proof: Option<Vec<u8>>,
}

pub(crate) struct DecodedPlonkArtifact {
    pub public_inputs: [String; 5],
    pub public_values: Vec<u8>,
    pub proof_bytes: Vec<u8>,
}

#[cfg(feature = "proving")]
pub fn encode_proof_artifact(proof: &SP1ProofWithPublicValues) -> Result<Vec<u8>, ProverError> {
    let proof = canonicalize_proof(proof.clone())?;

    let payload_size = proof_payload_options()
        .serialized_size(&proof)
        .map_err(|_| ProverError::ProofArtifactInvalid)?;
    if payload_size > MAX_PROOF_ARTIFACT_BYTES as u64 {
        return Err(ProverError::ProofArtifactTooLarge);
    }

    let payload = proof_payload_options()
        .serialize(&proof)
        .map_err(|_| ProverError::ProofArtifactInvalid)?;

    frame_payload(&payload)
}

#[cfg(feature = "proving")]
pub fn decode_proof_artifact(artifact: &[u8]) -> Result<SP1ProofWithPublicValues, ProverError> {
    let payload = decode_payload(artifact)?;

    let proof: SP1ProofWithPublicValues = proof_payload_options()
        .with_limit(payload.len() as u64)
        .deserialize(payload)
        .map_err(|_| ProverError::ProofArtifactInvalid)?;
    let proof = canonicalize_proof(proof)?;
    let canonical_payload = proof_payload_options()
        .serialize(&proof)
        .map_err(|_| ProverError::ProofArtifactInvalid)?;
    if canonical_payload.as_slice() != payload {
        return Err(ProverError::ProofArtifactInvalid);
    }
    Ok(proof)
}

pub(crate) fn decode_plonk_artifact(artifact: &[u8]) -> Result<DecodedPlonkArtifact, ProverError> {
    let encoded_payload = decode_payload(artifact)?;
    let payload: CompatibleProofPayload = proof_payload_options()
        .with_limit(encoded_payload.len() as u64)
        .deserialize(encoded_payload)
        .map_err(|_| ProverError::ProofArtifactInvalid)?;
    let payload = canonicalize_compatible_payload(payload)?;
    let canonical_payload = proof_payload_options()
        .serialize(&payload)
        .map_err(|_| ProverError::ProofArtifactInvalid)?;
    if canonical_payload.as_slice() != encoded_payload {
        return Err(ProverError::ProofArtifactInvalid);
    }

    let SP1Proof::Plonk(plonk) = payload.network_proof.proof else {
        return Err(ProverError::ProofModeUnsupported);
    };
    let encoded_proof = decode_lower_hex(&plonk.encoded_proof)?;
    let mut proof_bytes = Vec::with_capacity(4 + encoded_proof.len());
    proof_bytes.extend_from_slice(&plonk.plonk_vkey_hash[..4]);
    proof_bytes.extend_from_slice(&encoded_proof);

    Ok(DecodedPlonkArtifact {
        public_inputs: plonk.public_inputs,
        public_values: payload.network_proof.public_values.to_vec(),
        proof_bytes,
    })
}

#[cfg(any(feature = "proving", test))]
fn frame_payload(payload: &[u8]) -> Result<Vec<u8>, ProverError> {
    let mut artifact = Vec::with_capacity(
        ARTIFACT_MAGIC.len()
            + 1
            + encoded_bytes_len(SP1_SDK_VERSION.len())?
            + encoded_bytes_len(SP1_CIRCUIT_VERSION.len())?
            + encoded_bytes_len(payload.len())?,
    );
    artifact.extend_from_slice(ARTIFACT_MAGIC);
    artifact.push(PLONK_PROOF_MODE);
    write_bytes(&mut artifact, SP1_SDK_VERSION.as_bytes())?;
    write_bytes(&mut artifact, SP1_CIRCUIT_VERSION.as_bytes())?;
    write_bytes(&mut artifact, payload)?;

    if artifact.len() > MAX_PROOF_ARTIFACT_BYTES {
        return Err(ProverError::ProofArtifactTooLarge);
    }
    Ok(artifact)
}

fn decode_payload(artifact: &[u8]) -> Result<&[u8], ProverError> {
    if artifact.len() > MAX_PROOF_ARTIFACT_BYTES {
        return Err(ProverError::ProofArtifactTooLarge);
    }

    let mut decoder = ArtifactDecoder::new(artifact);
    decoder.expect_exact(ARTIFACT_MAGIC)?;
    if decoder.read_u8()? != PLONK_PROOF_MODE {
        return Err(ProverError::ProofModeUnsupported);
    }
    if decoder.read_utf8()? != SP1_SDK_VERSION {
        return Err(ProverError::SdkVersionMismatch);
    }
    if decoder.read_utf8()? != SP1_CIRCUIT_VERSION {
        return Err(ProverError::CircuitVersionMismatch);
    }
    let payload = decoder.read_bytes()?;
    decoder.finish()?;
    Ok(payload)
}

fn proof_payload_options() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_little_endian()
        .reject_trailing_bytes()
}

#[cfg(feature = "proving")]
fn canonicalize_proof(
    mut proof: SP1ProofWithPublicValues,
) -> Result<SP1ProofWithPublicValues, ProverError> {
    if proof.sp1_version != SP1_CIRCUIT_VERSION {
        return Err(ProverError::CircuitVersionMismatch);
    }
    if proof.tee_proof.is_some() {
        return Err(ProverError::TeeProofUnsupported);
    }

    canonicalize_plonk_proof(&mut proof.proof)?;
    Ok(proof)
}

fn canonicalize_compatible_payload(
    mut payload: CompatibleProofPayload,
) -> Result<CompatibleProofPayload, ProverError> {
    if payload.network_proof.sp1_version != SP1_CIRCUIT_VERSION {
        return Err(ProverError::CircuitVersionMismatch);
    }
    if payload.tee_proof.is_some() {
        return Err(ProverError::TeeProofUnsupported);
    }
    canonicalize_plonk_proof(&mut payload.network_proof.proof)?;
    Ok(payload)
}

fn canonicalize_plonk_proof(proof: &mut SP1Proof) -> Result<(), ProverError> {
    let SP1Proof::Plonk(plonk) = proof else {
        return Err(ProverError::ProofModeUnsupported);
    };
    if plonk.plonk_vkey_hash != PINNED_PLONK_VKEY_SHA256
        || plonk
            .public_inputs
            .iter()
            .any(|value| !is_canonical_decimal(value))
        || !is_canonical_encoded_proof(&plonk.encoded_proof)
    {
        return Err(ProverError::ProofArtifactInvalid);
    }

    // SP1 release verification ignores raw_proof. Raziel omits that unauthenticated duplicate.
    plonk.raw_proof.clear();
    Ok(())
}

fn is_canonical_decimal(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && (bytes.len() == 1 || bytes[0] != b'0')
        && bytes.iter().all(u8::is_ascii_digit)
        && (bytes.len() < BN254_SCALAR_MODULUS_DECIMAL.len()
            || (bytes.len() == BN254_SCALAR_MODULUS_DECIMAL.len()
                && bytes < BN254_SCALAR_MODULUS_DECIMAL))
}

fn is_canonical_encoded_proof(value: &str) -> bool {
    value.len() == PLONK_ENCODED_PROOF_HEX_CHARACTERS
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn decode_lower_hex(value: &str) -> Result<Vec<u8>, ProverError> {
    if !value.len().is_multiple_of(2) {
        return Err(ProverError::ProofArtifactInvalid);
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = lower_hex_nibble(pair[0])?;
            let low = lower_hex_nibble(pair[1])?;
            Ok((high << 4) | low)
        })
        .collect()
}

const fn lower_hex_nibble(byte: u8) -> Result<u8, ProverError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(ProverError::ProofArtifactInvalid),
    }
}

#[cfg(any(feature = "proving", test))]
fn encoded_bytes_len(length: usize) -> Result<usize, ProverError> {
    u32::try_from(length).map_err(|_| ProverError::ProofArtifactTooLarge)?;
    4_usize
        .checked_add(length)
        .ok_or(ProverError::ProofArtifactTooLarge)
}

#[cfg(any(feature = "proving", test))]
fn write_bytes(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), ProverError> {
    let length = u32::try_from(bytes.len()).map_err(|_| ProverError::ProofArtifactTooLarge)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(bytes);
    Ok(())
}

struct ArtifactDecoder<'a> {
    remaining: &'a [u8],
}

impl<'a> ArtifactDecoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    fn expect_exact(&mut self, expected: &[u8]) -> Result<(), ProverError> {
        if self.take(expected.len())? != expected {
            return Err(ProverError::ProofArtifactInvalid);
        }
        Ok(())
    }

    fn read_u8(&mut self) -> Result<u8, ProverError> {
        Ok(self.take(1)?[0])
    }

    fn read_utf8(&mut self) -> Result<&'a str, ProverError> {
        std::str::from_utf8(self.read_bytes()?).map_err(|_| ProverError::ProofArtifactInvalid)
    }

    fn read_bytes(&mut self) -> Result<&'a [u8], ProverError> {
        let length_bytes: [u8; 4] = self
            .take(4)?
            .try_into()
            .map_err(|_| ProverError::ProofArtifactInvalid)?;
        let length = u32::from_be_bytes(length_bytes) as usize;
        self.take(length)
    }

    fn finish(self) -> Result<(), ProverError> {
        if self.remaining.is_empty() {
            Ok(())
        } else {
            Err(ProverError::ProofArtifactInvalid)
        }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ProverError> {
        if self.remaining.len() < length {
            return Err(ProverError::ProofArtifactInvalid);
        }
        let (value, remaining) = self.remaining.split_at(length);
        self.remaining = remaining;
        Ok(value)
    }
}

#[cfg(all(test, feature = "proving"))]
mod tests {
    use sp1_sdk::{SP1Proof, SP1ProofWithPublicValues, SP1PublicValues};

    use super::*;

    fn plonk_bundle() -> SP1ProofWithPublicValues {
        let mut proof = SP1Proof::Plonk(Default::default());
        let SP1Proof::Plonk(plonk) = &mut proof else {
            unreachable!()
        };
        plonk.public_inputs = std::array::from_fn(|index| index.to_string());
        plonk.public_inputs[0] = "1".to_owned();
        plonk.encoded_proof = "00".repeat(PLONK_ENCODED_PROOF_BYTES);
        plonk.raw_proof = "ignored duplicate".to_owned();
        plonk.plonk_vkey_hash = PINNED_PLONK_VKEY_SHA256;
        SP1ProofWithPublicValues {
            proof,
            public_values: SP1PublicValues::from(&b"journal"[..]),
            sp1_version: SP1_CIRCUIT_VERSION.to_owned(),
            tee_proof: None,
        }
    }

    fn frame_unvalidated_payload(proof: &SP1ProofWithPublicValues) -> Vec<u8> {
        let payload = proof_payload_options().serialize(proof).unwrap();
        let mut artifact = Vec::new();
        artifact.extend_from_slice(ARTIFACT_MAGIC);
        artifact.push(PLONK_PROOF_MODE);
        write_bytes(&mut artifact, SP1_SDK_VERSION.as_bytes()).unwrap();
        write_bytes(&mut artifact, SP1_CIRCUIT_VERSION.as_bytes()).unwrap();
        write_bytes(&mut artifact, &payload).unwrap();
        artifact
    }

    #[test]
    fn proof_artifact_round_trips_with_one_canonical_encoding() {
        let expected = plonk_bundle();
        let encoded = encode_proof_artifact(&expected).unwrap();
        let decoded = decode_proof_artifact(&encoded).unwrap();

        assert!(matches!(decoded.proof, SP1Proof::Plonk(_)));
        assert!(matches!(&decoded.proof, SP1Proof::Plonk(plonk) if plonk.raw_proof.is_empty()));
        assert_eq!(decoded.public_values.as_slice(), b"journal");
        assert_eq!(decoded.sp1_version, SP1_CIRCUIT_VERSION);
        assert!(decoded.tee_proof.is_none());
        assert_eq!(encode_proof_artifact(&decoded).unwrap(), encoded);
    }

    #[test]
    fn direct_decoder_is_byte_compatible_with_the_sdk_payload() {
        let proof = canonicalize_proof(plonk_bundle()).unwrap();
        let sdk_payload = proof_payload_options().serialize(&proof).unwrap();
        let compatible: CompatibleProofPayload =
            proof_payload_options().deserialize(&sdk_payload).unwrap();
        let compatible_payload = proof_payload_options().serialize(&compatible).unwrap();
        assert_eq!(compatible_payload, sdk_payload);

        let artifact = frame_payload(&sdk_payload).unwrap();
        let decoded = decode_plonk_artifact(&artifact).unwrap();
        assert_eq!(decoded.public_values, b"journal");
        assert_eq!(decoded.public_inputs, ["1", "1", "2", "3", "4"]);
        assert_eq!(decoded.proof_bytes.len(), 4 + PLONK_ENCODED_PROOF_BYTES);
        assert_eq!(&decoded.proof_bytes[..4], &PINNED_PLONK_VKEY_SHA256[..4]);
    }

    #[test]
    fn proof_artifact_rejects_outer_mutations_and_trailing_bytes() {
        let encoded = encode_proof_artifact(&plonk_bundle()).unwrap();
        let mut wrong_magic = encoded.clone();
        wrong_magic[0] ^= 1;
        assert!(matches!(
            decode_proof_artifact(&wrong_magic),
            Err(ProverError::ProofArtifactInvalid)
        ));

        let mut wrong_mode = encoded.clone();
        wrong_mode[ARTIFACT_MAGIC.len()] = 1;
        assert!(matches!(
            decode_proof_artifact(&wrong_mode),
            Err(ProverError::ProofModeUnsupported)
        ));

        let mut wrong_sdk = encoded.clone();
        wrong_sdk[ARTIFACT_MAGIC.len() + 1 + 4] ^= 1;
        assert!(matches!(
            decode_proof_artifact(&wrong_sdk),
            Err(ProverError::SdkVersionMismatch)
        ));

        let circuit_start =
            ARTIFACT_MAGIC.len() + 1 + encoded_bytes_len(SP1_SDK_VERSION.len()).unwrap() + 4;
        let mut wrong_circuit = encoded.clone();
        wrong_circuit[circuit_start] ^= 1;
        assert!(matches!(
            decode_proof_artifact(&wrong_circuit),
            Err(ProverError::CircuitVersionMismatch)
        ));

        assert!(matches!(
            decode_proof_artifact(&encoded[..encoded.len() - 1]),
            Err(ProverError::ProofArtifactInvalid)
        ));

        let mut trailing = encoded;
        trailing.push(0);
        assert!(matches!(
            decode_proof_artifact(&trailing),
            Err(ProverError::ProofArtifactInvalid)
        ));
    }

    #[test]
    fn proof_artifact_rejects_version_and_tee_mutations() {
        let mut wrong_circuit = plonk_bundle();
        wrong_circuit.sp1_version = "another-circuit".into();
        assert_eq!(
            encode_proof_artifact(&wrong_circuit),
            Err(ProverError::CircuitVersionMismatch)
        );
        assert!(matches!(
            decode_proof_artifact(&frame_unvalidated_payload(&wrong_circuit)),
            Err(ProverError::CircuitVersionMismatch)
        ));

        let mut tee = plonk_bundle();
        tee.tee_proof = Some(vec![1]);
        assert_eq!(
            encode_proof_artifact(&tee),
            Err(ProverError::TeeProofUnsupported)
        );
        assert!(matches!(
            decode_proof_artifact(&frame_unvalidated_payload(&tee)),
            Err(ProverError::TeeProofUnsupported)
        ));

        let core = SP1ProofWithPublicValues {
            proof: SP1Proof::Core(Vec::new()),
            public_values: SP1PublicValues::from(&b"journal"[..]),
            sp1_version: SP1_CIRCUIT_VERSION.to_owned(),
            tee_proof: None,
        };
        assert!(matches!(
            decode_proof_artifact(&frame_unvalidated_payload(&core)),
            Err(ProverError::ProofModeUnsupported)
        ));
    }

    #[test]
    fn proof_artifact_rejects_unauthenticated_or_noncanonical_plonk_fields() {
        let mut raw_proof = plonk_bundle();
        let SP1Proof::Plonk(plonk) = &mut raw_proof.proof else {
            unreachable!()
        };
        plonk.raw_proof = "must not be accepted from the wire".to_owned();
        assert!(matches!(
            decode_proof_artifact(&frame_unvalidated_payload(&raw_proof)),
            Err(ProverError::ProofArtifactInvalid)
        ));

        let mut wrong_vkey = plonk_bundle();
        let SP1Proof::Plonk(plonk) = &mut wrong_vkey.proof else {
            unreachable!()
        };
        plonk.plonk_vkey_hash[0] ^= 1;
        assert_eq!(
            encode_proof_artifact(&wrong_vkey),
            Err(ProverError::ProofArtifactInvalid)
        );

        for invalid_decimal in [
            String::new(),
            "01".to_owned(),
            "+1".to_owned(),
            "a".to_owned(),
            BN254_SCALAR_MODULUS_DECIMAL
                .iter()
                .map(|byte| char::from(*byte))
                .collect(),
            "9".repeat(BN254_SCALAR_MODULUS_DECIMAL.len()),
            "9".repeat(BN254_SCALAR_MODULUS_DECIMAL.len() + 1),
        ] {
            let mut invalid = plonk_bundle();
            let SP1Proof::Plonk(plonk) = &mut invalid.proof else {
                unreachable!()
            };
            plonk.public_inputs[0] = invalid_decimal;
            assert_eq!(
                encode_proof_artifact(&invalid),
                Err(ProverError::ProofArtifactInvalid)
            );
        }

        for invalid_hex in [
            String::new(),
            "00".repeat(PLONK_ENCODED_PROOF_BYTES - 1),
            "00".repeat(PLONK_ENCODED_PROOF_BYTES + 1),
            "A0".repeat(PLONK_ENCODED_PROOF_BYTES),
            "gg".repeat(PLONK_ENCODED_PROOF_BYTES),
        ] {
            let mut invalid = plonk_bundle();
            let SP1Proof::Plonk(plonk) = &mut invalid.proof else {
                unreachable!()
            };
            plonk.encoded_proof = invalid_hex;
            assert_eq!(
                encode_proof_artifact(&invalid),
                Err(ProverError::ProofArtifactInvalid)
            );
        }
    }

    #[test]
    fn proof_artifact_accepts_the_largest_bn254_scalar() {
        let mut proof = plonk_bundle();
        let SP1Proof::Plonk(plonk) = &mut proof.proof else {
            unreachable!()
        };
        plonk.public_inputs[0] =
            "21888242871839275222246405745257275088548364400416034343698204186575808495616"
                .to_owned();

        assert!(encode_proof_artifact(&proof).is_ok());
    }

    #[test]
    fn wrapper_key_hash_matches_the_tracked_setup_file() {
        let tracked_hash = include_str!("../../../tools/sp1-plonk-files.sha256")
            .lines()
            .find_map(|line| line.strip_suffix("  plonk_vk.bin"))
            .expect("tracked wrapper-key hash must exist");
        let encoded = PINNED_PLONK_VKEY_SHA256
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();

        assert_eq!(encoded, tracked_hash);
    }

    #[test]
    fn proof_artifact_rejects_inner_trailing_bytes_and_oversized_lengths() {
        let encoded = encode_proof_artifact(&plonk_bundle()).unwrap();
        let payload_length_offset = ARTIFACT_MAGIC.len()
            + 1
            + encoded_bytes_len(SP1_SDK_VERSION.len()).unwrap()
            + encoded_bytes_len(SP1_CIRCUIT_VERSION.len()).unwrap();
        let payload_start = payload_length_offset + 4;
        let payload_length = u32::from_be_bytes(
            encoded[payload_length_offset..payload_start]
                .try_into()
                .unwrap(),
        );

        let mut inner_trailing = encoded.clone();
        inner_trailing[payload_length_offset..payload_start]
            .copy_from_slice(&(payload_length + 1).to_be_bytes());
        inner_trailing.push(0);
        assert!(matches!(
            decode_proof_artifact(&inner_trailing),
            Err(ProverError::ProofArtifactInvalid)
        ));

        let mut oversized_inner_length = encoded;
        let payload = &mut oversized_inner_length[payload_start..];
        let journal_prefix = 7_u64.to_le_bytes();
        let public_values_length_offset = payload
            .windows(journal_prefix.len() + b"journal".len())
            .position(|window| window.starts_with(&journal_prefix) && window.ends_with(b"journal"))
            .expect("serialized public-value length must be present");
        payload[public_values_length_offset..public_values_length_offset + 8]
            .copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(matches!(
            decode_proof_artifact(&oversized_inner_length),
            Err(ProverError::ProofArtifactInvalid)
        ));
    }

    #[test]
    fn proof_artifact_rejects_oversized_input_before_decoding() {
        let oversized = vec![0; MAX_PROOF_ARTIFACT_BYTES + 1];
        assert!(matches!(
            decode_proof_artifact(&oversized),
            Err(ProverError::ProofArtifactTooLarge)
        ));
    }
}

#[cfg(test)]
mod verification_tests {
    use super::*;

    fn compatible_artifact() -> Vec<u8> {
        let mut proof = SP1Proof::Plonk(Default::default());
        let SP1Proof::Plonk(plonk) = &mut proof else {
            unreachable!()
        };
        plonk.public_inputs = std::array::from_fn(|index| index.to_string());
        plonk.encoded_proof = "00".repeat(PLONK_ENCODED_PROOF_BYTES);
        plonk.plonk_vkey_hash = PINNED_PLONK_VKEY_SHA256;
        let payload = CompatibleProofPayload {
            network_proof: ProofFromNetwork {
                proof,
                public_values: Default::default(),
                sp1_version: SP1_CIRCUIT_VERSION.to_owned(),
            },
            tee_proof: None,
        };
        frame_payload(&proof_payload_options().serialize(&payload).unwrap()).unwrap()
    }

    #[test]
    fn verification_decoder_extracts_only_public_plonk_inputs() {
        let decoded = decode_plonk_artifact(&compatible_artifact()).unwrap();
        assert!(decoded.public_values.is_empty());
        assert_eq!(decoded.public_inputs, ["0", "1", "2", "3", "4"]);
        assert_eq!(decoded.proof_bytes.len(), 4 + PLONK_ENCODED_PROOF_BYTES);
        assert_eq!(&decoded.proof_bytes[..4], &PINNED_PLONK_VKEY_SHA256[..4]);
    }

    #[test]
    fn verification_decoder_rejects_noncanonical_payloads() {
        let mut artifact = compatible_artifact();
        artifact.push(0);
        assert!(matches!(
            decode_plonk_artifact(&artifact),
            Err(ProverError::ProofArtifactInvalid)
        ));
    }
}
