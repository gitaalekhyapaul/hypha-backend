//! Owner-private snapshot recovery. A synced receipt precedes any registry append.

use std::fs::{DirBuilder, File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use prost::Message;
use raziel_proto::raziel::v1 as proto;

use super::{RequestSnapshotSource, SnapshotOperationError as Error};

const MAX_RECEIPT_BYTES: u64 = 2 * 1024 * 1024;
#[cfg(target_os = "linux")]
const NOFOLLOW: i32 = 0x20000;
#[cfg(target_os = "macos")]
const NOFOLLOW: i32 = 0x100;

#[derive(Clone, PartialEq, Message)]
struct Receipt {
    #[prost(uint32, tag = "1")]
    version: u32,
    #[prost(message, optional, tag = "2")]
    request: Option<proto::BuildSnapshotRequest>,
    #[prost(string, tag = "3")]
    receipt_key: String,
    #[prost(uint64, tag = "4")]
    proposal_generation: u64,
    #[prost(string, tag = "5")]
    policy_record_id: String,
    #[prost(string, tag = "6")]
    zed_token: String,
    #[prost(bytes = "vec", tag = "7")]
    source_schema_digest: Vec<u8>,
    #[prost(string, tag = "8")]
    accepted_composition_schema: String,
    #[prost(message, optional, tag = "9")]
    descriptor: Option<proto::RelationshipSnapshotDescriptor>,
}

pub(super) struct RequestReceipt {
    root: PathBuf,
    directory: File,
    identity: Metadata,
    _lock: File,
    expected: Receipt,
    name: String,
}

pub(super) struct RetainedSnapshot {
    pub request: proto::BuildSnapshotRequest,
    pub descriptor: proto::RelationshipSnapshotDescriptor,
}

impl RequestReceipt {
    pub fn open(
        root: &Path,
        request: &proto::BuildSnapshotRequest,
        source: &RequestSnapshotSource,
    ) -> Result<Self, Error> {
        if !crate::spicedb::composition::valid_receipt_key(&source.receipt_key)
            || source.proposal_generation == 0
        {
            return Err(Error::InvalidRequest);
        }
        check_ancestors(root.parent().ok_or(Error::PrivateStore)?)?;
        match DirBuilder::new().mode(0o700).create(root) {
            Ok(()) => File::open(root.parent().unwrap())
                .and_then(|f| f.sync_all())
                .map_err(storage)?,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(storage(e)),
        }
        check_ancestors(root)?;
        let identity = std::fs::symlink_metadata(root).map_err(storage)?;
        if !identity.is_dir() || identity.permissions().mode() & 0o777 != 0o700 {
            return Err(Error::PrivateStore);
        }
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(NOFOLLOW)
            .open(root)
            .map_err(storage)?;
        if !same_file(&identity, &directory.metadata().map_err(storage)?) {
            return Err(Error::PrivateStore);
        }
        let lock_path = root.join(".lock");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(NOFOLLOW)
            .open(&lock_path)
            .map_err(storage)?;
        let lock_meta = lock.metadata().map_err(storage)?;
        check_file(&lock_meta, identity.uid())?;
        if lock_meta.len() != 0
            || !same_file(
                &lock_meta,
                &std::fs::symlink_metadata(&lock_path).map_err(storage)?,
            )
        {
            return Err(Error::PrivateStore);
        }
        lock.try_lock().map_err(|_| Error::PrivateStore)?;
        let store = Self {
            root: root.to_owned(),
            directory,
            identity,
            _lock: lock,
            name: format!(
                "{}-{}.receipt",
                source.receipt_key, source.proposal_generation
            ),
            expected: Receipt {
                version: 1,
                request: Some(request.clone()),
                receipt_key: source.receipt_key.clone(),
                proposal_generation: source.proposal_generation,
                policy_record_id: source.policy_record_id.clone(),
                zed_token: source.zed_token.clone(),
                source_schema_digest: source.source_schema_digest.to_vec(),
                accepted_composition_schema: source.accepted_composition_schema.clone(),
                descriptor: None,
            },
        };
        if store.expected.encoded_len() > MAX_RECEIPT_BYTES as usize {
            return Err(Error::InvalidRequest);
        }
        store.verify()?;
        Ok(store)
    }

    pub fn read(&self) -> Result<Option<RetainedSnapshot>, Error> {
        self.verify()?;
        let path = self.root.join(&self.name);
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(value) => value,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(storage(e)),
        };
        check_file(&metadata, self.identity.uid())?;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(NOFOLLOW)
            .open(&path)
            .map_err(storage)?;
        if !same_file(&metadata, &file.metadata().map_err(storage)?) {
            return Err(Error::PrivateStore);
        }
        let mut bytes = Vec::new();
        file.take(MAX_RECEIPT_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(storage)?;
        if bytes.len() as u64 != metadata.len()
            || !same_file(
                &metadata,
                &std::fs::symlink_metadata(&path).map_err(storage)?,
            )
        {
            return Err(Error::PrivateStore);
        }
        let mut receipt = Receipt::decode(bytes.as_slice()).map_err(|_| Error::PrivateStore)?;
        if receipt.encode_to_vec() != bytes {
            return Err(Error::PrivateStore);
        }
        let descriptor = receipt.descriptor.take().ok_or(Error::PrivateStore)?;
        let request = receipt.request.clone().ok_or(Error::PrivateStore)?;
        let mut expected = self.expected.clone();
        // A retry can observe our already-published epoch. Only that observation
        // may change. All authority, scope, source and expiry inputs stay exact.
        expected
            .request
            .as_mut()
            .ok_or(Error::PrivateStore)?
            .expected_previous_epoch = request.expected_previous_epoch;
        if receipt != expected {
            return Err(Error::WriterConflict);
        }
        self.verify()?;
        Ok(Some(RetainedSnapshot {
            request,
            descriptor,
        }))
    }

    pub fn publish(&self, descriptor: &proto::RelationshipSnapshotDescriptor) -> Result<(), Error> {
        if let Some(existing) = self.read()? {
            return if existing.descriptor == *descriptor {
                Ok(())
            } else {
                Err(Error::WriterConflict)
            };
        }
        let mut receipt = self.expected.clone();
        receipt.descriptor = Some(descriptor.clone());
        let bytes = receipt.encode_to_vec();
        if bytes.len() > MAX_RECEIPT_BYTES as usize {
            return Err(Error::PrivateStore);
        }
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).map_err(|_| Error::Entropy)?;
        let temp = self
            .root
            .join(format!(".{}.tmp", super::lowercase_hex(&nonce)));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(NOFOLLOW)
            .open(&temp)
            .map_err(storage)?;
        let result = (|| {
            check_file(&file.metadata().map_err(storage)?, self.identity.uid())?;
            file.write_all(&bytes).map_err(storage)?;
            file.sync_all().map_err(storage)?;
            self.verify()?;
            std::fs::hard_link(&temp, self.root.join(&self.name)).map_err(storage)?;
            std::fs::remove_file(&temp).map_err(storage)?;
            self.directory.sync_all().map_err(storage)?;
            self.verify()
        })();
        let _ = std::fs::remove_file(temp);
        result
    }

    fn verify(&self) -> Result<(), Error> {
        let current = std::fs::symlink_metadata(&self.root).map_err(storage)?;
        if !current.is_dir()
            || current.permissions().mode() & 0o777 != 0o700
            || !same_file(&current, &self.identity)
            || !same_file(&current, &self.directory.metadata().map_err(storage)?)
        {
            return Err(Error::PrivateStore);
        }
        Ok(())
    }
}

fn same_file(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino() && left.uid() == right.uid()
}

fn check_file(metadata: &Metadata, uid: u32) -> Result<(), Error> {
    if !metadata.is_file()
        || metadata.uid() != uid
        || metadata.permissions().mode() & 0o777 != 0o600
        || metadata.nlink() != 1
        || metadata.len() > MAX_RECEIPT_BYTES
    {
        return Err(Error::PrivateStore);
    }
    Ok(())
}

fn check_ancestors(path: &Path) -> Result<(), Error> {
    if !path.is_absolute() {
        return Err(Error::PrivateStore);
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        if !matches!(component, Component::RootDir | Component::Normal(_)) {
            return Err(Error::PrivateStore);
        }
        current.push(component);
        let metadata = std::fs::symlink_metadata(&current).map_err(storage)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(Error::PrivateStore);
        }
    }
    Ok(())
}

fn storage(_: std::io::Error) -> Error {
    Error::PrivateStore
}
