use raziel_authorization_core::{
    AUTHORIZATION_GUEST_INPUT_VERSION, CapabilityRequestBindingV1, TRUSTED_POLICY_CEILINGS,
    TRUSTED_SNAPSHOT_LIMITS, agent_subject, capability_request_commitment,
};
use raziel_canonical::policy_hash;
use raziel_policy_ir::{
    AllowedSubjectType, ObjectDefinition, PermissionDefinition, PermissionExpr, PolicyProgram,
    RelationDefinition, ValidatedPolicyProgram,
};
use raziel_snapshot::{RelationshipRecord, SnapshotTree};
use raziel_witness::WitnessBuildError;

use super::{
    InputAssemblyError, SelectedPrivateSnapshot, TrustedAuthorizationSelection,
    assemble_from_trusted_selection, proof_window_end,
};
use crate::admission::LOCAL_PLONK_ADMISSION_WINDOW_SECONDS;

const ISSUED_AT: i64 = 1_700_000_000;
const VALID_UNTIL: i64 = ISSUED_AT + LOCAL_PLONK_ADMISSION_WINDOW_SECONDS + 300;
const SNAPSHOT_ID: &str = "snapshot-private-alpha";
const SNAPSHOT_SALT: [u8; 32] = [0x5a; 32];

#[test]
fn assembles_the_exact_positive_input_and_expected_journal() {
    let fixture = Fixture::allowed();
    let expected_commitment = capability_request_commitment(&fixture.request).unwrap();
    let expires_at = proof_window_end(&fixture.request, ISSUED_AT).unwrap();
    let assembled = assemble_from_trusted_selection(
        &fixture.request,
        &expected_commitment,
        fixture.trusted,
        fixture.selected,
        ISSUED_AT,
        expires_at,
    )
    .unwrap();

    assert_eq!(
        assembled.guest_input().expires_at_unix_seconds,
        ISSUED_AT + LOCAL_PLONK_ADMISSION_WINDOW_SECONDS
    );
    assert_eq!(
        assembled
            .guest_input()
            .relationship_witness_set
            .entries
            .iter()
            .map(|entry| entry.leaf_index)
            .collect::<Vec<_>>(),
        [0]
    );
    assert_eq!(
        assembled
            .expected_public_values()
            .capability_request_commitment,
        expected_commitment
    );
    assert_eq!(
        assembled
            .expected_public_values()
            .relationship_snapshot_root,
        assembled.guest_input().relationship_snapshot_root
    );
    assert!(!assembled.expected_public_values_bytes().is_empty());
    assert_eq!(
        format!("{assembled:?}"),
        "ValidatedAuthorizationInput { private_fields: \"<redacted>\" }"
    );
}

#[test]
fn snapshot_denial_and_validity_fail_closed() {
    let mut mismatched = Fixture::allowed();
    mismatched.selected.snapshot_id = "snapshot-private-beta".into();
    assert_eq!(
        mismatched.assemble().unwrap_err(),
        InputAssemblyError::SnapshotSelectionMismatch
    );

    let mut inconsistent_trust = Fixture::allowed();
    inconsistent_trust.trusted.snapshot_policy_hash[0] ^= 1;
    assert_eq!(
        inconsistent_trust.assemble().unwrap_err(),
        InputAssemblyError::TrustedSelectionMismatch
    );

    let denied = Fixture::denied();
    assert_eq!(
        denied.assemble().unwrap_err(),
        InputAssemblyError::Witness(WitnessBuildError::AuthorizationDenied)
    );

    let mut short_policy = Fixture::allowed();
    short_policy.trusted.policy_valid_until_unix_seconds =
        ISSUED_AT + LOCAL_PLONK_ADMISSION_WINDOW_SECONDS - 1;
    assert_eq!(
        short_policy.assemble().unwrap_err(),
        InputAssemblyError::InsufficientValidityWindow
    );

    let mut short_request = request();
    short_request.requested_expires_at_unix_seconds =
        ISSUED_AT + LOCAL_PLONK_ADMISSION_WINDOW_SECONDS - 1;
    assert_eq!(
        proof_window_end(&short_request, ISSUED_AT),
        Err(InputAssemblyError::InsufficientValidityWindow)
    );
    assert_eq!(
        proof_window_end(&request(), -1),
        Err(InputAssemblyError::InvalidWorkerStart)
    );
}

#[test]
fn private_values_do_not_enter_debug_or_error_output() {
    let mut fixture = Fixture::allowed();
    fixture.selected.snapshot_id = "private-mismatched-snapshot".into();
    let selected_debug = format!("{:?}", fixture.selected);
    let error = fixture.assemble().unwrap_err();

    for output in [selected_debug, format!("{error:?}"), error.to_string()] {
        assert!(!output.contains("private-mismatched-snapshot"));
        assert!(!output.contains("private-payments"));
        assert!(!output.contains("snapshot-private-alpha"));
    }
}

struct Fixture {
    request: CapabilityRequestBindingV1,
    trusted: TrustedAuthorizationSelection,
    selected: SelectedPrivateSnapshot,
}

impl Fixture {
    fn allowed() -> Self {
        Self::new(true)
    }

    fn denied() -> Self {
        Self::new(false)
    }

    fn new(allowed: bool) -> Self {
        let policy = policy();
        let request = request();
        let subject_id = if allowed {
            agent_subject(&request.agent_key_thumbprint).object_id
        } else {
            "private-denied-agent".into()
        };
        let relationships = vec![relationship(subject_id)];
        let root = SnapshotTree::build(
            &policy,
            relationships.clone(),
            &SNAPSHOT_SALT,
            TRUSTED_SNAPSHOT_LIMITS,
        )
        .unwrap()
        .root();
        let policy_hash = policy_hash(&policy).unwrap();
        Self {
            request,
            trusted: TrustedAuthorizationSelection {
                policy,
                policy_hash,
                policy_record_id: "policy-private-alpha".into(),
                policy_valid_until_unix_seconds: VALID_UNTIL,
                snapshot_id: SNAPSHOT_ID.into(),
                relationship_snapshot_root: root,
                snapshot_policy_record_id: "policy-private-alpha".into(),
                snapshot_policy_hash: policy_hash,
                snapshot_expires_at_unix_seconds: VALID_UNTIL,
            },
            selected: SelectedPrivateSnapshot::new(
                SNAPSHOT_ID.into(),
                relationships,
                SNAPSHOT_SALT,
            ),
        }
    }

    fn assemble(self) -> Result<super::ValidatedAuthorizationInput, InputAssemblyError> {
        let expected_commitment = capability_request_commitment(&self.request).unwrap();
        let expires_at = proof_window_end(&self.request, ISSUED_AT)?;
        assemble_from_trusted_selection(
            &self.request,
            &expected_commitment,
            self.trusted,
            self.selected,
            ISSUED_AT,
            expires_at,
        )
    }
}

fn request() -> CapabilityRequestBindingV1 {
    CapabilityRequestBindingV1 {
        format_version: AUTHORIZATION_GUEST_INPUT_VERSION,
        agent_key_thumbprint: [0x11; 32],
        task_id: "reconcile-private-payments".into(),
        purpose: "payment-reconciliation".into(),
        action: "access".into(),
        resource_type: "dataset".into(),
        resource_scope: "private-payments".into(),
        requested_expires_at_unix_seconds: VALID_UNTIL,
        issuance_nonce: [0x22; 32],
        snapshot_id: SNAPSHOT_ID.into(),
        request_context: vec![],
    }
}

fn relationship(subject_id: String) -> RelationshipRecord {
    RelationshipRecord {
        resource_type: "dataset".into(),
        resource_id: "private-payments".into(),
        relation: "viewer".into(),
        subject_type: "agent".into(),
        subject_id,
        subject_relation: None,
        stored_caveat: None,
    }
}

fn policy() -> ValidatedPolicyProgram {
    let limits = TRUSTED_POLICY_CEILINGS.policy_limits;
    ValidatedPolicyProgram::new(
        PolicyProgram {
            ir_version: 1,
            compiler_name: "razield-input-assembly-test".into(),
            compiler_version: "1".into(),
            source_schema_digest: [0x33; 32],
            definitions: vec![
                ObjectDefinition {
                    name: "agent".into(),
                    relations: vec![],
                },
                ObjectDefinition {
                    name: "dataset".into(),
                    relations: vec![RelationDefinition {
                        name: "viewer".into(),
                        allowed_subject_types: vec![AllowedSubjectType {
                            object_type: "agent".into(),
                            subject_relation: None,
                            caveat_name: None,
                        }],
                    }],
                },
            ],
            permissions: vec![PermissionDefinition {
                object_type: "dataset".into(),
                name: "access".into(),
                expression: PermissionExpr::RelationRef {
                    name: "viewer".into(),
                },
            }],
            caveats: vec![],
            limits,
        },
        &TRUSTED_POLICY_CEILINGS,
    )
    .unwrap()
}
