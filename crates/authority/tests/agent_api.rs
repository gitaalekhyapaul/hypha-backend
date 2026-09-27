use ed25519_dalek::{Signer, SigningKey};
use raziel_authority::{
    AgentApiVerificationError, validate_agent_api_key, verify_agent_api_request,
};
use raziel_canonical::{
    AgentApiRequestSignatureInputV1, MAX_AGENT_API_TIMESTAMP_UNIX_SECONDS,
    agent_api_request_signature_message,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

struct Fixture {
    vector: Value,
    seed: [u8; 32],
    public_key: [u8; 32],
    thumbprint: [u8; 32],
    body_hash: [u8; 32],
    nonce: [u8; 32],
    signature: [u8; 64],
}

impl Fixture {
    fn new() -> Self {
        let vector: Value = serde_json::from_str(include_str!(
            "../../../tests/vectors/agent-api-request-signature-v1.json"
        ))
        .unwrap();
        Self {
            seed: decode_hex(vector["seed_hex"].as_str().unwrap()),
            public_key: decode_hex(vector["agent_public_key_hex"].as_str().unwrap()),
            thumbprint: decode_hex(vector["agent_key_thumbprint_hex"].as_str().unwrap()),
            body_hash: decode_hex(vector["request_body_hash_hex"].as_str().unwrap()),
            nonce: decode_hex(vector["request_nonce_hex"].as_str().unwrap()),
            signature: decode_hex(vector["request_signature_hex"].as_str().unwrap()),
            vector,
        }
    }

    fn input(&self) -> AgentApiRequestSignatureInputV1<'_> {
        AgentApiRequestSignatureInputV1 {
            format_version: 1,
            audience: self.vector["audience"].as_str().unwrap(),
            token_id: self.vector["token_id"].as_str().unwrap(),
            agent_key_thumbprint: &self.thumbprint,
            request_method: self.vector["request_method"].as_str().unwrap(),
            canonical_request_target: self.vector["canonical_request_target"].as_str().unwrap(),
            request_body_hash: &self.body_hash,
            request_nonce: &self.nonce,
            request_timestamp_unix_seconds: self.vector["request_timestamp_unix_seconds"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap(),
        }
    }
}

#[test]
fn fixed_node_signature_and_rust_signature_match() {
    let fixture = Fixture::new();
    let key = SigningKey::from_bytes(&fixture.seed);
    assert_eq!(key.verifying_key().to_bytes(), fixture.public_key);
    assert_eq!(
        validate_agent_api_key(&fixture.public_key),
        Ok(fixture.thumbprint)
    );
    let message = agent_api_request_signature_message(&fixture.input()).unwrap();
    assert_eq!(key.sign(&message).to_bytes(), fixture.signature);
    assert_eq!(
        verify_agent_api_request(&fixture.public_key, &fixture.input(), &fixture.signature),
        Ok(fixture.thumbprint)
    );
}

#[test]
fn every_signed_field_is_bound() {
    let fixture = Fixture::new();
    let input = fixture.input();
    let original = agent_api_request_signature_message(&input).unwrap();
    let mutations = [
        AgentApiRequestSignatureInputV1 {
            format_version: 2,
            ..input
        },
        AgentApiRequestSignatureInputV1 {
            audience: "other-installation",
            ..input
        },
        AgentApiRequestSignatureInputV1 {
            token_id: "1123456789abcdef0123456789abcdef",
            ..input
        },
        AgentApiRequestSignatureInputV1 {
            agent_key_thumbprint: &[0x44; 32],
            ..input
        },
        AgentApiRequestSignatureInputV1 {
            request_method: "POST",
            ..input
        },
        AgentApiRequestSignatureInputV1 {
            canonical_request_target: "/api/agent/v1/access-requests",
            ..input
        },
        AgentApiRequestSignatureInputV1 {
            canonical_request_target: "/api/agent/v1/me?a=1",
            ..input
        },
        AgentApiRequestSignatureInputV1 {
            canonical_request_target: "/api/agent/v1/me?b=1",
            ..input
        },
        AgentApiRequestSignatureInputV1 {
            canonical_request_target: "/api/agent/v1/me?a=2",
            ..input
        },
        AgentApiRequestSignatureInputV1 {
            request_body_hash: &[0x55; 32],
            ..input
        },
        AgentApiRequestSignatureInputV1 {
            request_nonce: &[0x66; 32],
            ..input
        },
        AgentApiRequestSignatureInputV1 {
            request_timestamp_unix_seconds: input.request_timestamp_unix_seconds + 1,
            ..input
        },
    ];
    for mutation in mutations {
        if let Ok(message) = agent_api_request_signature_message(&mutation) {
            assert_ne!(message, original);
        }
        assert!(
            verify_agent_api_request(&fixture.public_key, &mutation, &fixture.signature).is_err()
        );
    }
    let other_key = SigningKey::from_bytes(&[0x42; 32])
        .verifying_key()
        .to_bytes();
    let other_thumbprint = validate_agent_api_key(&other_key).unwrap();
    assert_eq!(
        verify_agent_api_request(&other_key, &input, &fixture.signature),
        Err(AgentApiVerificationError::KeyThumbprintMismatch)
    );
    assert_eq!(
        verify_agent_api_request(
            &other_key,
            &AgentApiRequestSignatureInputV1 {
                agent_key_thumbprint: &other_thumbprint,
                ..input
            },
            &fixture.signature
        ),
        Err(AgentApiVerificationError::InvalidSignature)
    );
}

#[test]
fn malformed_and_weak_keys_are_rejected_without_input_in_errors() {
    for key in [vec![], vec![0; 31], vec![0; 33], vec![0; 1_024]] {
        assert_eq!(
            validate_agent_api_key(&key),
            Err(AgentApiVerificationError::InvalidPublicKeyLength)
        );
    }
    // y = 2 is not a decompressible Edwards point.
    let mut malformed = [0; 32];
    malformed[0] = 2;
    assert_eq!(
        validate_agent_api_key(&malformed),
        Err(AgentApiVerificationError::InvalidPublicKey)
    );
    let mut identity = [0; 32];
    identity[0] = 1;
    for weak in [[0; 32], identity] {
        assert_eq!(
            validate_agent_api_key(&weak),
            Err(AgentApiVerificationError::WeakPublicKey)
        );
        assert_eq!(
            verify_agent_api_request(&weak, &Fixture::new().input(), &[0; 64]),
            Err(AgentApiVerificationError::WeakPublicKey)
        );
    }
    let errors = [
        AgentApiVerificationError::InvalidPublicKey,
        AgentApiVerificationError::InvalidSignature,
    ];
    for error in errors {
        assert!(!format!("{error} {error:?}").contains("d75a9801"));
    }
}

#[test]
fn noncanonical_compressed_keys_cannot_register_an_alias_identity() {
    let mut parsed_nonweak = 0;
    // Encodings of y >= 2^255 - 19 are not canonical RFC 8032 field elements.
    for low_byte in 0xed..=0xff {
        let mut encoded = [0xff; 32];
        encoded[0] = low_byte;
        encoded[31] = 0x7f;
        if let Ok(key) = ed25519_dalek::VerifyingKey::from_bytes(&encoded)
            && !key.is_weak()
        {
            parsed_nonweak += 1;
        }
        assert_eq!(
            validate_agent_api_key(&encoded),
            Err(AgentApiVerificationError::InvalidPublicKey)
        );
    }
    assert!(parsed_nonweak > 0);
}

#[test]
fn strict_verification_rejects_bad_lengths_malleability_and_wrong_domains() {
    let fixture = Fixture::new();
    let input = fixture.input();
    for signature in [vec![], vec![0; 63], vec![0; 65]] {
        assert_eq!(
            verify_agent_api_request(&fixture.public_key, &input, &signature),
            Err(AgentApiVerificationError::InvalidSignatureLength)
        );
    }
    for index in 0..fixture.signature.len() {
        let mut changed = fixture.signature;
        changed[index] ^= 1;
        assert_eq!(
            verify_agent_api_request(&fixture.public_key, &input, &changed),
            Err(AgentApiVerificationError::InvalidSignature)
        );
    }
    let mut noncanonical_s = fixture.signature;
    noncanonical_s[32..].fill(0xff);
    let mut weak_r = fixture.signature;
    weak_r[..32].fill(0);
    weak_r[0] = 1;
    for invalid in [noncanonical_s, weak_r, [0; 64]] {
        assert_eq!(
            verify_agent_api_request(&fixture.public_key, &input, &invalid),
            Err(AgentApiVerificationError::InvalidSignature)
        );
    }
    let key = SigningKey::from_bytes(&fixture.seed);
    let message = agent_api_request_signature_message(&input).unwrap();
    let payload_offset = 4 + b"raziel/agent-api-request/v1".len() + 4;
    let wrong_domain = raziel_canonical::framed_message(
        b"raziel/protected-request/v1",
        &message[payload_offset..],
    )
    .unwrap();
    for other_message in [
        wrong_domain,
        Sha256::digest(&message).to_vec(),
        message[payload_offset..].to_vec(),
    ] {
        assert_eq!(
            verify_agent_api_request(
                &fixture.public_key,
                &input,
                &key.sign(&other_message).to_bytes()
            ),
            Err(AgentApiVerificationError::InvalidSignature)
        );
    }
}

#[test]
fn valid_edge_timestamps_do_not_consult_a_clock_or_replay_store() {
    let fixture = Fixture::new();
    let key = SigningKey::from_bytes(&fixture.seed);
    for timestamp in [0, MAX_AGENT_API_TIMESTAMP_UNIX_SECONDS] {
        let input = AgentApiRequestSignatureInputV1 {
            request_timestamp_unix_seconds: timestamp,
            ..fixture.input()
        };
        let signature = key
            .sign(&agent_api_request_signature_message(&input).unwrap())
            .to_bytes();
        for _ in 0..2 {
            assert_eq!(
                verify_agent_api_request(&fixture.public_key, &input, &signature),
                Ok(fixture.thumbprint)
            );
        }
    }
}

fn decode_hex<const N: usize>(value: &str) -> [u8; N] {
    assert_eq!(value.len(), N * 2);
    let mut output = [0; N];
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap();
    }
    output
}
