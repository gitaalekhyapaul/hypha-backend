use std::fs::{DirBuilder, File, Metadata};
use std::io::Read;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use super::{BindingError, DatasetBinding, MAX_RECORD_BYTES, validate_key};

pub struct DatasetBindingStore {
    root: PathBuf,
    identity: Metadata,
}

impl DatasetBindingStore {
    /// Opens an existing owner-only directory without modifying it.
    pub fn open(root: PathBuf) -> Result<Self, BindingError> {
        if !root.is_absolute()
            || root == Path::new("/")
            || root
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return Err(BindingError::Invalid);
        }
        let mut prefix = PathBuf::new();
        for component in root.components() {
            prefix.push(component);
            let metadata =
                std::fs::symlink_metadata(&prefix).map_err(|_| BindingError::Unavailable)?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(BindingError::Invalid);
            }
        }
        let identity = directory(&root, None)?;
        Ok(Self { root, identity })
    }

    /// Exact retries are idempotent. An existing scope can never be redirected.
    /// Call only after owner approval with a freshly resolved approved package.
    pub fn retain(&self, binding: &DatasetBinding) -> Result<(), BindingError> {
        binding.validate()?;
        let lock = File::open(&self.root).map_err(|_| BindingError::Unavailable)?;
        lock.try_lock().map_err(|_| BindingError::Unavailable)?;
        self.check_root()?;
        same_file(
            &self.identity,
            &lock.metadata().map_err(|_| BindingError::Unavailable)?,
        )?;
        let parent = self.parent(&binding.policy_hash, &binding.resource_type, true)?;
        let path = parent.join(format!("{}.json", binding.resource_scope));
        match self.read(
            &binding.policy_hash,
            &binding.resource_type,
            &binding.resource_scope,
        ) {
            Ok(existing) if existing == *binding => return Ok(()),
            Ok(_) => return Err(BindingError::Conflict),
            Err(BindingError::NotFound) => {}
            Err(error) => return Err(error),
        }
        let bytes = serde_json::to_vec(binding).map_err(|_| BindingError::Invalid)?;
        crate::io::atomic_write_new(&path, &bytes).map_err(|_| BindingError::Unavailable)
    }

    pub fn read(
        &self,
        hash: &str,
        resource_type: &str,
        scope: &str,
    ) -> Result<DatasetBinding, BindingError> {
        validate_key(hash, resource_type, scope)?;
        self.check_root()?;
        let path = self
            .parent(hash, resource_type, false)?
            .join(format!("{scope}.json"));
        let metadata = std::fs::symlink_metadata(&path).map_err(read_error)?;
        self.private_file(&metadata)?;
        let file = File::open(&path).map_err(read_error)?;
        let opened = file.metadata().map_err(read_error)?;
        self.private_file(&opened)?;
        same_file(&metadata, &opened)?;
        let mut bytes = Vec::new();
        file.take(MAX_RECORD_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(read_error)?;
        if bytes.len() as u64 > MAX_RECORD_BYTES {
            return Err(BindingError::Invalid);
        }
        let binding: DatasetBinding =
            serde_json::from_slice(&bytes).map_err(|_| BindingError::Invalid)?;
        binding.validate()?;
        if binding.policy_hash != hash
            || binding.resource_type != resource_type
            || binding.resource_scope != scope
        {
            return Err(BindingError::Invalid);
        }
        Ok(binding)
    }

    fn check_root(&self) -> Result<(), BindingError> {
        same_file(
            &self.identity,
            &directory(&self.root, Some(self.identity.uid()))?,
        )
    }

    fn parent(
        &self,
        hash: &str,
        resource_type: &str,
        create: bool,
    ) -> Result<PathBuf, BindingError> {
        let mut path = self.root.clone();
        for part in [hash, resource_type] {
            let parent = path.clone();
            path.push(part);
            if create {
                match DirBuilder::new().mode(0o700).create(&path) {
                    Ok(()) => {
                        crate::io::sync_directory(&parent).map_err(|_| BindingError::Unavailable)?
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(_) => return Err(BindingError::Unavailable),
                }
            }
            directory(&path, Some(self.identity.uid()))?;
        }
        Ok(path)
    }

    fn private_file(&self, metadata: &Metadata) -> Result<(), BindingError> {
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.nlink() != 1
            || metadata.uid() != self.identity.uid()
            || metadata.permissions().mode() & 0o7777 != 0o600
            || metadata.len() > MAX_RECORD_BYTES
        {
            return Err(BindingError::Invalid);
        }
        Ok(())
    }
}

fn directory(path: &Path, owner: Option<u32>) -> Result<Metadata, BindingError> {
    let metadata = std::fs::symlink_metadata(path).map_err(read_error)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o7777 != 0o700
        || owner.is_some_and(|uid| uid != metadata.uid())
    {
        return Err(BindingError::Invalid);
    }
    Ok(metadata)
}

fn same_file(left: &Metadata, right: &Metadata) -> Result<(), BindingError> {
    if left.dev() != right.dev() || left.ino() != right.ino() || left.uid() != right.uid() {
        return Err(BindingError::Invalid);
    }
    Ok(())
}

fn read_error(error: std::io::Error) -> BindingError {
    if error.kind() == std::io::ErrorKind::NotFound {
        BindingError::NotFound
    } else {
        BindingError::Unavailable
    }
}
