use std::{
    fs::OpenOptions,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use crate::error::AdminError;

pub fn read_bounded(path: &Path, max_bytes: u64) -> Result<Vec<u8>, AdminError> {
    let file = std::fs::File::open(path).map_err(|_| AdminError::FileRead)?;
    let metadata = file.metadata().map_err(|_| AdminError::FileRead)?;
    if !metadata.is_file() || metadata.len() > max_bytes {
        return Err(AdminError::FileRead);
    }
    let limit = max_bytes.checked_add(1).ok_or(AdminError::FileRead)?;
    let mut bytes = Vec::new();
    file.take(limit)
        .read_to_end(&mut bytes)
        .map_err(|_| AdminError::FileRead)?;
    if bytes.len() as u64 > max_bytes {
        return Err(AdminError::FileRead);
    }
    Ok(bytes)
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), AdminError> {
    let parent = path.parent().ok_or(AdminError::FileWrite)?;
    std::fs::create_dir_all(parent).map_err(|_| AdminError::FileWrite)?;
    let temporary = temporary_path(path)?;
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|_| AdminError::FileWrite)?;
        file.write_all(bytes).map_err(|_| AdminError::FileWrite)?;
        file.sync_all().map_err(|_| AdminError::FileWrite)?;
        std::fs::rename(&temporary, path).map_err(|_| AdminError::FileWrite)?;
        sync_directory(parent)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

pub fn atomic_write_new(path: &Path, bytes: &[u8]) -> Result<(), AdminError> {
    let parent = path.parent().ok_or(AdminError::FileWrite)?;
    std::fs::create_dir_all(parent).map_err(|_| AdminError::FileWrite)?;
    let temporary = temporary_path(path)?;
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|_| AdminError::FileWrite)?;
        file.write_all(bytes).map_err(|_| AdminError::FileWrite)?;
        file.sync_all().map_err(|_| AdminError::FileWrite)?;
        std::fs::hard_link(&temporary, path).map_err(|_| AdminError::FileWrite)?;
        std::fs::remove_file(&temporary).map_err(|_| AdminError::FileWrite)?;
        sync_directory(parent)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

pub fn sync_directory(path: &Path) -> Result<(), AdminError> {
    std::fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| AdminError::FileWrite)
}

fn temporary_path(path: &Path) -> Result<PathBuf, AdminError> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(AdminError::FileWrite)?;
    Ok(path.with_file_name(format!(".{file_name}.tmp-{}", std::process::id())))
}

pub fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{atomic_write, atomic_write_new};
    use crate::error::AdminError;

    #[test]
    fn no_replace_writes_preserve_existing_records() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("raziel-admin-io-{}-{unique}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("record.pb");

        atomic_write_new(&path, b"first").unwrap();
        assert!(matches!(
            atomic_write_new(&path, b"second"),
            Err(AdminError::FileWrite)
        ));
        assert_eq!(std::fs::read(&path).unwrap(), b"first");

        atomic_write(&path, b"manifest replacement").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"manifest replacement");
        std::fs::remove_dir_all(root).unwrap();
    }
}
