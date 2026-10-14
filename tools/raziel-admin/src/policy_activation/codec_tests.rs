use super::*;
use crate::policy_activation::wire::PolicyActivationProjectionWire;

fn entry() -> PolicyCatalogueEntryWire {
    let identity = PolicyVersionIdentity::new("a".repeat(32), "b".repeat(32), 1).unwrap();
    PolicyCatalogueEntryWire {
        family_id: identity.family_id().into(),
        version_id: identity.version_id().into(),
        version: 1,
        record_id: "record-one".into(),
        policy_hash: "c".repeat(64),
        policy_name: identity.signed_policy_name(),
        policy_version: "1".into(),
        resource_type: identity.resource_type(),
        permission: "query".into(),
        approval_epoch: 1,
        valid_from_unix_seconds: 1_800_000_000,
        valid_until_unix_seconds: 1_800_005_000,
        required_grant_fields: vec![],
    }
}

#[test]
fn catalogue_field_metadata_survives_framed_owner_transport_and_json_projection() {
    for fields in [
        vec![],
        vec![1],
        vec![2],
        vec![3, 4, 5, 6],
        vec![1, 2, 3, 4, 5, 6],
    ] {
        let mut wire = response();
        wire.catalogue.as_mut().unwrap().policies[0].required_grant_fields = fields.clone();
        let (mut sender, mut receiver) = UnixStream::pair().unwrap();
        write_response(&mut sender, &wire).unwrap();
        let decoded = read_message::<PolicyActivationResponseWire>(&mut receiver).unwrap();
        assert_eq!(decoded, wire);
        let projection = catalogue_response_result(decoded).unwrap();
        let expected: Vec<_> = fields
            .iter()
            .map(|field| GrantInputField::try_from(*field).unwrap())
            .collect();
        assert_eq!(projection.policies[0].required_grant_fields, expected);
        assert_eq!(
            serde_json::to_value(&projection).unwrap()["policies"][0]["requiredGrantFields"],
            serde_json::to_value(expected).unwrap()
        );
        assert_eq!(catalogue_wire(projection).unwrap(), wire.catalogue.unwrap());
    }
}

#[test]
fn catalogue_field_decoder_rejects_unknown_enums_duplicates_and_oversized_lists() {
    for fields in [
        vec![0],
        vec![-1],
        vec![7],
        vec![i32::MAX],
        vec![1, 1],
        vec![4, 4],
        vec![1, 2, 3, 4, 5, 6, 1],
    ] {
        let mut wire = response();
        wire.catalogue.as_mut().unwrap().policies[0].required_grant_fields = fields;
        let decoded =
            PolicyActivationResponseWire::decode(wire.encode_to_vec().as_slice()).unwrap();
        assert_eq!(
            catalogue_response_result(decoded),
            Err(PolicyActivationClientError::Protocol)
        );
    }
}

#[test]
fn catalogue_field_encoder_rejects_duplicate_and_oversized_typed_lists() {
    for count in [2, 7] {
        let mut projection = catalogue_response_result(response()).unwrap();
        projection.policies[0].required_grant_fields = vec![GrantInputField::TeamId; count];
        assert_eq!(
            catalogue_wire(projection),
            Err(PolicyActivationClientError::Protocol)
        );
    }
}

fn response() -> PolicyActivationResponseWire {
    PolicyActivationResponseWire {
        format_version: 1,
        failure: 0,
        projection: None,
        catalogue: Some(PolicyCatalogueWire {
            format_version: 2,
            policies: vec![entry()],
        }),
    }
}

#[test]
fn catalogue_response_roundtrips_and_does_not_become_activation_success() {
    let wire = response();
    let value = catalogue_response_result(wire.clone()).unwrap();
    assert_eq!(
        catalogue_wire(value).unwrap(),
        wire.catalogue.clone().unwrap()
    );
    assert_eq!(
        response_result(wire),
        Err(PolicyActivationClientError::Protocol)
    );
    let mut ambiguous = response();
    ambiguous.projection = Some(PolicyActivationProjectionWire::default());
    assert_eq!(
        catalogue_response_result(ambiguous.clone()),
        Err(PolicyActivationClientError::Protocol)
    );
    ambiguous.failure = PolicyActivationFailureWire::Unavailable as i32;
    assert_eq!(
        catalogue_response_result(ambiguous),
        Err(PolicyActivationClientError::Protocol)
    );
    let mut failure = response();
    failure.catalogue = None;
    failure.failure = PolicyActivationFailureWire::Unavailable as i32;
    assert_eq!(
        catalogue_response_result(failure),
        Err(PolicyActivationClientError::Unavailable)
    );
}

#[test]
fn catalogue_client_rejects_malformed_fields_and_bounds() {
    let mutations: &[fn(&mut PolicyCatalogueEntryWire)] = &[
        |p| p.family_id = "A".repeat(32),
        |p| p.version_id.clear(),
        |p| p.version = 0,
        |p| p.record_id.clear(),
        |p| p.record_id = "x".repeat(513),
        |p| p.record_id = "private\npath".into(),
        |p| p.policy_hash = "G".repeat(64),
        |p| p.policy_name = "unbound-name".into(),
        |p| p.policy_version = "01".into(),
        |p| p.resource_type = "dataset".into(),
        |p| p.permission = "write".into(),
        |p| p.approval_epoch = 0,
        |p| p.valid_from_unix_seconds = -62_135_596_801,
        |p| p.valid_until_unix_seconds = 253_402_300_800,
        |p| p.valid_until_unix_seconds = p.valid_from_unix_seconds,
    ];
    for mutate in mutations {
        let mut wire = response();
        mutate(&mut wire.catalogue.as_mut().unwrap().policies[0]);
        assert_eq!(
            catalogue_response_result(wire),
            Err(PolicyActivationClientError::Protocol)
        );
    }
    for count in [2, 33] {
        let mut wire = response();
        wire.catalogue.as_mut().unwrap().policies = vec![entry(); count];
        assert_eq!(
            catalogue_response_result(wire),
            Err(PolicyActivationClientError::Protocol)
        );
    }
    let mut wire = response();
    wire.catalogue.as_mut().unwrap().format_version = 3;
    assert_eq!(
        catalogue_response_result(wire),
        Err(PolicyActivationClientError::Protocol)
    );
}

#[test]
fn pre_repair_catalogue_without_field_metadata_is_rejected() {
    let mut old = response();
    old.catalogue.as_mut().unwrap().format_version = 1;
    // Empty repeated fields have no encoded tag, matching a pre-repair daemon.
    let (mut sender, mut receiver) = UnixStream::pair().unwrap();
    write_response(&mut sender, &old).unwrap();
    let decoded = read_message::<PolicyActivationResponseWire>(&mut receiver).unwrap();
    assert_eq!(
        catalogue_response_result(decoded),
        Err(PolicyActivationClientError::Protocol)
    );
    let mut invalid_projection = catalogue_response_result(response()).unwrap();
    invalid_projection.format_version = 2;
    assert_eq!(
        catalogue_wire(invalid_projection),
        Err(PolicyActivationClientError::Protocol)
    );
}

#[test]
fn frames_keep_the_byte_ceiling_and_default_activation_encoding() {
    let request = PolicyActivationRequestWire {
        format_version: 1,
        candidate_id: "a".into(),
        approved_by: "b".into(),
        operation: 0,
    };
    assert_eq!(request.encode_to_vec(), [8, 1, 18, 1, b'a', 26, 1, b'b']);
    let (mut sender, mut receiver) = UnixStream::pair().unwrap();
    sender
        .write_all(&((MAX_FRAME_BYTES + 1) as u32).to_be_bytes())
        .unwrap();
    assert!(read_message::<PolicyActivationResponseWire>(&mut receiver).is_err());
    let mut oversized = response();
    oversized.catalogue.as_mut().unwrap().policies[0].record_id = "x".repeat(MAX_FRAME_BYTES);
    assert!(write_response(&mut sender, &oversized).is_err());
}
