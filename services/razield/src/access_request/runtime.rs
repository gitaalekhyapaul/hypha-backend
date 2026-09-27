use std::fmt;
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use raziel_admin::request_approval::ApprovalConfig;

use super::{AccessCoordinator, AccessError, AccessRequestHandler, bounded_access_service};

#[derive(Clone)]
pub struct AccessServiceConfig {
    bind: SocketAddr,
    private_root: PathBuf,
    redis_url: String,
    prefix: String,
    approval: Option<ApprovalConfig>,
}

impl AccessServiceConfig {
    pub fn from_env() -> Result<Self, AccessError> {
        let env = |key| std::env::var(key).map_err(|_| AccessError::Configuration);
        let mut config = Self::from_values(
            &env("RAZIEL_ACCESS_BIND_ADDR")?,
            &env("RAZIEL_ACCESS_PRIVATE_ROOT")?,
            &env("RAZIEL_REDIS_URL")?,
            &env("RAZIEL_ACCESS_PREFIX")?,
        )?;
        config.approval = approval_from_lookup(|key| match std::env::var(key) {
            Ok(value) => Ok(Some(value)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(_) => Err(AccessError::Configuration),
        })?;
        Ok(config)
    }

    pub fn from_values(
        bind: &str,
        private_root: &str,
        redis_url: &str,
        prefix: &str,
    ) -> Result<Self, AccessError> {
        let bind: SocketAddr = bind.parse().map_err(|_| AccessError::Configuration)?;
        let private_root = PathBuf::from(private_root);
        if bind.port() == 0
            || !private_root.is_absolute()
            || private_root == Path::new("/")
            || private_root
                .components()
                .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
            || prefix.is_empty()
            || prefix.len() > 64
            || !prefix
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
            || !redis_url.starts_with("redis://")
            || redis_url.bytes().any(|b| b.is_ascii_whitespace())
            || redis::Client::open(redis_url).is_err()
        {
            return Err(AccessError::Configuration);
        }
        Ok(Self {
            bind,
            private_root,
            redis_url: redis_url.to_owned(),
            prefix: prefix.to_owned(),
            approval: None,
        })
    }

    pub fn bind_address(&self) -> SocketAddr {
        self.bind
    }
    pub fn private_root(&self) -> &Path {
        &self.private_root
    }
    pub fn prefix(&self) -> &str {
        &self.prefix
    }
    pub(crate) fn redis_url(&self) -> &str {
        &self.redis_url
    }
    pub(crate) fn approval(&self) -> Option<&ApprovalConfig> {
        self.approval.as_ref()
    }

    pub fn with_approval_config(mut self, approval: ApprovalConfig) -> Result<Self, AccessError> {
        validate_approval_config(&approval)?;
        self.approval = Some(approval);
        Ok(self)
    }
}

fn approval_from_lookup(
    env: impl Fn(&str) -> Result<Option<String>, AccessError>,
) -> Result<Option<ApprovalConfig>, AccessError> {
    match env("RAZIEL_ACCESS_APPROVAL_MODE")?.as_deref() {
        None => {
            for key in [
                "RAZIEL_POLICY_PUBLICATION_ROOT",
                "RAZIEL_POLICY_PACKAGE_ROOT",
                "RAZIEL_DATASET_BINDING_ROOT",
            ] {
                if env(key)?.is_some() {
                    return Err(AccessError::Configuration);
                }
            }
            return Ok(None);
        }
        Some("installed") => {}
        _ => return Err(AccessError::Configuration),
    }
    let required = |key| env(key)?.ok_or(AccessError::Configuration);
    let config = ApprovalConfig {
        publication_root: required("RAZIEL_POLICY_PUBLICATION_ROOT")?.into(),
        package_root: required("RAZIEL_POLICY_PACKAGE_ROOT")?.into(),
        registry_root: required("RAZIEL_REGISTRY_PATH")?.into(),
        binding_root: required("RAZIEL_DATASET_BINDING_ROOT")?.into(),
        trust_domain_id: required("RAZIEL_TRUST_DOMAIN_ID")?,
    };
    validate_approval_config(&config)?;
    Ok(Some(config))
}

fn validate_approval_config(config: &ApprovalConfig) -> Result<(), AccessError> {
    for path in [
        &config.publication_root,
        &config.package_root,
        &config.registry_root,
        &config.binding_root,
    ] {
        if !path.is_absolute()
            || path == Path::new("/")
            || path
                .components()
                .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
        {
            return Err(AccessError::Configuration);
        }
    }
    if config.trust_domain_id.is_empty()
        || config.trust_domain_id.len() > 128
        || !config
            .trust_domain_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
    {
        return Err(AccessError::Configuration);
    }
    Ok(())
}

impl fmt::Debug for AccessServiceConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccessServiceConfig")
            .field("bind", &self.bind)
            .field("prefix", &self.prefix)
            .field("private_root", &"<redacted>")
            .field("redis_url", &"<redacted>")
            .field("approval_enabled", &self.approval.is_some())
            .finish()
    }
}

/// Bind only on the owner network. Compose must not publish this RPC's host port.
pub async fn run_from_env() -> Result<(), AccessError> {
    let config = AccessServiceConfig::from_env()?;
    let bind = config.bind_address();
    let coordinator = tokio::task::spawn_blocking(move || AccessCoordinator::open(&config))
        .await
        .map_err(|_| AccessError::ServiceUnavailable)??;
    let handler = AccessRequestHandler::new(coordinator);
    handler
        .reconcile()
        .await
        .map_err(|_| AccessError::ServiceUnavailable)?;
    let maintenance = handler.clone();
    let (stop, mut stopping) = tokio::sync::watch::channel(false);
    let background = tokio::spawn(async move {
        let mut timer = tokio::time::interval(Duration::from_secs(30));
        loop {
            tokio::select! {
                _ = stopping.changed() => break,
                _ = timer.tick() => {
                    if maintenance.reconcile().await.is_err() {
                        eprintln!("access request maintenance unavailable");
                    }
                }
            }
        }
    });
    let result = tonic::transport::Server::builder()
        .concurrency_limit_per_connection(32)
        .timeout(Duration::from_secs(15))
        .add_service(bounded_access_service(handler))
        .serve_with_shutdown(bind, shutdown())
        .await
        .map_err(|_| AccessError::ServiceUnavailable);
    let _ = stop.send(true);
    let _ = background.await;
    result
}

async fn shutdown() {
    use tokio::signal::unix::{SignalKind, signal};
    match signal(SignalKind::terminate()) {
        Ok(mut terminate) => {
            tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
        }
        Err(_) => {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approval_mode_is_explicit_and_installed_configuration_is_complete() {
        let values = [
            ("RAZIEL_ACCESS_APPROVAL_MODE", "installed"),
            ("RAZIEL_POLICY_PUBLICATION_ROOT", "/owner/publication"),
            ("RAZIEL_POLICY_PACKAGE_ROOT", "/owner/packages"),
            ("RAZIEL_REGISTRY_PATH", "/owner/registry"),
            ("RAZIEL_DATASET_BINDING_ROOT", "/owner/bindings"),
            ("RAZIEL_TRUST_DOMAIN_ID", "local-test"),
        ];
        let lookup = |key: &str| {
            Ok(values
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| (*value).to_owned()))
        };
        assert!(approval_from_lookup(|_| Ok(None)).unwrap().is_none());
        assert!(matches!(
            approval_from_lookup(|key| if key == "RAZIEL_ACCESS_APPROVAL_MODE" {
                Ok(None)
            } else {
                lookup(key)
            }),
            Err(AccessError::Configuration)
        ));
        assert!(approval_from_lookup(lookup).unwrap().is_some());
        for (missing, _) in values.iter().skip(1) {
            assert!(matches!(
                approval_from_lookup(|key| if key == *missing {
                    Ok(None)
                } else {
                    lookup(key)
                }),
                Err(AccessError::Configuration)
            ));
        }
        for mode in ["", "legacy", "mock"] {
            assert!(matches!(
                approval_from_lookup(|key| if key == "RAZIEL_ACCESS_APPROVAL_MODE" {
                    Ok(Some(mode.into()))
                } else {
                    lookup(key)
                }),
                Err(AccessError::Configuration)
            ));
        }
        for invalid in ["relative", "/", "/owner/../other"] {
            assert!(matches!(
                approval_from_lookup(|key| if key == "RAZIEL_POLICY_PACKAGE_ROOT" {
                    Ok(Some(invalid.into()))
                } else {
                    lookup(key)
                }),
                Err(AccessError::Configuration)
            ));
        }
    }
}
