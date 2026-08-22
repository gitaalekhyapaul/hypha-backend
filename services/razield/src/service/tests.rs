use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use super::clock::system_time_to_unix_seconds;
use super::{
    ClockError, MAX_SERVICE_MESSAGE_BYTES, ORPHAN_GRACE_PERIOD, ServiceConfig, ServiceConfigError,
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
