use raziel_canonical::CanonicalEncoder;
use raziel_snapshot::Digest;

use crate::{AUTHORIZATION_PUBLIC_VALUES_VERSION, AuthorizationError, AuthorizationPublicValuesV1};

pub const AUTHORIZATION_PUBLIC_VALUES_DOMAIN: &[u8] = b"raziel/authorization-public-values/v1";
const ALLOW_DECISION_TAG: u8 = 0x01;

pub fn encode_public_values(
    values: &AuthorizationPublicValuesV1,
) -> Result<Vec<u8>, AuthorizationError> {
    if values.format_version != AUTHORIZATION_PUBLIC_VALUES_VERSION
        || values.issued_at_unix_seconds < 0
        || values.issued_at_unix_seconds >= values.expires_at_unix_seconds
    {
        return Err(AuthorizationError::InvalidPublicValues);
    }

    let mut encoder = CanonicalEncoder::new();
    encoder
        .write_bytes(AUTHORIZATION_PUBLIC_VALUES_DOMAIN)
        .map_err(|_| AuthorizationError::CanonicalEncoding)?;
    encoder.write_u32(values.format_version);
    write_digest(&mut encoder, &values.policy_hash)?;
    write_digest(&mut encoder, &values.relationship_snapshot_root)?;
    write_digest(&mut encoder, &values.capability_request_commitment)?;
    encoder.write_u8(ALLOW_DECISION_TAG);
    write_digest(&mut encoder, &values.capability_scope_hash)?;
    encoder.write_i64(values.issued_at_unix_seconds);
    encoder.write_i64(values.expires_at_unix_seconds);
    write_digest(&mut encoder, &values.issuance_nullifier)?;
    Ok(encoder.finish())
}

pub fn decode_public_values(
    bytes: &[u8],
) -> Result<AuthorizationPublicValuesV1, AuthorizationError> {
    let mut decoder = Decoder::new(bytes);
    if decoder.read_bytes()? != AUTHORIZATION_PUBLIC_VALUES_DOMAIN {
        return Err(AuthorizationError::InvalidPublicValues);
    }
    let format_version = decoder.read_u32()?;
    if format_version != AUTHORIZATION_PUBLIC_VALUES_VERSION {
        return Err(AuthorizationError::InvalidPublicValues);
    }
    let policy_hash = decoder.read_digest()?;
    let relationship_snapshot_root = decoder.read_digest()?;
    let capability_request_commitment = decoder.read_digest()?;
    if decoder.read_u8()? != ALLOW_DECISION_TAG {
        return Err(AuthorizationError::InvalidPublicValues);
    }
    let capability_scope_hash = decoder.read_digest()?;
    let issued_at_unix_seconds = decoder.read_i64()?;
    let expires_at_unix_seconds = decoder.read_i64()?;
    let issuance_nullifier = decoder.read_digest()?;
    decoder.require_eof()?;

    let values = AuthorizationPublicValuesV1 {
        format_version,
        policy_hash,
        relationship_snapshot_root,
        capability_request_commitment,
        capability_scope_hash,
        issued_at_unix_seconds,
        expires_at_unix_seconds,
        issuance_nullifier,
    };
    if values.issued_at_unix_seconds < 0
        || values.issued_at_unix_seconds >= values.expires_at_unix_seconds
    {
        return Err(AuthorizationError::InvalidPublicValues);
    }
    Ok(values)
}

fn write_digest(encoder: &mut CanonicalEncoder, digest: &Digest) -> Result<(), AuthorizationError> {
    encoder
        .write_bytes(digest)
        .map_err(|_| AuthorizationError::CanonicalEncoding)
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_u8(&mut self) -> Result<u8, AuthorizationError> {
        let value = *self
            .bytes
            .get(self.offset)
            .ok_or(AuthorizationError::InvalidPublicValues)?;
        self.offset += 1;
        Ok(value)
    }

    fn read_u32(&mut self) -> Result<u32, AuthorizationError> {
        Ok(u32::from_be_bytes(self.read_array()?))
    }

    fn read_i64(&mut self) -> Result<i64, AuthorizationError> {
        Ok(i64::from_be_bytes(self.read_array()?))
    }

    fn read_bytes(&mut self) -> Result<&'a [u8], AuthorizationError> {
        let length = usize::try_from(self.read_u32()?)
            .map_err(|_| AuthorizationError::InvalidPublicValues)?;
        self.take(length)
    }

    fn read_digest(&mut self) -> Result<Digest, AuthorizationError> {
        self.read_bytes()?
            .try_into()
            .map_err(|_| AuthorizationError::InvalidPublicValues)
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], AuthorizationError> {
        self.take(N)?
            .try_into()
            .map_err(|_| AuthorizationError::InvalidPublicValues)
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], AuthorizationError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(AuthorizationError::InvalidPublicValues)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(AuthorizationError::InvalidPublicValues)?;
        self.offset = end;
        Ok(value)
    }

    fn require_eof(&self) -> Result<(), AuthorizationError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(AuthorizationError::InvalidPublicValues)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> AuthorizationPublicValuesV1 {
        AuthorizationPublicValuesV1 {
            format_version: 1,
            policy_hash: [0x11; 32],
            relationship_snapshot_root: [0x22; 32],
            capability_request_commitment: [0x33; 32],
            capability_scope_hash: [0x44; 32],
            issued_at_unix_seconds: 10,
            expires_at_unix_seconds: 20,
            issuance_nullifier: [0x55; 32],
        }
    }

    #[test]
    fn public_values_round_trip_exactly() {
        let values = values();
        let encoded = encode_public_values(&values).unwrap();
        assert_eq!(decode_public_values(&encoded).unwrap(), values);
    }

    #[test]
    fn public_values_reject_mutation_and_trailing_bytes() {
        let encoded = encode_public_values(&values()).unwrap();
        for index in 0..encoded.len() {
            let mut mutated = encoded.clone();
            mutated[index] ^= 0x80;
            assert_ne!(decode_public_values(&mutated), Ok(values()));
        }

        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(
            decode_public_values(&trailing),
            Err(AuthorizationError::InvalidPublicValues)
        );
    }

    #[test]
    fn public_values_reject_each_malformed_protocol_field() {
        let encoded = encode_public_values(&values()).unwrap();
        let offsets = protocol_offsets();

        let mut wrong_domain = encoded.clone();
        wrong_domain[4] ^= 1;
        assert_invalid(&wrong_domain);

        let mut wrong_version = encoded.clone();
        wrong_version[offsets.version..offsets.version + 4].copy_from_slice(&2_u32.to_be_bytes());
        assert_invalid(&wrong_version);

        for digest_length in offsets.digest_lengths {
            let mut wrong_length = encoded.clone();
            wrong_length[digest_length..digest_length + 4].copy_from_slice(&31_u32.to_be_bytes());
            assert_invalid(&wrong_length);
        }

        for decision in [0, 2] {
            let mut wrong_decision = encoded.clone();
            wrong_decision[offsets.decision] = decision;
            assert_invalid(&wrong_decision);
        }

        let mut negative_time = encoded.clone();
        negative_time[offsets.issued_at..offsets.issued_at + 8]
            .copy_from_slice(&(-1_i64).to_be_bytes());
        assert_invalid(&negative_time);

        let mut reversed_time = encoded.clone();
        reversed_time[offsets.expires_at..offsets.expires_at + 8]
            .copy_from_slice(&10_i64.to_be_bytes());
        assert_invalid(&reversed_time);

        for length in 0..encoded.len() {
            assert_invalid(&encoded[..length]);
        }
    }

    #[test]
    fn public_value_encoding_rejects_invalid_version_and_time() {
        let mut value = values();
        value.format_version += 1;
        assert_eq!(
            encode_public_values(&value),
            Err(AuthorizationError::InvalidPublicValues)
        );

        let mut value = values();
        value.issued_at_unix_seconds = -1;
        assert_eq!(
            encode_public_values(&value),
            Err(AuthorizationError::InvalidPublicValues)
        );

        let mut value = values();
        value.expires_at_unix_seconds = value.issued_at_unix_seconds;
        assert_eq!(
            encode_public_values(&value),
            Err(AuthorizationError::InvalidPublicValues)
        );
    }

    fn assert_invalid(bytes: &[u8]) {
        assert_eq!(
            decode_public_values(bytes),
            Err(AuthorizationError::InvalidPublicValues)
        );
    }

    struct ProtocolOffsets {
        version: usize,
        digest_lengths: [usize; 5],
        decision: usize,
        issued_at: usize,
        expires_at: usize,
    }

    fn protocol_offsets() -> ProtocolOffsets {
        let mut offset = 4 + AUTHORIZATION_PUBLIC_VALUES_DOMAIN.len();
        let version = offset;
        offset += 4;

        let policy_hash_length = offset;
        offset += 4 + 32;
        let snapshot_root_length = offset;
        offset += 4 + 32;
        let request_commitment_length = offset;
        offset += 4 + 32;

        let decision = offset;
        offset += 1;

        let scope_hash_length = offset;
        offset += 4 + 32;
        let issued_at = offset;
        offset += 8;
        let expires_at = offset;
        offset += 8;
        let nullifier_length = offset;

        ProtocolOffsets {
            version,
            digest_lengths: [
                policy_hash_length,
                snapshot_root_length,
                request_commitment_length,
                scope_hash_length,
                nullifier_length,
            ],
            decision,
            issued_at,
            expires_at,
        }
    }
}
