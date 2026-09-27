use std::io::{self, Cursor, Read, Write};
use std::process::{Command, Output, Stdio};

use raziel_admin::agent_api::{
    AgentAuthError, MAX_AGENT_AUTH_INPUT_BYTES, MAX_AGENT_AUTH_OUTPUT_BYTES, run,
};
use serde_json::{Value, json};

fn vector() -> Value {
    serde_json::from_str(include_str!(
        "../../../tests/vectors/agent-api-request-signature-v1.json"
    ))
    .unwrap()
}

fn validate_command() -> Value {
    json!({
        "operation": "validate_key",
        "agent_public_key": vector()["agent_public_key_hex"],
    })
}

fn verify_command() -> Value {
    let fixture = vector();
    json!({
        "operation": "verify",
        "audience": fixture["audience"],
        "token_id": fixture["token_id"],
        "agent_public_key": fixture["agent_public_key_hex"],
        "request_method": fixture["request_method"],
        "canonical_request_target": fixture["canonical_request_target"],
        "request_body_hash": fixture["request_body_hash_hex"],
        "request_nonce": fixture["request_nonce_hex"],
        "request_timestamp_unix_seconds": fixture["request_timestamp_unix_seconds"],
        "request_signature": fixture["request_signature_hex"],
    })
}

fn assert_rejected(input: &[u8]) {
    let mut output = Vec::new();
    assert_eq!(run(input, &mut output), Err(AgentAuthError::InvalidRequest));
    assert!(output.is_empty());
}

fn invoke(input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_raziel-agent-auth"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn both_operations_return_only_the_fixed_thumbprint() {
    for command in [validate_command(), verify_command()] {
        let input = serde_json::to_vec(&command).unwrap();
        let mut output = Vec::new();
        run(input.as_slice(), &mut output).unwrap();
        assert!(output.len() <= MAX_AGENT_AUTH_OUTPUT_BYTES);
        assert_eq!(
            serde_json::from_slice::<Value>(&output).unwrap(),
            json!({
                "agent_key_thumbprint": vector()["agent_key_thumbprint_hex"]
            })
        );
        let process = invoke(&input);
        assert_eq!(process.status.code(), Some(0));
        assert_eq!(process.stdout, output);
        assert!(process.stderr.is_empty());
    }
}

#[test]
fn json_rejects_unknown_missing_duplicate_and_mistyped_fields() {
    for command in [validate_command(), verify_command()] {
        for field in [
            "unknown",
            "token",
            "private_key",
            "agent_key_thumbprint",
            "format_version",
        ] {
            let mut changed = command.clone();
            changed[field] = json!("SECRET_INPUT_SENTINEL");
            assert_rejected(&serde_json::to_vec(&changed).unwrap());
        }
        for field in command.as_object().unwrap().keys() {
            let mut missing = command.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert_rejected(&serde_json::to_vec(&missing).unwrap());
            for invalid_type in [Value::Null, json!(true), json!(123), json!([]), json!({})] {
                let mut wrong_type = command.clone();
                wrong_type[field] = invalid_type;
                assert_rejected(&serde_json::to_vec(&wrong_type).unwrap());
            }
            let mut duplicate = serde_json::to_string(&command).unwrap();
            duplicate.pop();
            duplicate.push_str(&format!(
                ",{}:{}}}",
                serde_json::to_string(field).unwrap(),
                command[field]
            ));
            assert_rejected(duplicate.as_bytes());
        }
    }
    for input in [
        b"".as_slice(),
        b"null",
        b"[]",
        b"{}",
        b"{",
        b"\xff",
        b"{}{}",
        b"{\"operation\":\"other\"}",
    ] {
        assert_rejected(input);
    }
    let mut trailing = serde_json::to_vec(&validate_command()).unwrap();
    trailing.extend_from_slice(b"{}");
    assert_rejected(&trailing);
}

#[test]
fn helper_rejects_crypto_hex_and_signed_field_mutations() {
    let command = verify_command();
    for (field, bytes) in [
        ("agent_public_key", 32),
        ("request_body_hash", 32),
        ("request_nonce", 32),
        ("request_signature", 64),
    ] {
        for value in [
            String::new(),
            "00".repeat(bytes - 1),
            "00".repeat(bytes + 1),
            "AB".repeat(bytes),
            "gg".repeat(bytes),
            "\u{e9}".repeat(bytes),
            format!(" {}", "00".repeat(bytes)),
        ] {
            let mut changed = command.clone();
            changed[field] = json!(value);
            assert_rejected(&serde_json::to_vec(&changed).unwrap());
        }
    }
    for (field, value) in [
        ("audience", "other"),
        ("audience", ""),
        ("token_id", "1123456789abcdef0123456789abcdef"),
        ("token_id", "0123456789ABCDEF0123456789ABCDEF"),
        ("request_method", "POST"),
        ("request_method", "get"),
        ("canonical_request_target", "/api/agent/v1/access-requests"),
        ("canonical_request_target", "/api/agent/v1/%6de"),
        ("request_timestamp_unix_seconds", "1800000001"),
        ("request_timestamp_unix_seconds", "01800000000"),
        ("request_timestamp_unix_seconds", "253402300800"),
        ("request_timestamp_unix_seconds", "-1"),
        ("request_timestamp_unix_seconds", "+1"),
        ("request_timestamp_unix_seconds", "1.0"),
        ("request_timestamp_unix_seconds", "1e2"),
    ] {
        let mut changed = command.clone();
        changed[field] = json!(value);
        assert_rejected(&serde_json::to_vec(&changed).unwrap());
    }
    for field in [
        "agent_public_key",
        "request_body_hash",
        "request_nonce",
        "request_signature",
    ] {
        let mut changed = command.clone();
        let mut bytes = changed[field].as_str().unwrap().as_bytes().to_vec();
        bytes[0] = if bytes[0] == b'0' { b'1' } else { b'0' };
        changed[field] = json!(String::from_utf8(bytes).unwrap());
        assert_rejected(&serde_json::to_vec(&changed).unwrap());
    }
    for key in [
        "00".repeat(32),
        format!("01{}", "00".repeat(31)),
        format!("02{}", "00".repeat(31)),
    ] {
        let mut changed = validate_command();
        changed["agent_public_key"] = json!(key);
        assert_rejected(&serde_json::to_vec(&changed).unwrap());
    }
}

#[test]
fn input_limit_accepts_exactly_8192_and_stops_after_one_overflow_byte() {
    let mut input = serde_json::to_vec(&validate_command()).unwrap();
    input.resize(MAX_AGENT_AUTH_INPUT_BYTES, b' ');
    assert!(run(input.as_slice(), Vec::new()).is_ok());
    assert_eq!(invoke(&input).status.code(), Some(0));
    input.push(b' ');
    assert_rejected(&input);
    assert_eq!(invoke(&input).status.code(), Some(2));
    input.resize(100_000, b' ');
    let mut reader = Cursor::new(input);
    assert_eq!(
        run(&mut reader, Vec::new()),
        Err(AgentAuthError::InvalidRequest)
    );
    assert_eq!(reader.position(), (MAX_AGENT_AUTH_INPUT_BYTES + 1) as u64);
}

#[test]
fn invalid_cli_requests_and_arguments_exit_two_silently() {
    let mut invalid_key = validate_command();
    invalid_key["agent_public_key"] = json!("00".repeat(32));
    let mut invalid_signature = verify_command();
    invalid_signature["request_signature"] = json!("00".repeat(64));
    for input in [
        br#"{"operation":"verify","token":"SECRET_INPUT_SENTINEL"}"#.to_vec(),
        serde_json::to_vec(&invalid_key).unwrap(),
        serde_json::to_vec(&invalid_signature).unwrap(),
        Vec::new(),
        vec![b' '; MAX_AGENT_AUTH_INPUT_BYTES + 1],
    ] {
        let process = invoke(&input);
        assert_eq!(process.status.code(), Some(2));
        assert!(process.stdout.is_empty());
        assert!(process.stderr.is_empty());
    }
    let process = Command::new(env!("CARGO_BIN_EXE_raziel-agent-auth"))
        .arg("SECRET_INPUT_SENTINEL")
        .output()
        .unwrap();
    assert_eq!(process.status.code(), Some(2));
    assert!(process.stdout.is_empty());
    assert!(process.stderr.is_empty());
}

#[test]
fn io_errors_are_internal_and_never_include_source_errors() {
    struct BrokenReader;
    impl Read for BrokenReader {
        fn read(&mut self, _bytes: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("SECRET_INPUT_SENTINEL"))
        }
    }
    struct BrokenWriter;
    impl Write for BrokenWriter {
        fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("SECRET_INPUT_SENTINEL"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    struct BrokenFlush;
    impl Write for BrokenFlush {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("SECRET_INPUT_SENTINEL"))
        }
    }
    assert_eq!(run(BrokenReader, Vec::new()), Err(AgentAuthError::Internal));
    let input = serde_json::to_vec(&validate_command()).unwrap();
    assert_eq!(
        run(input.as_slice(), BrokenWriter),
        Err(AgentAuthError::Internal)
    );
    assert_eq!(
        run(input.as_slice(), BrokenFlush),
        Err(AgentAuthError::Internal)
    );
    assert_eq!(AgentAuthError::Internal.exit_code(), 1);
    assert_eq!(
        AgentAuthError::Internal.to_string(),
        "authentication helper failed"
    );
}

#[cfg(unix)]
#[test]
fn cli_stdin_io_error_exits_one() {
    let directory = std::fs::File::open(std::env::temp_dir()).unwrap();
    let process = Command::new(env!("CARGO_BIN_EXE_raziel-agent-auth"))
        .stdin(Stdio::from(directory))
        .output()
        .unwrap();
    assert_eq!(process.status.code(), Some(1));
    assert!(process.stdout.is_empty());
    assert_eq!(
        process.stderr,
        b"raziel-agent-auth: authentication helper failed\n"
    );
}
