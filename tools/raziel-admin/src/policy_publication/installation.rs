use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::policy_activation::store::StoreActivationError as Error;
use crate::policy_package::{PolicyPackageKey, PolicyPackageStore, PolicyVersionIdentity};

use super::PublicationConfig;
use super::compiler::{Compiler, Composition, Input};
use super::files::{self, Artifacts};
use super::network::SchemaApi;

const MAX_STATE_BYTES: u64 = 16 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct Key {
    version_id: String,
    approval_epoch: u64,
}

impl Key {
    pub(super) fn key(&self) -> Result<PolicyPackageKey, Error> {
        PolicyPackageKey::new(self.version_id.clone(), self.approval_epoch)
            .map_err(|_| Error::CandidateRejected)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct State {
    format_version: u32,
    generation: u64,
    pub(super) packages: Vec<Key>,
    legacy: bool,
    schema_sha256: String,
}

impl State {
    fn validate(&self) -> Result<(), Error> {
        let mut ids = BTreeSet::new();
        if self.format_version != 1
            || self.packages.len() > 32
            || self.schema_sha256.len() != 64
            || !self
                .schema_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::CandidateRejected);
        }
        for key in &self.packages {
            key.key()?;
            if !ids.insert(&key.version_id) {
                return Err(Error::Conflict);
            }
        }
        if self
            .packages
            .windows(2)
            .any(|pair| pair[0].version_id >= pair[1].version_id)
        {
            return Err(Error::Conflict);
        }
        Ok(())
    }
}

pub(super) struct Target {
    pub version_id: String,
    pub approval_epoch: u64,
    state: State,
    composition: Composition,
}

pub(super) struct Installation {
    root: PathBuf,
    current: State,
    composition: Composition,
    legacy: Option<Artifacts>,
}

impl Installation {
    pub(super) fn load_for_request(
        root: &std::path::Path,
        registry_root: &std::path::Path,
        trust_domain_id: &str,
        packages: &mut PolicyPackageStore,
        compiler: &Compiler,
        now: i64,
    ) -> Result<Self, Error> {
        match std::fs::symlink_metadata(root.join("pending.json")) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(Error::Unavailable),
            Ok(_) => return Err(Error::Conflict),
        }
        if load_state(root, "installed.json")?.is_none() {
            return Err(Error::Conflict);
        }
        Self::load_at(
            root,
            registry_root,
            trust_domain_id,
            None,
            packages,
            compiler,
            now,
        )
    }

    pub(super) fn contains(&self, key: &PolicyPackageKey) -> bool {
        self.current
            .packages
            .iter()
            .any(|installed| installed.key().is_ok_and(|value| value == *key))
    }

    pub(super) fn composition(&self) -> &Composition {
        &self.composition
    }

    pub fn load(
        c: &PublicationConfig,
        packages: &mut PolicyPackageStore,
        compiler: &Compiler,
        now: i64,
    ) -> Result<Self, Error> {
        Self::load_at(
            &c.root,
            &c.registry_root,
            &c.trust_domain_id,
            c.legacy_root.as_deref(),
            packages,
            compiler,
            now,
        )
    }

    fn load_at(
        root: &std::path::Path,
        registry_root: &std::path::Path,
        trust_domain_id: &str,
        supplied_legacy: Option<&std::path::Path>,
        packages: &mut PolicyPackageStore,
        compiler: &Compiler,
        now: i64,
    ) -> Result<Self, Error> {
        let legacy_root = root.join("legacy");
        let legacy = if legacy_root.try_exists().map_err(|_| Error::Unavailable)? {
            Some(Artifacts::load(&legacy_root)?)
        } else if let Some(root) = supplied_legacy {
            let artifacts = Artifacts::load(root)?;
            artifacts.validate_legacy(registry_root, trust_domain_id, now)?;
            artifacts.persist(&legacy_root)?;
            Some(artifacts)
        } else {
            None
        };
        if let Some(legacy) = &legacy {
            legacy.validate_legacy(registry_root, trust_domain_id, now)?;
        }
        let stored = load_state(root, "installed.json")?;
        let mut current = stored.clone().unwrap_or(State {
            format_version: 1,
            generation: 0,
            packages: Vec::new(),
            legacy: legacy.is_some(),
            schema_sha256: String::new(),
        });
        if current.legacy != legacy.is_some() {
            return Err(Error::Conflict);
        }
        let composition = compile_state(&current, legacy.as_ref(), packages, compiler, now)?;
        if stored.is_some() && current.schema_sha256 != composition.digest() {
            return Err(Error::Conflict);
        }
        current.schema_sha256 = composition.digest();
        current.validate()?;
        Ok(Self {
            root: root.into(),
            current,
            composition,
            legacy,
        })
    }

    pub async fn check_live(
        &self,
        api: &mut impl SchemaApi,
        compiler: &Compiler,
    ) -> Result<(), Error> {
        if !compiler.equivalent(&self.composition.schema, &api.read().await?)? {
            return Err(Error::Conflict);
        }
        Ok(())
    }

    pub fn prepare(
        &self,
        candidate: &Artifacts,
        identity: &PolicyVersionIdentity,
        packages: &mut PolicyPackageStore,
        compiler: &Compiler,
        now: i64,
    ) -> Result<Target, Error> {
        let epoch = candidate.manifest()?.expected_approval_epoch;
        let key = Key {
            version_id: identity.version_id().into(),
            approval_epoch: epoch,
        };
        key.key()?;
        let mut state = self.current.clone();
        if let Some(existing) = state
            .packages
            .iter_mut()
            .find(|k| k.version_id == key.version_id)
        {
            if key.approval_epoch < existing.approval_epoch {
                return Err(Error::Conflict);
            }
            *existing = key;
        } else {
            state.packages.push(key);
            state
                .packages
                .sort_by(|a, b| a.version_id.cmp(&b.version_id));
        }
        state.validate()?;
        let mut inputs = inputs_for(&self.current, self.legacy.as_ref(), packages, now)?;
        if inputs.iter().any(|input| {
            input.version_id.as_deref() == Some(identity.version_id())
                && (input.artifacts.schema != candidate.schema
                    || input.artifacts.config != candidate.config
                    || input.artifacts.policy != candidate.policy)
        }) {
            return Err(Error::Conflict);
        }
        inputs.retain(|input| input.version_id.as_deref() != Some(identity.version_id()));
        inputs.push(Input {
            version_id: Some(identity.version_id().into()),
            artifacts: Artifacts {
                schema: candidate.schema.clone(),
                config: candidate.config.clone(),
                policy: candidate.policy.clone(),
                manifest: candidate.manifest.clone(),
            },
        });
        let composition = compiler.compose(&inputs)?;
        if state.packages != self.current.packages {
            state.generation = state.generation.checked_add(1).ok_or(Error::Unavailable)?;
        } else if composition.digest() != self.current.schema_sha256 {
            return Err(Error::Conflict);
        }
        state.schema_sha256 = composition.digest();
        Ok(Target {
            version_id: identity.version_id().into(),
            approval_epoch: epoch,
            state,
            composition,
        })
    }

    pub async fn reconcile(
        &mut self,
        api: &mut impl SchemaApi,
        packages: &mut PolicyPackageStore,
        compiler: &Compiler,
        now: i64,
    ) -> Result<(), Error> {
        let Some(pending) = load_state(&self.root, "pending.json")? else {
            return Ok(());
        };
        if pending == self.current {
            self.check_live(api, compiler).await?;
            return self.clear_pending();
        }
        if pending.generation
            != self
                .current
                .generation
                .checked_add(1)
                .ok_or(Error::Conflict)?
            || pending.legacy != self.current.legacy
            || self.current.packages.iter().any(|old| {
                !pending.packages.iter().any(|new| {
                    new.version_id == old.version_id && new.approval_epoch >= old.approval_epoch
                })
            })
        {
            return Err(Error::Conflict);
        }
        let composition = compile_state(&pending, self.legacy.as_ref(), packages, compiler, now)?;
        if composition.digest() != pending.schema_sha256 {
            return Err(Error::Conflict);
        }
        self.finish_install(pending, composition, api, compiler)
            .await
    }

    pub async fn install(
        &mut self,
        target: Target,
        api: &mut impl SchemaApi,
        compiler: &Compiler,
    ) -> Result<(), Error> {
        if target.state == self.current {
            return self.check_live(api, compiler).await;
        }
        target.state.validate()?;
        let bytes = serde_json::to_vec(&target.state).map_err(|_| Error::Unavailable)?;
        files::write_new(&self.root, "pending.json", &bytes)?;
        self.finish_install(target.state, target.composition, api, compiler)
            .await
    }

    async fn finish_install(
        &mut self,
        target: State,
        composition: Composition,
        api: &mut impl SchemaApi,
        compiler: &Compiler,
    ) -> Result<(), Error> {
        let live = api.read().await?;
        if !compiler.equivalent(&composition.schema, &live)? {
            if !compiler.equivalent(&self.composition.schema, &live)? {
                return Err(Error::Conflict);
            }
            api.write(&composition.schema).await?;
            if !compiler.equivalent(&composition.schema, &api.read().await?)? {
                return Err(Error::Conflict);
            }
        }
        let bytes = serde_json::to_vec(&target).map_err(|_| Error::Unavailable)?;
        crate::io::atomic_write(&self.root.join("installed.json"), &bytes)
            .map_err(|_| Error::Unavailable)?;
        self.current = target;
        self.composition = composition;
        self.clear_pending()
    }

    fn clear_pending(&self) -> Result<(), Error> {
        std::fs::remove_file(self.root.join("pending.json")).map_err(|_| Error::Unavailable)?;
        crate::io::sync_directory(&self.root).map_err(|_| Error::Unavailable)
    }
}

pub(super) fn load_state(root: &std::path::Path, name: &str) -> Result<Option<State>, Error> {
    match std::fs::symlink_metadata(root.join(name)) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error::Unavailable),
    }
    let bytes = files::read(root, name, MAX_STATE_BYTES)?;
    let state: State = serde_json::from_slice(&bytes).map_err(|_| Error::CandidateRejected)?;
    state.validate()?;
    Ok(Some(state))
}

fn compile_state(
    state: &State,
    legacy: Option<&Artifacts>,
    packages: &mut PolicyPackageStore,
    compiler: &Compiler,
    now: i64,
) -> Result<Composition, Error> {
    compiler.compose(&inputs_for(state, legacy, packages, now)?)
}

fn inputs_for(
    state: &State,
    legacy: Option<&Artifacts>,
    packages: &mut PolicyPackageStore,
    now: i64,
) -> Result<Vec<Input>, Error> {
    let mut inputs = Vec::new();
    if state.legacy {
        let legacy = legacy.ok_or(Error::Conflict)?;
        inputs.push(Input {
            version_id: None,
            artifacts: Artifacts {
                schema: legacy.schema.clone(),
                config: legacy.config.clone(),
                policy: legacy.policy.clone(),
                manifest: legacy.manifest.clone(),
            },
        });
    }
    for key in &state.packages {
        let package = packages
            .read_retained(&key.key()?, now)
            .map_err(|_| Error::CandidateRejected)?;
        inputs.push(Input {
            version_id: Some(key.version_id.clone()),
            artifacts: Artifacts {
                schema: package.source_schema().into(),
                config: package.compiler_config().into(),
                policy: package.policy_pb().into(),
                manifest: package.candidate_manifest().into(),
            },
        });
    }
    Ok(inputs)
}
