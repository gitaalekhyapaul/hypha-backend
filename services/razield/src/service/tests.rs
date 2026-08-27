use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use super::clock::system_time_to_unix_seconds;
use super::{
    ClockError, MAX_SERVICE_MESSAGE_BYTES, ORPHAN_GRACE_PERIOD, ServiceConfig, ServiceConfigError,
    SnapshotServiceConfig, SnapshotServiceConfigError,
};

#[test]
fn valid_configuration_has_fixed_limits_and_redacts_owner_state() {
    let config = ServiceConfig::from_values(
        "127.0.0.1:50052",
        "redis://raziel:private-password@example.invalid:6379/0",
        "raziel-capabilities",
        "/var/lib/raziel/private-envelopes",
    )
    .unwrap();

    assert_eq!(
        config.bind_address(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 50_052)
    );
    assert_eq!(config.queue_prefix(), "raziel-capabilities");
    assert_eq!(
        config.private_envelope_root(),
        Path::new("/var/lib/raziel/private-envelopes")
    );
    assert_eq!(MAX_SERVICE_MESSAGE_BYTES, 2 * 1024 * 1024);
    assert_eq!(ORPHAN_GRACE_PERIOD, Duration::from_secs(10 * 60));

    let debug = format!("{config:?}");
    assert!(debug.contains("127.0.0.1:50052"));
    assert!(debug.contains("raziel-capabilities"));
    assert!(!debug.contains("private-password"));
    assert!(!debug.contains("example.invalid"));
    assert!(!debug.contains("private-envelopes"));
    assert!(debug.matches("<redacted>").count() >= 2);
}

#[test]
fn configuration_rejects_unsafe_or_ambiguous_values() {
    let valid = || {
        ServiceConfig::from_values(
            "127.0.0.1:50052",
            "redis://redis:6379/0",
            "raziel-capabilities",
            "/var/lib/raziel/private-envelopes",
        )
    };
    assert!(valid().is_ok());

    assert_eq!(
        ServiceConfig::from_values(
            "localhost:50052",
            "redis://redis:6379/0",
            "raziel-capabilities",
            "/var/lib/raziel/private-envelopes",
        ),
        Err(ServiceConfigError::InvalidBindAddress)
    );
    assert_eq!(
        ServiceConfig::from_values(
            "127.0.0.1:0",
            "redis://redis:6379/0",
            "raziel-capabilities",
            "/var/lib/raziel/private-envelopes",
        ),
        Err(ServiceConfigError::InvalidBindAddress)
    );
    assert_eq!(
        ServiceConfig::from_values(
            "127.0.0.1:50052",
            "http://redis:6379/0",
            "raziel-capabilities",
            "/var/lib/raziel/private-envelopes",
        ),
        Err(ServiceConfigError::InvalidRedisUrl)
    );
    assert_eq!(
        ServiceConfig::from_values(
            "127.0.0.1:50052",
            "redis://redis:6379/0 hidden",
            "raziel-capabilities",
            "/var/lib/raziel/private-envelopes",
        ),
        Err(ServiceConfigError::InvalidRedisUrl)
    );
    assert_eq!(
        ServiceConfig::from_values(
            "127.0.0.1:50052",
            "redis://redis:6379/0",
            "raziel capabilities",
            "/var/lib/raziel/private-envelopes",
        ),
        Err(ServiceConfigError::InvalidQueuePrefix)
    );
    assert_eq!(
        ServiceConfig::from_values(
            "127.0.0.1:50052",
            "redis://redis:6379/0",
            "raziel-capabilities",
            "relative/private-envelopes",
        ),
        Err(ServiceConfigError::InvalidPrivateEnvelopeRoot)
    );
    assert_eq!(
        ServiceConfig::from_values(
            "127.0.0.1:50052",
            "redis://redis:6379/0",
            "raziel-capabilities",
            "/",
        ),
        Err(ServiceConfigError::InvalidPrivateEnvelopeRoot)
    );
}

#[test]
fn trusted_clock_uses_whole_seconds_and_rejects_invalid_system_time() {
    let with_subseconds = UNIX_EPOCH + Duration::new(1_700_000_000, 999_999_999);
    assert_eq!(
        system_time_to_unix_seconds(with_subseconds),
        Ok(1_700_000_000)
    );

    assert_eq!(
        system_time_to_unix_seconds(UNIX_EPOCH - Duration::from_secs(1)),
        Err(ClockError::BeforeUnixEpoch)
    );

    let too_late = UNIX_EPOCH + Duration::from_secs(253_402_300_800);
    assert_eq!(
        system_time_to_unix_seconds(too_late),
        Err(ClockError::OutOfRange)
    );
}

#[test]
fn environment_loader_requires_all_explicit_owner_settings() {
    let values = HashMap::from([
        ("RAZIEL_BIND_ADDRESS", "127.0.0.1:50052"),
        ("RAZIEL_REDIS_URL", "redis://redis:6379/0"),
        ("RAZIEL_QUEUE_PREFIX", "raziel-capabilities"),
        (
            "RAZIEL_PRIVATE_ENVELOPE_ROOT",
            "/var/lib/raziel/private-envelopes",
        ),
    ]);
    let config =
        ServiceConfig::from_lookup(|key| values.get(key).map(ToString::to_string)).unwrap();
    assert_eq!(config.bind_address().port(), 50_052);

    let missing_prefix = ServiceConfig::from_lookup(|key| {
        (key != "RAZIEL_QUEUE_PREFIX")
            .then(|| values.get(key).map(ToString::to_string))
            .flatten()
    });
    assert_eq!(missing_prefix, Err(ServiceConfigError::InvalidQueuePrefix));
}

#[test]
fn snapshot_configuration_uses_fixed_refs_and_redacts_secret_paths() {
    let values = snapshot_values();
    let config =
        SnapshotServiceConfig::from_lookup(|key| values.get(key).map(ToString::to_string)).unwrap();

    assert_eq!(config.spicedb_endpoint_ref(), "owner-spicedb");
    assert_eq!(config.spicedb_endpoint(), "http://spicedb:50051");
    assert_eq!(
        config.fixed_zed_token_secret_ref(),
        Some("reconciliation-revision")
    );
    assert_eq!(config.candidate_revision_root(), None);
    assert_eq!(config.trust_domain_id(), "raziel-test");
    let debug = format!("{config:?}");
    assert!(!debug.contains("spicedb-token"));
    assert!(!debug.contains("zed-token"));
    assert_eq!(debug.matches("<redacted>").count(), 1);
}

#[test]
fn snapshot_configuration_accepts_one_private_candidate_root() {
    let mut values = snapshot_values();
    values.remove("RAZIEL_ZED_TOKEN_SECRET_REF");
    values.remove("RAZIEL_ZED_TOKEN_PATH");
    values.insert(
        "RAZIEL_ZED_TOKEN_CANDIDATE_ROOT",
        "/var/lib/raziel/policy-candidates",
    );
    let config =
        SnapshotServiceConfig::from_lookup(|key| values.get(key).map(ToString::to_string)).unwrap();
    assert_eq!(config.fixed_zed_token_secret_ref(), None);
    assert_eq!(
        config.candidate_revision_root(),
        Some(Path::new("/var/lib/raziel/policy-candidates"))
    );
    assert!(config.accepts_zed_token_reference("candidate-1"));
    assert!(!config.accepts_zed_token_reference("../private"));
}

#[test]
fn snapshot_configuration_rejects_remote_tls_and_unsafe_paths() {
    let values = snapshot_values();
    let invalid_endpoint = SnapshotServiceConfig::from_lookup(|key| {
        if key == "RAZIEL_SPICEDB_ENDPOINT" {
            Some("https://remote.example:443".into())
        } else {
            values.get(key).map(ToString::to_string)
        }
    });
    assert_eq!(
        invalid_endpoint,
        Err(SnapshotServiceConfigError::InvalidSpiceDbEndpoint)
    );

    let relative_socket = SnapshotServiceConfig::from_lookup(|key| {
        if key == "RAZIEL_REGISTRY_WRITER_SOCKET_PATH" {
            Some("relative/writer.sock".into())
        } else {
            values.get(key).map(ToString::to_string)
        }
    });
    assert_eq!(
        relative_socket,
        Err(SnapshotServiceConfigError::InvalidRegistryWriterSocket)
    );

    let ambiguous_source = SnapshotServiceConfig::from_lookup(|key| {
        if key == "RAZIEL_ZED_TOKEN_CANDIDATE_ROOT" {
            Some("/var/lib/raziel/policy-candidates".into())
        } else {
            values.get(key).map(ToString::to_string)
        }
    });
    assert_eq!(
        ambiguous_source,
        Err(SnapshotServiceConfigError::InvalidZedTokenSource)
    );
}

fn snapshot_values() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("RAZIEL_SPICEDB_ENDPOINT_REF", "owner-spicedb"),
        ("RAZIEL_SPICEDB_ENDPOINT", "http://spicedb:50051"),
        (
            "RAZIEL_SPICEDB_AUTH_TOKEN_PATH",
            "/run/secrets/raziel/spicedb-token",
        ),
        ("RAZIEL_ZED_TOKEN_SECRET_REF", "reconciliation-revision"),
        ("RAZIEL_ZED_TOKEN_PATH", "/run/secrets/raziel/zed-token"),
        (
            "RAZIEL_PRIVATE_SNAPSHOT_ROOT",
            "/var/lib/raziel/private-snapshots",
        ),
        (
            "RAZIEL_REGISTRY_WRITER_SOCKET_PATH",
            "/run/raziel-registry/writer.sock",
        ),
        ("RAZIEL_REGISTRY_PATH", "/var/lib/raziel/registry"),
        (
            "RAZIEL_SNAPSHOT_KEY_PATH",
            "/run/secrets/raziel/snapshot-authority.pk8",
        ),
        ("RAZIEL_TRUST_DOMAIN_ID", "raziel-test"),
    ])
}
