use std::collections::HashSet;
use std::fmt::{self, Debug, Formatter};
use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use super::codec;
use super::{
    CleanupSummary, DeleteStatus, MAX_PRIVATE_ENVELOPE_BYTES, PrivateCapabilityEnvelope,
    PrivateEnvelopeError, PrivateEnvelopeRef,
};
use crate::admission::AdmittedCapabilityRequest;

const DIRECTORY_MODE: u32 = 0o700;
const ENVELOPE_FILE_MODE: u32 = 0o600;
const TEMPORARY_CREATE_ATTEMPTS: usize = 32;

static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Owner-only filesystem boundary for admitted private capability requests.
#[derive(Clone)]
pub struct PrivateEnvelopeStore {
    inner: Arc<StoreInner>,
}

struct StoreInner {
    root: PathBuf,
    directory: File,
    identity: FileIdentity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

struct CheckedFile {
    file: File,
    identity: FileIdentity,
    length: u64,
}

#[derive(Clone, Copy)]
enum CleanupKind {
    Envelope,
    Temporary,
}

struct CleanupCandidate {
    path: PathBuf,
    identity: FileIdentity,
    kind: CleanupKind,
}

impl PrivateEnvelopeStore {
    /// Opens or creates one dedicated `0700` store directory.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, PrivateEnvelopeError> {
        let requested_root = root.as_ref();
        if !requested_root.is_absolute() {
            return Err(PrivateEnvelopeError::StoreRootUnsafe);
        }

        let prepared_identity = prepare_root(requested_root)?;
        let root = std::fs::canonicalize(requested_root)
            .map_err(|_| PrivateEnvelopeError::StoreRootUnavailable)?;
        let path_metadata = std::fs::symlink_metadata(&root)
            .map_err(|_| PrivateEnvelopeError::StoreRootUnavailable)?;
        validate_directory_metadata(&path_metadata)?;
        if FileIdentity::from_metadata(&path_metadata) != prepared_identity {
            return Err(PrivateEnvelopeError::StoreRootUnsafe);
        }

        let directory =
            File::open(&root).map_err(|_| PrivateEnvelopeError::StoreRootUnavailable)?;
        let opened_metadata = directory
            .metadata()
            .map_err(|_| PrivateEnvelopeError::StoreRootUnavailable)?;
        validate_directory_metadata(&opened_metadata)?;
        let identity = FileIdentity::from_metadata(&path_metadata);
        if identity != FileIdentity::from_metadata(&opened_metadata) {
            return Err(PrivateEnvelopeError::StoreRootUnsafe);
        }

        let store = Self {
            inner: Arc::new(StoreInner {
                root,
                directory,
                identity,
            }),
        };
        store.verify_root()?;
        Ok(store)
    }

    /// Writes, syncs, and atomically publishes one admitted envelope without replacement.
    pub fn stage(
        &self,
        admitted: &AdmittedCapabilityRequest,
    ) -> Result<PrivateEnvelopeRef, PrivateEnvelopeError> {
        let (reference, bytes) = codec::encode(admitted)?;
        self.verify_root()?;

        let final_path = self.final_path(&reference);
        require_absent_final_path(&final_path)?;
        let (temporary_path, mut temporary_file) = self.create_temporary_file(&reference)?;
        let temporary_identity = FileIdentity::from_metadata(
            &temporary_file
                .metadata()
                .map_err(|_| PrivateEnvelopeError::StorageWrite)?,
        );
        let mut final_link_created = false;
        let mut temporary_name_present = true;

        let result = (|| {
            temporary_file
                .write_all(&bytes)
                .map_err(|_| PrivateEnvelopeError::StorageWrite)?;
            temporary_file
                .sync_all()
                .map_err(|_| PrivateEnvelopeError::StorageSync)?;
            validate_staged_file(&temporary_file, bytes.len() as u64, temporary_identity)?;

            match std::fs::hard_link(&temporary_path, &final_path) {
                Ok(()) => final_link_created = true,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    return Err(PrivateEnvelopeError::EnvelopeAlreadyExists);
                }
                Err(_) => return Err(PrivateEnvelopeError::StoragePublish),
            }
            require_path_identity(&final_path, temporary_identity, false)?;

            std::fs::remove_file(&temporary_path)
                .map_err(|_| PrivateEnvelopeError::CleanupFailed)?;
            temporary_name_present = false;
            self.sync_root()?;
            require_path_identity(&final_path, temporary_identity, true)?;
            self.verify_root()
        })();

        if let Err(error) = result {
            if final_link_created {
                rollback_file(&final_path, temporary_identity);
            }
            if temporary_name_present {
                rollback_file(&temporary_path, temporary_identity);
            }
            let _ = self.inner.directory.sync_all();
            return Err(error);
        }

        Ok(reference)
    }

    /// Reads one bounded envelope and repeats all proof-critical validation.
    pub fn read(
        &self,
        reference: &PrivateEnvelopeRef,
    ) -> Result<PrivateCapabilityEnvelope, PrivateEnvelopeError> {
        self.verify_root()?;
        let path = self.final_path(reference);
        let mut checked = self.open_checked_file(&path, true)?;
        let read_limit = u64::try_from(MAX_PRIVATE_ENVELOPE_BYTES)
            .map_err(|_| PrivateEnvelopeError::StorageRead)?
            .checked_add(1)
            .ok_or(PrivateEnvelopeError::StorageRead)?;
        let capacity =
            usize::try_from(checked.length).map_err(|_| PrivateEnvelopeError::EnvelopeTooLarge)?;
        let mut bytes = Vec::with_capacity(capacity);
        Read::by_ref(&mut checked.file)
            .take(read_limit)
            .read_to_end(&mut bytes)
            .map_err(|_| PrivateEnvelopeError::StorageRead)?;
        if bytes.len() > MAX_PRIVATE_ENVELOPE_BYTES {
            return Err(PrivateEnvelopeError::EnvelopeTooLarge);
        }
        if bytes.len() as u64 != checked.length {
            return Err(PrivateEnvelopeError::UnsafeEnvelopeFile);
        }
        require_path_identity(&path, checked.identity, true)?;
        self.verify_root()?;
        codec::decode(reference, &bytes)
    }

    /// Removes one final envelope. Missing files make repeated cancellation idempotent.
    pub fn delete(
        &self,
        reference: &PrivateEnvelopeRef,
    ) -> Result<DeleteStatus, PrivateEnvelopeError> {
        self.verify_root()?;
        let path = self.final_path(reference);
        match std::fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(DeleteStatus::NotFound);
            }
            Err(_) => return Err(PrivateEnvelopeError::StorageDelete),
            Ok(_) => {}
        }

        let checked = self.open_checked_file(&path, false)?;
        std::fs::remove_file(&path).map_err(|_| PrivateEnvelopeError::StorageDelete)?;
        if !matches!(
            std::fs::symlink_metadata(&path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        ) {
            return Err(PrivateEnvelopeError::UnsafeEnvelopeFile);
        }
        drop(checked.file);
        self.sync_root()?;
        self.verify_root()?;
        Ok(DeleteStatus::Deleted)
    }

    /// Deletes old files that are absent from a caller-supplied BullMQ live set.
    ///
    /// The grace cutoff protects envelopes staged just before queue publication.
    pub fn cleanup_orphans(
        &self,
        live_references: &HashSet<PrivateEnvelopeRef>,
        not_modified_after: SystemTime,
    ) -> Result<CleanupSummary, PrivateEnvelopeError> {
        self.verify_root()?;
        let mut candidates = Vec::new();
        let entries =
            std::fs::read_dir(&self.inner.root).map_err(|_| PrivateEnvelopeError::CleanupFailed)?;

        for entry in entries {
            let entry = entry.map_err(|_| PrivateEnvelopeError::CleanupFailed)?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| PrivateEnvelopeError::CleanupFailed)?;
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path)
                .map_err(|_| PrivateEnvelopeError::CleanupFailed)?;
            validate_cleanup_metadata(&metadata)?;
            let modified = metadata
                .modified()
                .map_err(|_| PrivateEnvelopeError::CleanupFailed)?;

            if let Some(reference) = reference_from_file_name(&name) {
                let reference = reference?;
                if live_references.contains(&reference) {
                    if metadata.nlink() != 1 {
                        return Err(PrivateEnvelopeError::UnsafeEnvelopeFile);
                    }
                    continue;
                }
                if modified <= not_modified_after {
                    candidates.push(CleanupCandidate {
                        path,
                        identity: FileIdentity::from_metadata(&metadata),
                        kind: CleanupKind::Envelope,
                    });
                }
                continue;
            }

            if is_temporary_file_name(&name) {
                if modified <= not_modified_after {
                    candidates.push(CleanupCandidate {
                        path,
                        identity: FileIdentity::from_metadata(&metadata),
                        kind: CleanupKind::Temporary,
                    });
                }
                continue;
            }

            return Err(PrivateEnvelopeError::CleanupFailed);
        }

        let mut summary = CleanupSummary::default();
        for candidate in candidates {
            require_cleanup_identity(&candidate.path, candidate.identity)?;
            std::fs::remove_file(&candidate.path)
                .map_err(|_| PrivateEnvelopeError::CleanupFailed)?;
            match candidate.kind {
                CleanupKind::Envelope => summary.envelopes_deleted += 1,
                CleanupKind::Temporary => summary.temporary_files_deleted += 1,
            }
        }
        if summary.envelopes_deleted > 0 || summary.temporary_files_deleted > 0 {
            self.sync_root()?;
        }
        self.verify_root()?;
        Ok(summary)
    }

    fn create_temporary_file(
        &self,
        reference: &PrivateEnvelopeRef,
    ) -> Result<(PathBuf, File), PrivateEnvelopeError> {
        for _ in 0..TEMPORARY_CREATE_ATTEMPTS {
            let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let name = format!(
                ".{}.tmp-{}-{sequence}",
                reference.file_name(),
                std::process::id()
            );
            let path = self.inner.root.join(name);
            let opened = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(ENVELOPE_FILE_MODE)
                .open(&path);
            match opened {
                Ok(file) => {
                    file.set_permissions(std::fs::Permissions::from_mode(ENVELOPE_FILE_MODE))
                        .map_err(|_| PrivateEnvelopeError::StorageWrite)?;
                    let metadata = file
                        .metadata()
                        .map_err(|_| PrivateEnvelopeError::StorageWrite)?;
                    validate_private_file_metadata(&metadata, false)?;
                    return Ok((path, file));
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err(PrivateEnvelopeError::StorageWrite),
            }
        }
        Err(PrivateEnvelopeError::TemporaryFileConflict)
    }

    fn open_checked_file(
        &self,
        path: &Path,
        apply_byte_ceiling: bool,
    ) -> Result<CheckedFile, PrivateEnvelopeError> {
        let path_metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(PrivateEnvelopeError::EnvelopeNotFound);
            }
            Err(_) => return Err(PrivateEnvelopeError::StorageRead),
        };
        validate_private_file_metadata(&path_metadata, true)?;
        if apply_byte_ceiling && path_metadata.len() > MAX_PRIVATE_ENVELOPE_BYTES as u64 {
            return Err(PrivateEnvelopeError::EnvelopeTooLarge);
        }

        let file = File::open(path).map_err(|_| PrivateEnvelopeError::StorageRead)?;
        let opened_metadata = file
            .metadata()
            .map_err(|_| PrivateEnvelopeError::StorageRead)?;
        validate_private_file_metadata(&opened_metadata, true)?;
        if apply_byte_ceiling && opened_metadata.len() > MAX_PRIVATE_ENVELOPE_BYTES as u64 {
            return Err(PrivateEnvelopeError::EnvelopeTooLarge);
        }
        let identity = FileIdentity::from_metadata(&path_metadata);
        if identity != FileIdentity::from_metadata(&opened_metadata)
            || path_metadata.len() != opened_metadata.len()
        {
            return Err(PrivateEnvelopeError::UnsafeEnvelopeFile);
        }
        require_path_identity(path, identity, true)?;

        Ok(CheckedFile {
            file,
            identity,
            length: opened_metadata.len(),
        })
    }

    fn final_path(&self, reference: &PrivateEnvelopeRef) -> PathBuf {
        self.inner.root.join(reference.file_name())
    }

    fn verify_root(&self) -> Result<(), PrivateEnvelopeError> {
        let path_metadata = std::fs::symlink_metadata(&self.inner.root)
            .map_err(|_| PrivateEnvelopeError::StoreRootUnavailable)?;
        validate_directory_metadata(&path_metadata)?;
        let opened_metadata = self
            .inner
            .directory
            .metadata()
            .map_err(|_| PrivateEnvelopeError::StoreRootUnavailable)?;
        validate_directory_metadata(&opened_metadata)?;
        if FileIdentity::from_metadata(&path_metadata) != self.inner.identity
            || FileIdentity::from_metadata(&opened_metadata) != self.inner.identity
        {
            return Err(PrivateEnvelopeError::StoreRootUnsafe);
        }
        Ok(())
    }

    fn sync_root(&self) -> Result<(), PrivateEnvelopeError> {
        self.inner
            .directory
            .sync_all()
            .map_err(|_| PrivateEnvelopeError::StorageSync)
    }

    #[cfg(test)]
    pub(super) fn final_path_for_test(&self, reference: &PrivateEnvelopeRef) -> PathBuf {
        self.final_path(reference)
    }
}

impl Debug for PrivateEnvelopeStore {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateEnvelopeStore")
            .field("root", &"<redacted>")
            .finish()
    }
}

impl FileIdentity {
    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
}

fn prepare_root(root: &Path) -> Result<FileIdentity, PrivateEnvelopeError> {
    match std::fs::symlink_metadata(root) {
        Ok(metadata) => {
            validate_directory_metadata(&metadata)?;
            Ok(FileIdentity::from_metadata(&metadata))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = DirBuilder::new();
            builder.mode(DIRECTORY_MODE);
            builder
                .create(root)
                .map_err(|_| PrivateEnvelopeError::StoreRootUnavailable)?;
            let parent = root
                .parent()
                .ok_or(PrivateEnvelopeError::StoreRootUnavailable)?;
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|_| PrivateEnvelopeError::StorageSync)?;
            let metadata = std::fs::symlink_metadata(root)
                .map_err(|_| PrivateEnvelopeError::StoreRootUnavailable)?;
            validate_directory_metadata(&metadata)?;
            Ok(FileIdentity::from_metadata(&metadata))
        }
        Err(_) => Err(PrivateEnvelopeError::StoreRootUnavailable),
    }
}

fn validate_directory_metadata(metadata: &std::fs::Metadata) -> Result<(), PrivateEnvelopeError> {
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o7777 != DIRECTORY_MODE
    {
        return Err(PrivateEnvelopeError::StoreRootUnsafe);
    }
    Ok(())
}

fn validate_private_file_metadata(
    metadata: &std::fs::Metadata,
    require_single_link: bool,
) -> Result<(), PrivateEnvelopeError> {
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o7777 != ENVELOPE_FILE_MODE
        || (require_single_link && metadata.nlink() != 1)
    {
        return Err(PrivateEnvelopeError::UnsafeEnvelopeFile);
    }
    Ok(())
}

fn validate_cleanup_metadata(metadata: &std::fs::Metadata) -> Result<(), PrivateEnvelopeError> {
    validate_private_file_metadata(metadata, false)?;
    if metadata.len() > MAX_PRIVATE_ENVELOPE_BYTES as u64 || metadata.nlink() > 2 {
        return Err(PrivateEnvelopeError::UnsafeEnvelopeFile);
    }
    Ok(())
}

fn validate_staged_file(
    file: &File,
    expected_length: u64,
    expected_identity: FileIdentity,
) -> Result<(), PrivateEnvelopeError> {
    let metadata = file
        .metadata()
        .map_err(|_| PrivateEnvelopeError::StorageWrite)?;
    validate_private_file_metadata(&metadata, true)?;
    if metadata.len() != expected_length
        || FileIdentity::from_metadata(&metadata) != expected_identity
    {
        return Err(PrivateEnvelopeError::UnsafeEnvelopeFile);
    }
    Ok(())
}

fn require_absent_final_path(path: &Path) -> Result<(), PrivateEnvelopeError> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(metadata)
            if metadata.file_type().is_file()
                && !metadata.file_type().is_symlink()
                && metadata.permissions().mode() & 0o7777 == ENVELOPE_FILE_MODE =>
        {
            Err(PrivateEnvelopeError::EnvelopeAlreadyExists)
        }
        Ok(_) => Err(PrivateEnvelopeError::UnsafeEnvelopeFile),
        Err(_) => Err(PrivateEnvelopeError::StoragePublish),
    }
}

fn require_path_identity(
    path: &Path,
    expected_identity: FileIdentity,
    require_single_link: bool,
) -> Result<(), PrivateEnvelopeError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| PrivateEnvelopeError::UnsafeEnvelopeFile)?;
    validate_private_file_metadata(&metadata, require_single_link)?;
    if FileIdentity::from_metadata(&metadata) != expected_identity {
        return Err(PrivateEnvelopeError::UnsafeEnvelopeFile);
    }
    Ok(())
}

fn require_cleanup_identity(
    path: &Path,
    expected_identity: FileIdentity,
) -> Result<(), PrivateEnvelopeError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| PrivateEnvelopeError::CleanupFailed)?;
    validate_cleanup_metadata(&metadata)?;
    if FileIdentity::from_metadata(&metadata) != expected_identity {
        return Err(PrivateEnvelopeError::CleanupFailed);
    }
    Ok(())
}

fn rollback_file(path: &Path, expected_identity: FileIdentity) {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_file()
        && !metadata.file_type().is_symlink()
        && FileIdentity::from_metadata(&metadata) == expected_identity
    {
        let _ = std::fs::remove_file(path);
    }
}

fn reference_from_file_name(
    name: &str,
) -> Option<Result<PrivateEnvelopeRef, PrivateEnvelopeError>> {
    name.strip_suffix(".pb")
        .map(PrivateEnvelopeRef::from_queue_token)
}

fn is_temporary_file_name(name: &str) -> bool {
    let Some(without_dot) = name.strip_prefix('.') else {
        return false;
    };
    let Some((final_name, suffix)) = without_dot.split_once(".tmp-") else {
        return false;
    };
    let mut parts = suffix.split('-');
    let process_id = parts.next().unwrap_or_default();
    let sequence = parts.next().unwrap_or_default();
    if process_id.is_empty()
        || sequence.is_empty()
        || parts.next().is_some()
        || !process_id.bytes().all(|byte| byte.is_ascii_digit())
        || !sequence.bytes().all(|byte| byte.is_ascii_digit())
    {
        return false;
    }
    matches!(reference_from_file_name(final_name), Some(Ok(_)))
}
