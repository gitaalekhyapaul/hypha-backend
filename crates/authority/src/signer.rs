use std::{
    fmt::{self, Debug, Formatter},
    fs::{File, OpenOptions},
    io::Read,
    marker::PhantomData,
    path::Path,
};

use ed25519_dalek::{
    Signer, SigningKey,
    pkcs8::{DecodePrivateKey, SecretDocument},
};
use raziel_canonical::{ValidatedPolicyApprovalClaimsV1, policy_approval_signature_message};
use raziel_snapshot::{ValidatedSnapshotDescriptorClaimsV1, snapshot_descriptor_signature_message};

use crate::{AuthorityError, AuthorityRole, derive_key_id};

const MAX_PKCS8_BYTES: u64 = 4_096;

mod sealed {
    pub trait Sealed {}
}

pub trait RoleMarker: sealed::Sealed {
    const ROLE: AuthorityRole;
}

pub enum PolicyApprovalRole {}
pub enum SnapshotRole {}

impl sealed::Sealed for PolicyApprovalRole {}
impl sealed::Sealed for SnapshotRole {}

impl RoleMarker for PolicyApprovalRole {
    const ROLE: AuthorityRole = AuthorityRole::PolicyApproval;
}

impl RoleMarker for SnapshotRole {
    const ROLE: AuthorityRole = AuthorityRole::Snapshot;
}

pub struct AuthoritySigner<R: RoleMarker> {
    signing_key: SigningKey,
    key_id: String,
    role: PhantomData<R>,
}

impl<R: RoleMarker> AuthoritySigner<R> {
    pub fn from_pkcs8_der(der: &[u8]) -> Result<Self, AuthorityError> {
        let signing_key =
            SigningKey::from_pkcs8_der(der).map_err(|_| AuthorityError::InvalidPkcs8)?;
        let public_key = signing_key.verifying_key().to_bytes();
        if signing_key.verifying_key().is_weak() {
            return Err(AuthorityError::WeakPublicKey);
        }
        Ok(Self {
            key_id: derive_key_id(&public_key),
            signing_key,
            role: PhantomData,
        })
    }

    pub fn from_read_only_pkcs8_file(path: &Path) -> Result<Self, AuthorityError> {
        let file = File::open(path).map_err(|_| AuthorityError::KeyFileRead)?;
        let metadata = file.metadata().map_err(|_| AuthorityError::KeyFileRead)?;
        let path_metadata =
            std::fs::symlink_metadata(path).map_err(|_| AuthorityError::KeyFileRead)?;
        if path_metadata.file_type().is_symlink() || !metadata.is_file() || !path_metadata.is_file()
        {
            return Err(AuthorityError::KeyFileNotRegular);
        }
        require_same_file(&metadata, &path_metadata)?;
        reject_insecure_permissions(&metadata)?;
        reject_effectively_writable(path, &metadata)?;

        let mut bytes = Vec::new();
        file.take(MAX_PKCS8_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| AuthorityError::KeyFileRead)?;
        if bytes.len() as u64 > MAX_PKCS8_BYTES {
            bytes.fill(0);
            return Err(AuthorityError::InvalidPkcs8);
        }
        let document = SecretDocument::try_from(bytes.as_slice());
        bytes.fill(0);
        let document = document.map_err(|_| AuthorityError::InvalidPkcs8)?;
        Self::from_pkcs8_der(document.as_bytes())
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    pub const fn role(&self) -> AuthorityRole {
        R::ROLE
    }

    fn sign_message(&self, message: &[u8]) -> [u8; 64] {
        self.signing_key.sign(message).to_bytes()
    }
}

impl AuthoritySigner<PolicyApprovalRole> {
    pub fn sign_policy_approval(
        &self,
        claims: &ValidatedPolicyApprovalClaimsV1,
    ) -> Result<[u8; 64], AuthorityError> {
        let claims = claims.claims();
        if claims.signing_key_id != self.key_id {
            return Err(AuthorityError::InvalidKeyId);
        }
        let message = policy_approval_signature_message(claims)?;
        Ok(self.sign_message(&message))
    }
}

impl AuthoritySigner<SnapshotRole> {
    /// Signs claims after a trusted caller has allocated their epoch.
    ///
    /// Runtime snapshot publication should use `SnapshotAuthorityPublisher`.
    pub fn sign_snapshot_descriptor(
        &self,
        claims: &ValidatedSnapshotDescriptorClaimsV1,
    ) -> Result<[u8; 64], AuthorityError> {
        let claims = claims.claims();
        if claims.signing_key_id != self.key_id {
            return Err(AuthorityError::InvalidKeyId);
        }
        let message = snapshot_descriptor_signature_message(claims)?;
        Ok(self.sign_message(&message))
    }
}

impl<R: RoleMarker> Debug for AuthoritySigner<R> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthoritySigner")
            .field("role", &R::ROLE)
            .field("key_id", &self.key_id)
            .field("private_key", &"[REDACTED]")
            .finish()
    }
}

#[cfg(unix)]
fn reject_insecure_permissions(metadata: &std::fs::Metadata) -> Result<(), AuthorityError> {
    use std::os::unix::fs::PermissionsExt;

    if metadata.permissions().mode() & 0o7777 != 0o400 {
        return Err(AuthorityError::KeyFileWritable);
    }
    Ok(())
}

#[cfg(not(unix))]
fn reject_insecure_permissions(_metadata: &std::fs::Metadata) -> Result<(), AuthorityError> {
    Err(AuthorityError::KeyFileWritable)
}

#[cfg(unix)]
fn require_same_file(
    left: &std::fs::Metadata,
    right: &std::fs::Metadata,
) -> Result<(), AuthorityError> {
    use std::os::unix::fs::MetadataExt;

    if left.dev() != right.dev() || left.ino() != right.ino() {
        return Err(AuthorityError::KeyFileRead);
    }
    Ok(())
}

#[cfg(not(unix))]
fn require_same_file(
    _left: &std::fs::Metadata,
    _right: &std::fs::Metadata,
) -> Result<(), AuthorityError> {
    Ok(())
}

fn reject_effectively_writable(
    path: &Path,
    expected_metadata: &std::fs::Metadata,
) -> Result<(), AuthorityError> {
    let Ok(writable) = OpenOptions::new().write(true).open(path) else {
        return Ok(());
    };
    let writable_metadata = writable
        .metadata()
        .map_err(|_| AuthorityError::KeyFileRead)?;
    require_same_file(expected_metadata, &writable_metadata)?;
    Err(AuthorityError::KeyFileWritable)
}
