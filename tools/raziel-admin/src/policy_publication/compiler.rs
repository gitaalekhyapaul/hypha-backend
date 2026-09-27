use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::policy_activation::store::StoreActivationError as Error;

use super::files::{self, Artifacts, MAX_SCHEMA};

pub(super) struct Compiler {
    executable: PathBuf,
    root: PathBuf,
}

pub(super) struct Input {
    pub version_id: Option<String>,
    pub artifacts: Artifacts,
}

pub(super) struct Composition {
    pub schema: Vec<u8>,
    pub reflection: Vec<u8>,
}

impl Composition {
    pub fn digest(&self) -> String {
        crate::io::lowercase_hex(&Sha256::digest(&self.schema))
    }
}

#[derive(Serialize)]
struct Manifest {
    format_version: u32,
    packages: Vec<Package>,
}

#[derive(Serialize)]
struct Package {
    #[serde(skip_serializing_if = "Option::is_none")]
    version_id: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    legacy: bool,
    schema_path: PathBuf,
    config_path: PathBuf,
    expected_policy_ir_path: PathBuf,
}

impl Compiler {
    pub fn new(executable: &Path, root: &Path) -> Result<Self, Error> {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::symlink_metadata(executable).map_err(|_| Error::Unavailable)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.permissions().mode() & 0o111 == 0
        {
            return Err(Error::Unavailable);
        }
        Ok(Self {
            executable: executable.into(),
            root: root.into(),
        })
    }

    pub fn compose(&self, inputs: &[Input]) -> Result<Composition, Error> {
        if inputs.is_empty() {
            return Ok(Composition {
                schema: Vec::new(),
                reflection: Vec::new(),
            });
        }
        if inputs.len() > 33 {
            return Err(Error::CandidateRejected);
        }
        let scratch = Scratch::new(&self.root)?;
        let mut packages = Vec::new();
        for (index, input) in inputs.iter().enumerate() {
            let root = scratch.0.join(index.to_string());
            input.artifacts.persist(&root)?;
            packages.push(Package {
                version_id: input.version_id.clone(),
                legacy: input.version_id.is_none(),
                schema_path: root.join("schema.zed"),
                config_path: root.join("compiler.json"),
                expected_policy_ir_path: root.join("policy.pb"),
            });
        }
        let manifest = serde_json::to_vec(&Manifest {
            format_version: 1,
            packages,
        })
        .map_err(|_| Error::Unavailable)?;
        files::write_new(&scratch.0, "manifest.json", &manifest)?;
        self.run(&[
            "compose".into(),
            "--manifest".into(),
            scratch.0.join("manifest.json"),
            "--out".into(),
            scratch.0.join("schema.zed"),
            "--reflection-out".into(),
            scratch.0.join("reflection.json"),
            "--compiler-version".into(),
            "1.54.0".into(),
        ])?;
        let result = Composition {
            schema: files::read(&scratch.0, "schema.zed", MAX_SCHEMA)?,
            reflection: files::read(&scratch.0, "reflection.json", MAX_SCHEMA)?,
        };
        if result.schema.len() + result.reflection.len() > 2 * MAX_SCHEMA as usize {
            return Err(Error::CandidateRejected);
        }
        Ok(result)
    }

    pub fn equivalent(&self, expected: &[u8], actual: &[u8]) -> Result<bool, Error> {
        if expected.len() > MAX_SCHEMA as usize || actual.len() > MAX_SCHEMA as usize {
            return Err(Error::CandidateRejected);
        }
        if expected.is_empty() || actual.is_empty() {
            return Ok(expected == actual);
        }
        let scratch = Scratch::new(&self.root)?;
        files::write_new(&scratch.0, "expected.zed", expected)?;
        files::write_new(&scratch.0, "actual.zed", actual)?;
        match self.run(&[
            "compare-schema".into(),
            "--expected".into(),
            scratch.0.join("expected.zed"),
            "--actual".into(),
            scratch.0.join("actual.zed"),
            "--compiler-version".into(),
            "1.54.0".into(),
        ]) {
            Ok(()) => Ok(true),
            Err(Error::CandidateRejected) => Ok(false),
            Err(error) => Err(error),
        }
    }

    fn run(&self, args: &[PathBuf]) -> Result<(), Error> {
        // Compiler diagnostics contain private policy expressions. Never relay them.
        let mut child = Command::new(&self.executable)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| Error::Unavailable)?;
        let start = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    return if status.success() {
                        Ok(())
                    } else {
                        Err(Error::CandidateRejected)
                    };
                }
                Ok(None) if start.elapsed() < Duration::from_secs(30) => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(Error::Unavailable);
                }
            }
        }
    }
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(root: &Path) -> Result<Self, Error> {
        let mut nonce = [0_u8; 16];
        getrandom::fill(&mut nonce).map_err(|_| Error::Unavailable)?;
        let path = root.join(format!(".compile-{}", crate::io::lowercase_hex(&nonce)));
        files::create_private_directory(&path)?;
        Ok(Self(path))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
