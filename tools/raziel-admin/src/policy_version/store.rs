use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::{AdminError, io::atomic_write_new};

use super::{
    AllocatedPolicyVersion, MAX_ALLOCATION_BYTES, MAX_ALLOCATION_RECORDS, StoredAllocation,
    private::{self, DirectoryIdentity},
    random_id, valid_candidate_id, validate_request,
};

pub struct PolicyVersionStore {
    root: PathBuf,
    directory: DirectoryIdentity,
    writable: bool,
}

impl PolicyVersionStore {
    /// Creates only the leaf directory, or validates an existing owner-private 0700 root.
    /// Its parent must exist. No allocation cleanup or number reuse is permitted.
    pub fn new(root: PathBuf) -> Result<Self, AdminError> {
        let directory = private::initialize(&root)?;
        Ok(Self {
            root,
            directory,
            writable: true,
        })
    }

    /// Opens retained allocations without creating files, including on read-only mounts.
    pub fn open_read_only(root: PathBuf) -> Result<Self, AdminError> {
        let directory = private::open_existing(&root)?;
        Ok(Self {
            root,
            directory,
            writable: false,
        })
    }

    pub fn allocate(
        &self,
        candidate_id: &str,
        request_digest: &str,
        requested_family_id: Option<&str>,
    ) -> Result<AllocatedPolicyVersion, AdminError> {
        if !self.writable {
            return Err(AdminError::InvalidCandidate);
        }
        validate_request(candidate_id, request_digest, requested_family_id)?;
        let _lock = private::lock(&self.root, self.directory)?;
        let allocations = self.allocations()?;
        if let Some(existing) = allocations.iter().find(|r| r.candidate_id == candidate_id) {
            if existing.request_digest != request_digest
                || existing.requested_family_id.as_deref() != requested_family_id
            {
                return Err(AdminError::InvalidCandidate);
            }
            return Ok(existing.clone());
        }
        if allocations.len() >= MAX_ALLOCATION_RECORDS {
            return Err(AdminError::InvalidCandidate);
        }
        let (family_id, version) = match requested_family_id {
            Some(family) => {
                let maximum = allocations
                    .iter()
                    .filter(|r| r.identity.family_id() == family)
                    .map(|r| r.identity.version())
                    .max()
                    .ok_or(AdminError::InvalidCandidate)?;
                let version = maximum
                    .checked_add(1)
                    .filter(|v| *v <= i32::MAX as u32)
                    .ok_or(AdminError::InvalidCandidate)?;
                (family.to_owned(), version)
            }
            None => (unique_id(&allocations)?, 1),
        };
        let version_id = unique_id(&allocations)?;
        if version_id == family_id {
            return Err(AdminError::FileWrite);
        }
        let stored = StoredAllocation {
            format_version: 1,
            candidate_id: candidate_id.into(),
            request_digest: request_digest.into(),
            requested_family_id: requested_family_id.map(str::to_owned),
            family_id,
            version_id,
            version,
        };
        let bytes = serde_json::to_vec(&stored).map_err(|_| AdminError::InvalidCandidate)?;
        if bytes.len() as u64 > MAX_ALLOCATION_BYTES {
            return Err(AdminError::InvalidCandidate);
        }
        atomic_write_new(&self.path(candidate_id), &bytes)?;
        stored.validate()
    }

    pub fn read(&self, candidate_id: &str) -> Result<AllocatedPolicyVersion, AdminError> {
        if !valid_candidate_id(candidate_id) {
            return Err(AdminError::InvalidCandidate);
        }
        let _lock = private::lock(&self.root, self.directory)?;
        self.read_locked(candidate_id)
    }

    fn read_locked(&self, candidate_id: &str) -> Result<AllocatedPolicyVersion, AdminError> {
        let bytes = private::read(&self.path(candidate_id), self.directory)?;
        let stored: StoredAllocation =
            serde_json::from_slice(&bytes).map_err(|_| AdminError::InvalidCandidate)?;
        let allocated = stored.validate()?;
        if allocated.candidate_id != candidate_id {
            return Err(AdminError::InvalidCandidate);
        }
        Ok(allocated)
    }

    fn allocations(&self) -> Result<Vec<AllocatedPolicyVersion>, AdminError> {
        let mut records = Vec::new();
        let mut versions = BTreeSet::new();
        let mut numbers = BTreeSet::new();
        let mut families = BTreeMap::<String, bool>::new();
        for entry in std::fs::read_dir(&self.root).map_err(|_| AdminError::FileRead)? {
            let entry = entry.map_err(|_| AdminError::FileRead)?;
            let name = entry.file_name();
            let name = name.to_str().ok_or(AdminError::InvalidCandidate)?;
            if records.len() >= MAX_ALLOCATION_RECORDS {
                return Err(AdminError::InvalidCandidate);
            }
            let candidate = name
                .strip_suffix(".json")
                .ok_or(AdminError::InvalidCandidate)?;
            if !valid_candidate_id(candidate) {
                return Err(AdminError::InvalidCandidate);
            }
            let record = self.read_locked(candidate)?;
            if !versions.insert(record.identity.version_id().to_owned())
                || !numbers.insert((
                    record.identity.family_id().to_owned(),
                    record.identity.version(),
                ))
            {
                return Err(AdminError::InvalidCandidate);
            }
            let has_initial = families
                .entry(record.identity.family_id().into())
                .or_default();
            *has_initial |= record.requested_family_id.is_none();
            records.push(record);
        }
        if families.values().any(|has_initial| !has_initial) {
            return Err(AdminError::InvalidCandidate);
        }
        Ok(records)
    }

    fn path(&self, candidate_id: &str) -> PathBuf {
        self.root.join(format!("{candidate_id}.json"))
    }
}

fn unique_id(records: &[AllocatedPolicyVersion]) -> Result<String, AdminError> {
    for _ in 0..8 {
        let id = random_id()?;
        if records
            .iter()
            .all(|r| r.identity.family_id() != id && r.identity.version_id() != id)
        {
            return Ok(id);
        }
    }
    Err(AdminError::FileWrite)
}
