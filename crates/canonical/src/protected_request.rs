use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::{CanonicalEncoder, framed_message};

pub const PROTECTED_REQUEST_FORMAT_VERSION: u32 = 1;
pub const PROTECTED_REQUEST_SIGNATURE_DOMAIN: &[u8] = b"raziel/protected-request/v1";
pub const MAX_CANONICAL_REQUEST_TARGET_BYTES: usize = 2_048;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtectedRequestCanonicalError {
    UnsupportedFormatVersion,
    InvalidCapabilityId,
    InvalidMethod,
    InvalidTarget,
    NonCanonicalMethod,
    NonCanonicalTarget,
    InvalidTimestamp,
    LengthOverflow,
}

impl Display for ProtectedRequestCanonicalError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedFormatVersion => "unsupported protected-request version",
            Self::InvalidCapabilityId => "invalid protected-request capability identifier",
            Self::InvalidMethod => "invalid protected-request method",
            Self::InvalidTarget => "invalid protected-request target",
            Self::NonCanonicalMethod => "protected-request method is not canonical",
            Self::NonCanonicalTarget => "protected-request target is not canonical",
            Self::InvalidTimestamp => "invalid protected-request timestamp",
            Self::LengthOverflow => "protected-request canonical value is too large",
        })
    }
}

impl Error for ProtectedRequestCanonicalError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtectedRequestSignatureInputV1<'a> {
    pub format_version: u32,
    pub capability_id: &'a str,
    pub request_method: &'a str,
    pub canonical_request_target: &'a str,
    pub request_body_hash: &'a [u8; 32],
    pub request_nonce: &'a [u8; 32],
    pub request_timestamp_unix_seconds: i64,
}

pub fn protected_request_signature_message(
    input: &ProtectedRequestSignatureInputV1<'_>,
) -> Result<Vec<u8>, ProtectedRequestCanonicalError> {
    if input.format_version != PROTECTED_REQUEST_FORMAT_VERSION {
        return Err(ProtectedRequestCanonicalError::UnsupportedFormatVersion);
    }
    if !valid_capability_id(input.capability_id) {
        return Err(ProtectedRequestCanonicalError::InvalidCapabilityId);
    }
    if canonicalize_request_method(input.request_method)? != input.request_method {
        return Err(ProtectedRequestCanonicalError::NonCanonicalMethod);
    }
    if canonicalize_request_target(input.canonical_request_target)?
        != input.canonical_request_target
    {
        return Err(ProtectedRequestCanonicalError::NonCanonicalTarget);
    }
    if input.request_timestamp_unix_seconds < 0 {
        return Err(ProtectedRequestCanonicalError::InvalidTimestamp);
    }

    let mut payload = CanonicalEncoder::new();
    payload.write_u32(input.format_version);
    payload
        .write_str(input.capability_id)
        .map_err(|_| ProtectedRequestCanonicalError::LengthOverflow)?;
    payload
        .write_str(input.request_method)
        .map_err(|_| ProtectedRequestCanonicalError::LengthOverflow)?;
    payload
        .write_str(input.canonical_request_target)
        .map_err(|_| ProtectedRequestCanonicalError::LengthOverflow)?;
    payload
        .write_bytes(input.request_body_hash)
        .map_err(|_| ProtectedRequestCanonicalError::LengthOverflow)?;
    payload
        .write_bytes(input.request_nonce)
        .map_err(|_| ProtectedRequestCanonicalError::LengthOverflow)?;
    payload.write_i64(input.request_timestamp_unix_seconds);

    framed_message(PROTECTED_REQUEST_SIGNATURE_DOMAIN, &payload.finish())
        .map_err(|_| ProtectedRequestCanonicalError::LengthOverflow)
}

pub fn canonicalize_request_method(value: &str) -> Result<String, ProtectedRequestCanonicalError> {
    if !value.is_ascii() {
        return Err(ProtectedRequestCanonicalError::InvalidMethod);
    }
    let canonical = value.to_ascii_uppercase();
    if matches!(
        canonical.as_str(),
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE"
    ) {
        Ok(canonical)
    } else {
        Err(ProtectedRequestCanonicalError::InvalidMethod)
    }
}

pub fn canonicalize_request_target(value: &str) -> Result<String, ProtectedRequestCanonicalError> {
    if value.is_empty()
        || value.len() > MAX_CANONICAL_REQUEST_TARGET_BYTES
        || !value.is_ascii()
        || value.contains('#')
    {
        return Err(ProtectedRequestCanonicalError::InvalidTarget);
    }

    let (path, query) = match value.split_once('?') {
        Some((path, query)) if !query.is_empty() && !query.contains('?') => (path, Some(query)),
        Some(_) => return Err(ProtectedRequestCanonicalError::InvalidTarget),
        None => (value, None),
    };
    let path = canonicalize_path(path)?;
    let query = query.map(canonicalize_query).transpose()?;

    let mut canonical = path;
    if let Some(query) = query {
        canonical.push('?');
        canonical.push_str(&query);
    }
    if canonical.len() > MAX_CANONICAL_REQUEST_TARGET_BYTES {
        return Err(ProtectedRequestCanonicalError::InvalidTarget);
    }
    Ok(canonical)
}

fn canonicalize_path(path: &str) -> Result<String, ProtectedRequestCanonicalError> {
    if !path.starts_with('/') || (path.len() > 1 && path.ends_with('/')) {
        return Err(ProtectedRequestCanonicalError::InvalidTarget);
    }
    if path == "/" {
        return Ok(path.to_owned());
    }

    let mut canonical = String::with_capacity(path.len());
    canonical.push('/');
    let mut first = true;
    for segment in path[1..].split('/') {
        if segment.is_empty() {
            return Err(ProtectedRequestCanonicalError::InvalidTarget);
        }
        let segment = canonicalize_component(segment, ComponentKind::Path)?;
        if matches!(segment.as_str(), "." | "..") {
            return Err(ProtectedRequestCanonicalError::InvalidTarget);
        }
        if !first {
            canonical.push('/');
        }
        first = false;
        canonical.push_str(&segment);
    }
    Ok(canonical)
}

fn canonicalize_query(query: &str) -> Result<String, ProtectedRequestCanonicalError> {
    let mut entries = BTreeMap::new();
    for entry in query.split('&') {
        if entry.is_empty() {
            return Err(ProtectedRequestCanonicalError::InvalidTarget);
        }
        let (key, value) = entry.split_once('=').unwrap_or((entry, ""));
        if key.is_empty() {
            return Err(ProtectedRequestCanonicalError::InvalidTarget);
        }
        let key = canonicalize_component(key, ComponentKind::Query)?;
        let value = canonicalize_component(value, ComponentKind::Query)?;
        if entries.insert(key, value).is_some() {
            return Err(ProtectedRequestCanonicalError::InvalidTarget);
        }
    }

    let mut canonical = String::with_capacity(query.len());
    for (index, (key, value)) in entries.into_iter().enumerate() {
        if index != 0 {
            canonical.push('&');
        }
        canonical.push_str(&key);
        canonical.push('=');
        canonical.push_str(&value);
    }
    Ok(canonical)
}

#[derive(Clone, Copy)]
enum ComponentKind {
    Path,
    Query,
}

fn canonicalize_component(
    value: &str,
    kind: ComponentKind,
) -> Result<String, ProtectedRequestCanonicalError> {
    let bytes = value.as_bytes();
    let mut canonical = String::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if is_unreserved(byte) {
            canonical.push(char::from(byte));
            index += 1;
            continue;
        }
        if matches!(kind, ComponentKind::Query) && byte == b'+' {
            canonical.push_str("%2B");
            index += 1;
            continue;
        }
        if byte != b'%' || index + 2 >= bytes.len() {
            return Err(ProtectedRequestCanonicalError::InvalidTarget);
        }
        let decoded = decode_hex_pair(bytes[index + 1], bytes[index + 2])
            .ok_or(ProtectedRequestCanonicalError::InvalidTarget)?;
        if matches!(kind, ComponentKind::Path) && matches!(decoded, b'/' | b'\\' | b'?' | b'#') {
            return Err(ProtectedRequestCanonicalError::InvalidTarget);
        }
        if is_unreserved(decoded) {
            canonical.push(char::from(decoded));
        } else {
            canonical.push('%');
            canonical.push(hex_digit(decoded >> 4));
            canonical.push(hex_digit(decoded & 0x0f));
        }
        index += 3;
    }
    Ok(canonical)
}

fn valid_capability_id(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(is_lower_hex_digit)
}

const fn is_lower_hex_digit(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')
}

const fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

const fn decode_hex_pair(high: u8, low: u8) -> Option<u8> {
    let Some(high) = decode_hex_digit(high) else {
        return None;
    };
    let Some(low) = decode_hex_digit(low) else {
        return None;
    };
    Some((high << 4) | low)
}

const fn decode_hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn hex_digit(value: u8) -> char {
    char::from(b"0123456789ABCDEF"[usize::from(value)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normative_method_and_target_examples_match_adr_0002() {
        assert_eq!(canonicalize_request_method("post").unwrap(), "POST");
        assert_eq!(
            canonicalize_request_target("/payments/%72econcile?status=pending&account=%61-1")
                .unwrap(),
            "/payments/reconcile?account=a-1&status=pending"
        );
        assert_eq!(
            canonicalize_request_target("/payments?flag&q=a+b").unwrap(),
            "/payments?flag=&q=a%2Bb"
        );
        assert_eq!(
            canonicalize_request_target("/r%C3%A9sum%C3%A9").unwrap(),
            "/r%C3%A9sum%C3%A9"
        );
    }

    #[test]
    fn invalid_adr_0002_targets_fail_closed() {
        for target in [
            "//payments",
            "/payments/",
            "/payments/../admin",
            "/payments/%2fadmin",
            "/payments?a=1&%61=2",
            "/payments?",
        ] {
            assert_eq!(
                canonicalize_request_target(target),
                Err(ProtectedRequestCanonicalError::InvalidTarget),
                "accepted invalid target {target:?}"
            );
        }
    }

    #[test]
    fn already_canonical_signature_fields_are_required() {
        let capability_id = "33".repeat(32);
        let input = ProtectedRequestSignatureInputV1 {
            format_version: PROTECTED_REQUEST_FORMAT_VERSION,
            capability_id: &capability_id,
            request_method: "post",
            canonical_request_target: "/payments/%72econcile",
            request_body_hash: &[0x11; 32],
            request_nonce: &[0x22; 32],
            request_timestamp_unix_seconds: 1_800_000_000,
        };
        assert_eq!(
            protected_request_signature_message(&input),
            Err(ProtectedRequestCanonicalError::NonCanonicalMethod)
        );

        let canonical_method = ProtectedRequestSignatureInputV1 {
            request_method: "POST",
            ..input
        };
        assert_eq!(
            protected_request_signature_message(&canonical_method),
            Err(ProtectedRequestCanonicalError::NonCanonicalTarget)
        );
    }
}
