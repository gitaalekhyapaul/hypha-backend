use raziel_canonical::{
    AGENT_API_REQUEST_SIGNATURE_DOMAIN, AgentApiCanonicalError, AgentApiRequestSignatureInputV1,
    MAX_AGENT_API_TIMESTAMP_UNIX_SECONDS, ProtectedRequestCanonicalError,
    agent_api_request_signature_message,
};
use sha2::{Digest, Sha256};

fn input() -> AgentApiRequestSignatureInputV1<'static> {
    AgentApiRequestSignatureInputV1 {
        format_version: 1,
        audience: "raziel-test",
        token_id: "0123456789abcdef0123456789abcdef",
        agent_key_thumbprint: &[0x11; 32],
        request_method: "GET",
        canonical_request_target: "/api/agent/v1/me",
        request_body_hash: &[0x22; 32],
        request_nonce: &[0x33; 32],
        request_timestamp_unix_seconds: 1_800_000_000,
    }
}

#[test]
fn agent_api_message_matches_the_cross_language_vector() {
    let vector: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/vectors/agent-api-request-signature-v1.json"
    ))
    .unwrap();
    let text = |name: &str| vector[name].as_str().unwrap();
    let thumbprint = decode_hex(text("agent_key_thumbprint_hex"))
        .try_into()
        .unwrap();
    let body_hash = decode_hex(text("request_body_hash_hex"))
        .try_into()
        .unwrap();
    let nonce = decode_hex(text("request_nonce_hex")).try_into().unwrap();
    assert_eq!(
        AGENT_API_REQUEST_SIGNATURE_DOMAIN,
        text("signature_domain").as_bytes()
    );
    assert_eq!(
        Sha256::digest(decode_hex(text("request_body_hex"))).as_slice(),
        body_hash
    );
    let message = agent_api_request_signature_message(&AgentApiRequestSignatureInputV1 {
        format_version: vector["format_version"]
            .as_u64()
            .unwrap()
            .try_into()
            .unwrap(),
        audience: text("audience"),
        token_id: text("token_id"),
        agent_key_thumbprint: &thumbprint,
        request_method: text("request_method"),
        canonical_request_target: text("canonical_request_target"),
        request_body_hash: &body_hash,
        request_nonce: &nonce,
        request_timestamp_unix_seconds: text("request_timestamp_unix_seconds").parse().unwrap(),
    })
    .unwrap();
    assert_eq!(message, decode_hex(text("signature_message_hex")));
}

#[test]
fn rejects_unsupported_versions_and_invalid_audience_or_token_id() {
    for version in [0, 2, u32::MAX] {
        assert_eq!(
            agent_api_request_signature_message(&AgentApiRequestSignatureInputV1 {
                format_version: version,
                ..input()
            }),
            Err(AgentApiCanonicalError::UnsupportedFormatVersion)
        );
    }
    for audience in ["", "\0", "\n", "a\x7fb", "\u{e9}", &"a".repeat(129)] {
        assert_eq!(
            agent_api_request_signature_message(&AgentApiRequestSignatureInputV1 {
                audience,
                ..input()
            }),
            Err(AgentApiCanonicalError::InvalidAudience)
        );
    }
    for token_id in [
        "",
        &"a".repeat(31),
        &"a".repeat(33),
        &"A".repeat(32),
        &"g".repeat(32),
        &"\u{e9}".repeat(16),
    ] {
        assert_eq!(
            agent_api_request_signature_message(&AgentApiRequestSignatureInputV1 {
                token_id,
                ..input()
            }),
            Err(AgentApiCanonicalError::InvalidTokenId)
        );
    }
    for audience in [" ", "~", &"a".repeat(128)] {
        assert!(
            agent_api_request_signature_message(&AgentApiRequestSignatureInputV1 {
                audience,
                ..input()
            })
            .is_ok()
        );
    }
    let exact = agent_api_request_signature_message(&AgentApiRequestSignatureInputV1 {
        audience: " raziel-test ",
        ..input()
    })
    .unwrap();
    assert_ne!(
        exact,
        agent_api_request_signature_message(&input()).unwrap()
    );
}

#[test]
fn method_and_target_validation_reuses_adr_0002_without_normalizing() {
    for method in ["GET", "POST", "PUT", "PATCH", "DELETE"] {
        assert!(
            agent_api_request_signature_message(&AgentApiRequestSignatureInputV1 {
                request_method: method,
                ..input()
            })
            .is_ok()
        );
    }
    for method in ["get", "HEAD", "OPTIONS", "GET ", "", "G\u{c9}T"] {
        assert!(
            agent_api_request_signature_message(&AgentApiRequestSignatureInputV1 {
                request_method: method,
                ..input()
            })
            .is_err()
        );
    }
    for target in [
        "",
        "https://example.test/me",
        "//me",
        "/me/",
        "/a/../b",
        "/a/%2E/b",
        "/a/%2f/b",
        "/me?a=1&%61=2",
        "/me?",
        "/me#x",
        "/me?a=1&&b=2",
        "/\u{e9}",
        "/m e",
        &format!("/{}", "a".repeat(2_048)),
    ] {
        assert!(
            agent_api_request_signature_message(&AgentApiRequestSignatureInputV1 {
                canonical_request_target: target,
                ..input()
            })
            .is_err()
        );
    }
    for target in ["/%6de", "/me?b=2&a=1", "/me?flag", "/me?q=a+b", "/r%c3%a9"] {
        assert_eq!(
            agent_api_request_signature_message(&AgentApiRequestSignatureInputV1 {
                canonical_request_target: target,
                ..input()
            }),
            Err(AgentApiCanonicalError::Request(
                ProtectedRequestCanonicalError::NonCanonicalTarget
            ))
        );
    }
    for target in [
        "/",
        "/me?a=1&b=2",
        "/me?flag=&q=a%2Bb",
        "/r%C3%A9",
        &format!("/{}", "a".repeat(2_047)),
    ] {
        assert!(
            agent_api_request_signature_message(&AgentApiRequestSignatureInputV1 {
                canonical_request_target: target,
                ..input()
            })
            .is_ok()
        );
    }
}

#[test]
fn timestamp_edges_are_exact_signed_i64_values_not_freshness_checks() {
    for timestamp in [0, 1, MAX_AGENT_API_TIMESTAMP_UNIX_SECONDS] {
        let message = agent_api_request_signature_message(&AgentApiRequestSignatureInputV1 {
            request_timestamp_unix_seconds: timestamp,
            ..input()
        })
        .unwrap();
        assert_eq!(&message[message.len() - 8..], timestamp.to_be_bytes());
    }
    for timestamp in [
        i64::MIN,
        -1,
        MAX_AGENT_API_TIMESTAMP_UNIX_SECONDS + 1,
        i64::MAX,
    ] {
        assert_eq!(
            agent_api_request_signature_message(&AgentApiRequestSignatureInputV1 {
                request_timestamp_unix_seconds: timestamp,
                ..input()
            }),
            Err(AgentApiCanonicalError::InvalidTimestamp)
        );
    }
}

fn decode_hex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
