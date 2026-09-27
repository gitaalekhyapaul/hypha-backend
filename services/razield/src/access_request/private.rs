use std::fs::{DirBuilder, File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use prost::Message;
use raziel_proto::raziel::v1::CreateAccessRequestRequest;

use super::approval::{MAX_APPROVAL_BYTES, StoredApproval};
use super::model::{AccessError, MAX_INTENT_BYTES, NormalizedIntent, valid_hex, valid_identity};

#[cfg(target_os = "linux")]
const NOFOLLOW: i32 = 0x20000;
#[cfg(target_os = "macos")]
const NOFOLLOW: i32 = 0x100;

#[derive(Clone, PartialEq, Message)]
struct PrivateIntent {
    #[prost(uint32, tag = "1")]
    version: u32,
    #[prost(message, optional, tag = "2")]
    input: Option<CreateAccessRequestRequest>,
}

pub(super) struct PrivateStore {
    root: PathBuf,
    directory: File,
    device: u64,
    inode: u64,
    uid: u32,
}

pub(super) struct StoreLock(File);
impl Drop for StoreLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

impl PrivateStore {
    pub fn open(root: &Path) -> Result<Self, AccessError> {
        check_ancestors(root.parent().ok_or(AccessError::Configuration)?)?;
        match std::fs::symlink_metadata(root) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                match DirBuilder::new().mode(0o700).create(root) {
                    Ok(()) => File::open(root.parent().unwrap())
                        .and_then(|f| f.sync_all())
                        .map_err(storage)?,
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(e) => return Err(storage(e)),
                }
            }
            Err(e) => return Err(storage(e)),
        }
        check_ancestors(root)?;
        let metadata = std::fs::symlink_metadata(root).map_err(storage)?;
        directory_metadata(&metadata)?;
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(NOFOLLOW)
            .open(root)
            .map_err(storage)?;
        let store = Self {
            root: root.to_owned(),
            directory,
            device: metadata.dev(),
            inode: metadata.ino(),
            uid: metadata.uid(),
        };
        store.verify()?;
        let path = store.root.join(".lock");
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(NOFOLLOW)
            .open(&path)
        {
            Ok(file) => {
                file.sync_all().map_err(storage)?;
                store.directory.sync_all().map_err(storage)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(storage(e)),
        }
        store.open_checked(&path, 0)?;
        Ok(store)
    }

    /// All processes sharing this Redis namespace must share this owner root.
    pub fn lock(&self) -> Result<StoreLock, AccessError> {
        self.verify()?;
        let file = self.open_checked(&self.root.join(".lock"), 0)?;
        file.try_lock()
            .map_err(|_| AccessError::ServiceUnavailable)?;
        self.verify()?;
        Ok(StoreLock(file))
    }

    pub fn read(&self, reference: &str) -> Result<NormalizedIntent, AccessError> {
        self.verify()?;
        let path = self.path(reference, "intent")?;
        let mut file = self.open_checked(&path, MAX_INTENT_BYTES + 64)?;
        let expected = file.metadata().map_err(storage)?;
        let mut bytes = Vec::with_capacity(expected.len() as usize);
        Read::by_ref(&mut file)
            .take((MAX_INTENT_BYTES + 65) as u64)
            .read_to_end(&mut bytes)
            .map_err(storage)?;
        if bytes.len() as u64 != expected.len() {
            return Err(AccessError::ServiceUnavailable);
        }
        self.same_file(&path, &expected)?;
        let decoded =
            PrivateIntent::decode(bytes.as_slice()).map_err(|_| AccessError::ServiceUnavailable)?;
        if decoded.version != 1 || decoded.encode_to_vec() != bytes {
            return Err(AccessError::ServiceUnavailable);
        }
        let input = decoded.input.ok_or(AccessError::ServiceUnavailable)?;
        let normalized =
            NormalizedIntent::parse(input.clone()).map_err(|_| AccessError::ServiceUnavailable)?;
        if normalized.0 != input
            || input.dataset.is_none()
            || !reference.starts_with(&format!("{}.", normalized.identity()))
        {
            return Err(AccessError::ServiceUnavailable);
        }
        self.verify()?;
        Ok(normalized)
    }

    pub fn read_approval(
        &self,
        reference: &str,
        original: &NormalizedIntent,
    ) -> Result<Option<StoredApproval>, AccessError> {
        self.verify()?;
        let path = self.path(reference, "approval")?;
        match std::fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(storage(error)),
            Ok(_) => {}
        }
        let mut file = self.open_checked(&path, MAX_APPROVAL_BYTES)?;
        let expected = file.metadata().map_err(storage)?;
        let mut bytes = Vec::with_capacity(expected.len() as usize);
        Read::by_ref(&mut file)
            .take((MAX_APPROVAL_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(storage)?;
        if bytes.len() as u64 != expected.len() {
            return Err(AccessError::ServiceUnavailable);
        }
        self.same_file(&path, &expected)?;
        let value = StoredApproval::decode(&bytes, original)?;
        if !reference.starts_with(&format!("{}.", original.identity())) {
            return Err(AccessError::ServiceUnavailable);
        }
        self.verify()?;
        Ok(Some(value))
    }

    pub fn persist_approval(
        &self,
        reference: &str,
        original: &NormalizedIntent,
        approval: &StoredApproval,
    ) -> Result<(), AccessError> {
        approval.validate(original)?;
        if let Some(existing) = self.read_approval(reference, original)? {
            return if existing == *approval {
                Ok(())
            } else {
                Err(AccessError::Conflict)
            };
        }
        let bytes = approval.encode()?;
        self.persist_bytes(reference, "approval", "approval-stage", &bytes)?;
        if self.read_approval(reference, original)?.as_ref() != Some(approval) {
            return Err(AccessError::ServiceUnavailable);
        }
        Ok(())
    }

    pub fn persist(&self, reference: &str, input: &NormalizedIntent) -> Result<(), AccessError> {
        if input.0.dataset.is_none() {
            return Err(AccessError::InvalidRequest);
        }
        self.verify()?;
        let destination = self.path(reference, "intent")?;
        match std::fs::symlink_metadata(&destination) {
            Ok(_) => {
                return if self.read(reference)? == *input {
                    Ok(())
                } else {
                    Err(AccessError::Conflict)
                };
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(storage(e)),
        }
        let bytes = PrivateIntent {
            version: 1,
            input: Some(input.0.clone()),
        }
        .encode_to_vec();
        if bytes.len() > MAX_INTENT_BYTES + 64 {
            return Err(AccessError::InvalidRequest);
        }
        self.persist_bytes(reference, "intent", "stage", &bytes)?;
        if self.read(reference)? != *input {
            return Err(AccessError::ServiceUnavailable);
        }
        Ok(())
    }

    fn persist_bytes(
        &self,
        reference: &str,
        extension: &str,
        staging_extension: &str,
        bytes: &[u8],
    ) -> Result<(), AccessError> {
        self.verify()?;
        let destination = self.path(reference, extension)?;
        let staged = self.path(reference, staging_extension)?;
        self.remove_checked(&staged)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(NOFOLLOW)
            .open(&staged)
            .map_err(storage)?;
        self.file_metadata(&file.metadata().map_err(storage)?, MAX_APPROVAL_BYTES)?;
        file.write_all(bytes).map_err(storage)?;
        file.sync_all().map_err(storage)?;
        self.same_file(&staged, &file.metadata().map_err(storage)?)?;
        // The shared-root lock covers this absence check and atomic rename.
        match std::fs::symlink_metadata(&destination) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(AccessError::ServiceUnavailable),
        }
        std::fs::rename(&staged, &destination).map_err(storage)?;
        self.directory.sync_all().map_err(storage)?;
        self.verify()
    }

    pub fn delete(&self, reference: &str) -> Result<(), AccessError> {
        self.verify()?;
        self.remove_checked(&self.path(reference, "stage")?)?;
        self.remove_checked(&self.path(reference, "approval-stage")?)?;
        self.remove_checked(&self.path(reference, "approval")?)?;
        self.remove_checked(&self.path(reference, "intent")?)?;
        self.directory.sync_all().map_err(storage)?;
        self.verify()
    }

    fn path(&self, reference: &str, extension: &str) -> Result<PathBuf, AccessError> {
        if !reference
            .rsplit_once('.')
            .is_some_and(|(id, suffix)| valid_identity(id) && valid_hex(suffix, 32))
        {
            return Err(AccessError::ServiceUnavailable);
        }
        Ok(self.root.join(format!("{reference}.{extension}")))
    }

    fn remove_checked(&self, path: &Path) -> Result<(), AccessError> {
        match std::fs::symlink_metadata(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(storage(e)),
            Ok(_) => {}
        }
        let file = self.open_checked(path, MAX_APPROVAL_BYTES)?;
        self.same_file(path, &file.metadata().map_err(storage)?)?;
        std::fs::remove_file(path).map_err(storage)
    }

    fn open_checked(&self, path: &Path, max: usize) -> Result<File, AccessError> {
        let before = std::fs::symlink_metadata(path).map_err(storage)?;
        self.file_metadata(&before, max)?;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(NOFOLLOW)
            .open(path)
            .map_err(storage)?;
        let after = file.metadata().map_err(storage)?;
        self.file_metadata(&after, max)?;
        if before.dev() != after.dev() || before.ino() != after.ino() || before.len() != after.len()
        {
            return Err(AccessError::ServiceUnavailable);
        }
        Ok(file)
    }

    fn same_file(&self, path: &Path, expected: &Metadata) -> Result<(), AccessError> {
        let actual = std::fs::symlink_metadata(path).map_err(storage)?;
        self.file_metadata(&actual, MAX_APPROVAL_BYTES)?;
        if actual.dev() != expected.dev()
            || actual.ino() != expected.ino()
            || actual.len() != expected.len()
        {
            return Err(AccessError::ServiceUnavailable);
        }
        Ok(())
    }

    fn file_metadata(&self, metadata: &Metadata, max: usize) -> Result<(), AccessError> {
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.nlink() != 1
            || metadata.uid() != self.uid
            || metadata.permissions().mode() & 0o7777 != 0o600
            || metadata.len() > max as u64
        {
            return Err(AccessError::ServiceUnavailable);
        }
        Ok(())
    }

    fn verify(&self) -> Result<(), AccessError> {
        check_ancestors(&self.root)?;
        for metadata in [
            std::fs::symlink_metadata(&self.root).map_err(storage)?,
            self.directory.metadata().map_err(storage)?,
        ] {
            directory_metadata(&metadata)?;
            if metadata.dev() != self.device
                || metadata.ino() != self.inode
                || metadata.uid() != self.uid
            {
                return Err(AccessError::ServiceUnavailable);
            }
        }
        Ok(())
    }
}

fn check_ancestors(root: &Path) -> Result<(), AccessError> {
    if !root.is_absolute()
        || root
            .components()
            .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
    {
        return Err(AccessError::Configuration);
    }
    for ancestor in root.ancestors() {
        let metadata = std::fs::symlink_metadata(ancestor).map_err(storage)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(AccessError::ServiceUnavailable);
        }
    }
    Ok(())
}

fn directory_metadata(metadata: &Metadata) -> Result<(), AccessError> {
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o7777 != 0o700
    {
        return Err(AccessError::ServiceUnavailable);
    }
    Ok(())
}

fn storage(_: std::io::Error) -> AccessError {
    AccessError::ServiceUnavailable
}
