use std::fs::{File, Metadata, OpenOptions};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use crate::AdminError;

use super::{MAX_ALLOCATION_BYTES, random_id};

#[derive(Clone, Copy)]
pub(super) struct DirectoryIdentity {
    owner: u32,
    device: u64,
    inode: u64,
}

pub(super) fn open_existing(root: &Path) -> Result<DirectoryIdentity, AdminError> {
    validate_components(root)?;
    validate_ancestors(root)?;
    directory_identity(root)
}

pub(super) fn initialize(root: &Path) -> Result<DirectoryIdentity, AdminError> {
    validate_components(root)?;
    let parent = root.parent().ok_or(AdminError::InvalidCandidate)?;
    validate_ancestors(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
        match std::fs::DirBuilder::new().mode(0o700).create(root) {
            Ok(()) => crate::io::sync_directory(parent)?,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(AdminError::FileWrite),
        }
        let identity = directory_identity(root)?;
        let _lock = lock(root, identity)?;
        // A newly created file supplies the effective owner's UID without an unsafe syscall.
        let probe = root.join(format!(".owner-check-{}", random_id()?));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&probe)
            .map_err(|_| AdminError::FileWrite)?;
        let owner = file.metadata().map(|m| m.uid());
        std::fs::remove_file(&probe).map_err(|_| AdminError::FileWrite)?;
        if owner.map_err(|_| AdminError::FileRead)? != identity.owner {
            return Err(AdminError::InvalidCandidate);
        }
        Ok(identity)
    }
    #[cfg(not(unix))]
    Err(AdminError::InvalidCandidate)
}

pub(super) fn lock(root: &Path, identity: DirectoryIdentity) -> Result<File, AdminError> {
    validate_ancestors(root)?;
    same_directory(directory_identity(root)?, identity)?;
    let lock = File::open(root).map_err(|_| AdminError::FileRead)?;
    same_directory(
        metadata_identity(&lock.metadata().map_err(|_| AdminError::FileRead)?)?,
        identity,
    )?;
    lock.lock().map_err(|_| AdminError::FileWrite)?;
    same_directory(directory_identity(root)?, identity)?;
    Ok(lock)
}

fn same_directory(
    actual: DirectoryIdentity,
    expected: DirectoryIdentity,
) -> Result<(), AdminError> {
    if actual.owner != expected.owner
        || actual.device != expected.device
        || actual.inode != expected.inode
    {
        return Err(AdminError::InvalidCandidate);
    }
    Ok(())
}

fn validate_components(path: &Path) -> Result<(), AdminError> {
    if !path.is_absolute()
        || path == Path::new("/")
        || path.as_os_str().as_encoded_bytes().contains(&0)
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(AdminError::InvalidCandidate);
    }
    Ok(())
}

fn validate_ancestors(path: &Path) -> Result<(), AdminError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        let metadata = std::fs::symlink_metadata(&current).map_err(|_| AdminError::FileRead)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(AdminError::InvalidCandidate);
        }
    }
    Ok(())
}

fn directory_identity(path: &Path) -> Result<DirectoryIdentity, AdminError> {
    metadata_identity(&std::fs::symlink_metadata(path).map_err(|_| AdminError::FileRead)?)
}

#[cfg(unix)]
fn metadata_identity(metadata: &Metadata) -> Result<DirectoryIdentity, AdminError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o7777 != 0o700
    {
        return Err(AdminError::InvalidCandidate);
    }
    Ok(DirectoryIdentity {
        owner: metadata.uid(),
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(not(unix))]
fn metadata_identity(_: &Metadata) -> Result<DirectoryIdentity, AdminError> {
    Err(AdminError::InvalidCandidate)
}

#[cfg(unix)]
fn private_metadata(metadata: &Metadata, owner: u32) -> Result<(), AdminError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != owner
        || metadata.permissions().mode() & 0o7777 != 0o600
        || metadata.len() > MAX_ALLOCATION_BYTES
    {
        return Err(AdminError::InvalidCandidate);
    }
    Ok(())
}

#[cfg(not(unix))]
fn private_metadata(_: &Metadata, _: u32) -> Result<(), AdminError> {
    Err(AdminError::InvalidCandidate)
}

pub(super) fn read(path: &Path, identity: DirectoryIdentity) -> Result<Vec<u8>, AdminError> {
    let before = std::fs::symlink_metadata(path).map_err(|_| AdminError::FileRead)?;
    private_metadata(&before, identity.owner)?;
    let file = File::open(path).map_err(|_| AdminError::FileRead)?;
    let opened = file.metadata().map_err(|_| AdminError::FileRead)?;
    private_metadata(&opened, identity.owner)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.dev() != opened.dev() || before.ino() != opened.ino() {
            return Err(AdminError::InvalidCandidate);
        }
    }
    let mut bytes = Vec::new();
    file.take(MAX_ALLOCATION_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AdminError::FileRead)?;
    if bytes.len() as u64 > MAX_ALLOCATION_BYTES {
        return Err(AdminError::InvalidCandidate);
    }
    Ok(bytes)
}
