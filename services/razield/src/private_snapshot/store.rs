use std::fmt::{self, Debug, Formatter};
use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use raziel_policy_ir::ValidatedPolicyProgram;
use raziel_snapshot::Digest;

use super::codec;
use super::{
    MAX_PRIVATE_SNAPSHOT_BYTES, PrivateRelationshipSnapshot, PrivateSnapshotError,
    snapshot_file_name,
};

const DIRECTORY_MODE: u32 = 0o700;
const SNAPSHOT_FILE_MODE: u32 = 0o600;
const TEMPORARY_CREATE_ATTEMPTS: usize = 32;

static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Owner-only immutable store for private relationship snapshots.
#[derive(Clone)]
pub struct PrivateSnapshotStore {
    inner: Arc<StoreInner>,
}

struct StoreInner {
    root: PathBuf,
    directory: File,
    identity: FileIdentity,
    owner_uid: u32,
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

impl PrivateSnapshotStore {
    /// Opens or creates one dedicated `0700` snapshot directory.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, PrivateSnapshotError> {
        let requested_root = root.as_ref();
        if !requested_root.is_absolute() {
            return Err(PrivateSnapshotError::StoreRootUnsafe);
        }

        let prepared = prepare_root(requested_root)?;
        let root = std::fs::canonicalize(requested_root)
            .map_err(|_| PrivateSnapshotError::StoreRootUnavailable)?;
        let path_metadata = std::fs::symlink_metadata(&root)
            .map_err(|_| PrivateSnapshotError::StoreRootUnavailable)?;
        validate_directory_metadata(&path_metadata)?;
        if FileIdentity::from_metadata(&path_metadata) != prepared.identity
            || path_metadata.uid() != prepared.owner_uid
        {
            return Err(PrivateSnapshotError::StoreRootUnsafe);
        }

        let directory =
            File::open(&root).map_err(|_| PrivateSnapshotError::StoreRootUnavailable)?;
        let opened_metadata = directory
            .metadata()
            .map_err(|_| PrivateSnapshotError::StoreRootUnavailable)?;
        validate_directory_metadata(&opened_metadata)?;
        if FileIdentity::from_metadata(&opened_metadata) != prepared.identity
            || opened_metadata.uid() != prepared.owner_uid
        {
            return Err(PrivateSnapshotError::StoreRootUnsafe);
        }

        let store = Self {
            inner: Arc::new(StoreInner {
                root,
                directory,
                identity: prepared.identity,
                owner_uid: prepared.owner_uid,
            }),
        };
        store.verify_root()?;
        Ok(store)
    }

    /// Writes, syncs, and publishes one new immutable root-keyed snapshot.
    pub fn publish(
        &self,
        snapshot: &PrivateRelationshipSnapshot,
    ) -> Result<(), PrivateSnapshotError> {
        let bytes = codec::encode(snapshot)?;
        self.publish_encoded(snapshot.relationship_root(), &bytes)
    }

    /// Loads and fully revalidates the exact private data for a trusted registry record.
    pub fn load(
        &self,
        policy: &ValidatedPolicyProgram,
        expected_snapshot_id: &str,
        expected_root: &Digest,
    ) -> Result<PrivateRelationshipSnapshot, PrivateSnapshotError> {
        self.verify_root()?;
        let path = self.final_path(expected_root);
        let mut checked = self.open_checked_file(&path)?;
        let read_limit = u64::try_from(MAX_PRIVATE_SNAPSHOT_BYTES)
            .map_err(|_| PrivateSnapshotError::StorageRead)?
            .checked_add(1)
            .ok_or(PrivateSnapshotError::StorageRead)?;
        let capacity =
            usize::try_from(checked.length).map_err(|_| PrivateSnapshotError::SnapshotTooLarge)?;
        let mut bytes = Vec::with_capacity(capacity);
        Read::by_ref(&mut checked.file)
            .take(read_limit)
            .read_to_end(&mut bytes)
            .map_err(|_| PrivateSnapshotError::StorageRead)?;
        if bytes.len() > MAX_PRIVATE_SNAPSHOT_BYTES {
            return Err(PrivateSnapshotError::SnapshotTooLarge);
        }
        if bytes.len() as u64 != checked.length {
            return Err(PrivateSnapshotError::UnsafeSnapshotFile);
        }
        require_path_identity(&path, checked.identity, self.inner.owner_uid, true)?;
        self.verify_root()?;
        codec::decode(policy, expected_snapshot_id, expected_root, &bytes)
    }

    fn publish_encoded(
        &self,
        relationship_root: &Digest,
        bytes: &[u8],
    ) -> Result<(), PrivateSnapshotError> {
        self.verify_root()?;
        let final_path = self.final_path(relationship_root);
        require_absent_final_path(&final_path, self.inner.owner_uid)?;
        let (temporary_path, mut temporary_file) = self.create_temporary_file(relationship_root)?;
        let temporary_identity = FileIdentity::from_metadata(
            &temporary_file
                .metadata()
                .map_err(|_| PrivateSnapshotError::StorageWrite)?,
        );
        let mut final_link_created = false;
        let mut temporary_name_present = true;

        let result = (|| {
            temporary_file
                .write_all(bytes)
                .map_err(|_| PrivateSnapshotError::StorageWrite)?;
            temporary_file
                .sync_all()
                .map_err(|_| PrivateSnapshotError::StorageSync)?;
            validate_staged_file(
                &temporary_file,
                bytes.len() as u64,
                temporary_identity,
                self.inner.owner_uid,
            )?;

            match std::fs::hard_link(&temporary_path, &final_path) {
                Ok(()) => final_link_created = true,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    return Err(PrivateSnapshotError::SnapshotAlreadyExists);
                }
                Err(_) => return Err(PrivateSnapshotError::StoragePublish),
            }
            require_path_identity(&final_path, temporary_identity, self.inner.owner_uid, false)?;

            std::fs::remove_file(&temporary_path)
                .map_err(|_| PrivateSnapshotError::CleanupFailed)?;
            temporary_name_present = false;
            self.sync_root()?;
            require_path_identity(&final_path, temporary_identity, self.inner.owner_uid, true)?;
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
        Ok(())
    }

    fn create_temporary_file(
        &self,
        relationship_root: &Digest,
    ) -> Result<(PathBuf, File), PrivateSnapshotError> {
        let final_name = snapshot_file_name(relationship_root);
        for _ in 0..TEMPORARY_CREATE_ATTEMPTS {
            let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = self.inner.root.join(format!(
                ".{final_name}.tmp-{}-{sequence}",
                std::process::id()
            ));
            match OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(SNAPSHOT_FILE_MODE)
                .open(&path)
            {
                Ok(file) => {
                    file.set_permissions(std::fs::Permissions::from_mode(SNAPSHOT_FILE_MODE))
                        .map_err(|_| PrivateSnapshotError::StorageWrite)?;
                    validate_private_file_metadata(
                        &file
                            .metadata()
                            .map_err(|_| PrivateSnapshotError::StorageWrite)?,
                        self.inner.owner_uid,
                        true,
                    )?;
                    return Ok((path, file));
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err(PrivateSnapshotError::StorageWrite),
            }
        }
        Err(PrivateSnapshotError::TemporaryFileConflict)
    }

    fn open_checked_file(&self, path: &Path) -> Result<CheckedFile, PrivateSnapshotError> {
        let path_metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(PrivateSnapshotError::SnapshotNotFound);
            }
            Err(_) => return Err(PrivateSnapshotError::StorageRead),
        };
        validate_private_file_metadata(&path_metadata, self.inner.owner_uid, true)?;
        if path_metadata.len() > MAX_PRIVATE_SNAPSHOT_BYTES as u64 {
            return Err(PrivateSnapshotError::SnapshotTooLarge);
        }

        let file = File::open(path).map_err(|_| PrivateSnapshotError::StorageRead)?;
        let opened_metadata = file
            .metadata()
            .map_err(|_| PrivateSnapshotError::StorageRead)?;
        validate_private_file_metadata(&opened_metadata, self.inner.owner_uid, true)?;
        if opened_metadata.len() > MAX_PRIVATE_SNAPSHOT_BYTES as u64 {
            return Err(PrivateSnapshotError::SnapshotTooLarge);
        }
        let identity = FileIdentity::from_metadata(&path_metadata);
        if identity != FileIdentity::from_metadata(&opened_metadata)
            || path_metadata.len() != opened_metadata.len()
        {
            return Err(PrivateSnapshotError::UnsafeSnapshotFile);
        }
        require_path_identity(path, identity, self.inner.owner_uid, true)?;
        Ok(CheckedFile {
            file,
            identity,
            length: opened_metadata.len(),
        })
    }

    fn final_path(&self, relationship_root: &Digest) -> PathBuf {
        self.inner.root.join(snapshot_file_name(relationship_root))
    }

    fn verify_root(&self) -> Result<(), PrivateSnapshotError> {
        let path_metadata = std::fs::symlink_metadata(&self.inner.root)
            .map_err(|_| PrivateSnapshotError::StoreRootUnavailable)?;
        validate_directory_metadata(&path_metadata)?;
        let opened_metadata = self
            .inner
            .directory
            .metadata()
            .map_err(|_| PrivateSnapshotError::StoreRootUnavailable)?;
        validate_directory_metadata(&opened_metadata)?;
        if FileIdentity::from_metadata(&path_metadata) != self.inner.identity
            || FileIdentity::from_metadata(&opened_metadata) != self.inner.identity
            || path_metadata.uid() != self.inner.owner_uid
            || opened_metadata.uid() != self.inner.owner_uid
        {
            return Err(PrivateSnapshotError::StoreRootUnsafe);
        }
        Ok(())
    }

    fn sync_root(&self) -> Result<(), PrivateSnapshotError> {
        self.inner
            .directory
            .sync_all()
            .map_err(|_| PrivateSnapshotError::StorageSync)
    }

    #[cfg(test)]
    pub(super) fn final_path_for_test(&self, relationship_root: &Digest) -> PathBuf {
        self.final_path(relationship_root)
    }
}

impl Debug for PrivateSnapshotStore {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateSnapshotStore")
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

struct PreparedRoot {
    identity: FileIdentity,
    owner_uid: u32,
}

fn prepare_root(root: &Path) -> Result<PreparedRoot, PrivateSnapshotError> {
    match std::fs::symlink_metadata(root) {
        Ok(metadata) => {
            validate_directory_metadata(&metadata)?;
            Ok(PreparedRoot {
                identity: FileIdentity::from_metadata(&metadata),
                owner_uid: metadata.uid(),
            })
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = DirBuilder::new();
            builder.mode(DIRECTORY_MODE);
            builder
                .create(root)
                .map_err(|_| PrivateSnapshotError::StoreRootUnavailable)?;
            let parent = root
                .parent()
                .ok_or(PrivateSnapshotError::StoreRootUnavailable)?;
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|_| PrivateSnapshotError::StorageSync)?;
            let metadata = std::fs::symlink_metadata(root)
                .map_err(|_| PrivateSnapshotError::StoreRootUnavailable)?;
            validate_directory_metadata(&metadata)?;
            Ok(PreparedRoot {
                identity: FileIdentity::from_metadata(&metadata),
                owner_uid: metadata.uid(),
            })
        }
        Err(_) => Err(PrivateSnapshotError::StoreRootUnavailable),
    }
}

fn validate_directory_metadata(metadata: &std::fs::Metadata) -> Result<(), PrivateSnapshotError> {
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o7777 != DIRECTORY_MODE
    {
        return Err(PrivateSnapshotError::StoreRootUnsafe);
    }
    Ok(())
}

fn validate_private_file_metadata(
    metadata: &std::fs::Metadata,
    expected_owner_uid: u32,
    require_single_link: bool,
) -> Result<(), PrivateSnapshotError> {
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o7777 != SNAPSHOT_FILE_MODE
        || metadata.uid() != expected_owner_uid
        || (require_single_link && metadata.nlink() != 1)
    {
        return Err(PrivateSnapshotError::UnsafeSnapshotFile);
    }
    Ok(())
}

fn validate_staged_file(
    file: &File,
    expected_length: u64,
    expected_identity: FileIdentity,
    expected_owner_uid: u32,
) -> Result<(), PrivateSnapshotError> {
    let metadata = file
        .metadata()
        .map_err(|_| PrivateSnapshotError::StorageWrite)?;
    validate_private_file_metadata(&metadata, expected_owner_uid, true)?;
    if metadata.len() != expected_length
        || FileIdentity::from_metadata(&metadata) != expected_identity
    {
        return Err(PrivateSnapshotError::UnsafeSnapshotFile);
    }
    Ok(())
}

fn require_absent_final_path(
    path: &Path,
    expected_owner_uid: u32,
) -> Result<(), PrivateSnapshotError> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(metadata)
            if validate_private_file_metadata(&metadata, expected_owner_uid, true).is_ok() =>
        {
            Err(PrivateSnapshotError::SnapshotAlreadyExists)
        }
        Ok(_) => Err(PrivateSnapshotError::UnsafeSnapshotFile),
        Err(_) => Err(PrivateSnapshotError::StoragePublish),
    }
}

fn require_path_identity(
    path: &Path,
    expected_identity: FileIdentity,
    expected_owner_uid: u32,
    require_single_link: bool,
) -> Result<(), PrivateSnapshotError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| PrivateSnapshotError::UnsafeSnapshotFile)?;
    validate_private_file_metadata(&metadata, expected_owner_uid, require_single_link)?;
    if FileIdentity::from_metadata(&metadata) != expected_identity {
        return Err(PrivateSnapshotError::UnsafeSnapshotFile);
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
