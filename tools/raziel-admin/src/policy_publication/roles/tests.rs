use super::super::catalogue::tests::{Fixture, NOW};
use super::*;
use raziel_policy_ir::{
    EvaluationRequest, ObjectRef, RelationshipResolver, ResolvedRelationship, ResolverError,
    evaluate,
};
use std::collections::BTreeMap;

fn request() -> AgentRolesRequest {
    AgentRolesRequest {
        format_version: 1,
        operation: RoleOperation::Add,
        policy_version_id: "b".repeat(32),
        approval_epoch: "1".into(),
        dataset_id: "processor-demo-v1".into(),
        agent_key_thumbprint: "d".repeat(64),
        role: Some(AgentRole::TeamMember),
        team_id: Some("finance-operations".into()),
    }
}

fn package(fixture: &Fixture) -> ApprovedPolicyPackage {
    PolicyPackageStore::new(
        fixture.config.package_root.clone(),
        fixture.config.registry_root.clone(),
        &fixture.config.trust_domain_id,
    )
    .unwrap()
    .read_approved(&PolicyPackageKey::new("b".repeat(32), 1).unwrap(), NOW)
    .unwrap()
}

#[test]
fn strict_role_requests_reject_unknown_fields_cross_namespace_and_invalid_team_values() {
    let request = request();
    assert_eq!(request.validate().unwrap(), 1);
    let mut json = serde_json::to_value(&request).unwrap();
    json["agentId"] = "untrusted".into();
    assert!(serde_json::from_value::<AgentRolesRequest>(json).is_err());
    for change in [
        |r: &mut AgentRolesRequest| r.policy_version_id = "../escape".into(),
        |r: &mut AgentRolesRequest| r.approval_epoch = "01".into(),
        |r: &mut AgentRolesRequest| r.agent_key_thumbprint = "D".repeat(64),
        |r: &mut AgentRolesRequest| r.team_id = Some("../finance".into()),
        |r: &mut AgentRolesRequest| r.dataset_id = "a".repeat(129),
        |r: &mut AgentRolesRequest| r.operation = RoleOperation::Enable,
        |r: &mut AgentRolesRequest| r.role = Some(AgentRole::Viewer),
    ] {
        let mut bad = request.clone();
        change(&mut bad);
        assert!(bad.validate().is_err());
    }
}

#[test]
fn team_remove_keeps_the_shared_dataset_link_and_other_agents() {
    let mut input = request();
    let added = role_updates(&input).unwrap();
    assert_eq!(added.len(), 2);
    let link = added[0].relationship.clone().unwrap();
    assert_eq!(
        link.resource.as_ref().unwrap().object_type,
        format!("dataset_{}", "b".repeat(32))
    );
    input.operation = RoleOperation::Remove;
    let removed = role_updates(&input).unwrap();
    assert_eq!(removed.len(), 1);
    assert_eq!(
        removed[0].operation,
        api::relationship_update::Operation::Delete as i32
    );
    assert_eq!(removed[0].relationship, added[1].relationship);
    assert_ne!(removed[0].relationship.as_ref(), Some(&link));
}

#[derive(Default)]
struct Rows(BTreeMap<(ObjectRef, String), Vec<ResolvedRelationship>>);
impl RelationshipResolver for Rows {
    fn resolve<'a>(
        &'a self,
        resource: &ObjectRef,
        relation: &str,
    ) -> Result<&'a [ResolvedRelationship], ResolverError> {
        Ok(self
            .0
            .get(&(resource.clone(), relation.into()))
            .map(Vec::as_slice)
            .unwrap_or_default())
    }
}
impl Rows {
    fn from(rows: &[api::Relationship]) -> Self {
        let mut result = Self::default();
        for (index, row) in rows.iter().enumerate() {
            let resource = row.resource.as_ref().unwrap();
            let subject = row.subject.as_ref().unwrap();
            let object = subject.object.as_ref().unwrap();
            result
                .0
                .entry((
                    ObjectRef {
                        object_type: resource.object_type.clone(),
                        object_id: resource.object_id.clone(),
                    },
                    row.relation.clone(),
                ))
                .or_default()
                .push(ResolvedRelationship {
                    subject: ObjectRef {
                        object_type: object.object_type.clone(),
                        object_id: object.object_id.clone(),
                    },
                    subject_relation: (!subject.optional_relation.is_empty())
                        .then(|| subject.optional_relation.clone()),
                    caveat: None,
                    witness_id: Some(index as u32),
                });
        }
        result
    }
}

#[test]
fn missing_add_remove_sequence_is_deny_allow_deny_in_the_real_policy_evaluator() {
    let mut fixture = Fixture::new();
    fixture.retain_rule('b', 1, 1, true);
    let package = package(&fixture);
    assert_eq!(
        available_roles(&package).unwrap(),
        vec![AgentRole::TeamMember]
    );
    let input = request();
    let request = EvaluationRequest {
        resource: ObjectRef {
            object_type: format!("dataset_{}", input.policy_version_id),
            object_id: input.dataset_id.clone(),
        },
        subject: ObjectRef {
            object_type: "agent".into(),
            object_id: input.agent_key_thumbprint.clone(),
        },
        permission: "query".into(),
        request_context: vec![],
        capability_claims: vec![],
    };
    let rows = role_updates(&input)
        .unwrap()
        .into_iter()
        .map(|u| u.relationship.unwrap())
        .collect::<Vec<_>>();
    assert!(!evaluate(package.policy(), &request, &Rows::default()).unwrap());
    assert!(evaluate(package.policy(), &request, &Rows::from(&rows)).unwrap());
    assert!(!evaluate(package.policy(), &request, &Rows::from(&rows[..1])).unwrap());
    let mut other = request.clone();
    other.subject.object_id = "e".repeat(64);
    assert!(!evaluate(package.policy(), &other, &Rows::from(&rows)).unwrap());
    let visible = projection(
        &input,
        true,
        false,
        vec![AgentRole::TeamMember],
        &Observed {
            token: "revision".into(),
            relationships: rows,
        },
    );
    assert_eq!(
        visible.assignments[0].team_id.as_deref(),
        Some("finance-operations")
    );
}

#[test]
fn mode_record_is_sticky_and_corruption_never_falls_back_to_automatic_grants() {
    let mut fixture = Fixture::new();
    fixture.retain('b', 1, 1);
    let package = package(&fixture);
    let root = &fixture.config.publication_root;
    assert!(!is_managed(root, &package).unwrap());
    enable(root, &package).unwrap();
    enable(root, &package).unwrap();
    assert!(is_managed(root, &package).unwrap());
    crate::io::atomic_write(
        &root
            .join("managed-roles")
            .join(format!("{}.json", "b".repeat(32))),
        b"{}",
    )
    .unwrap();
    assert!(is_managed(root, &package).is_err());
}

#[test]
fn role_options_are_derived_from_the_approved_policy_not_the_requested_role() {
    let mut fixture = Fixture::new();
    fixture.retain('b', 1, 1);
    let package = package(&fixture);
    assert_eq!(available_roles(&package).unwrap(), vec![AgentRole::Viewer]);
    let mut roles = BTreeSet::new();
    assert!(
        collect_roles(
            &PermissionExpr::CaveatedRelation {
                relation: "viewer".into(),
                caveat_name: "private".into()
            },
            &mut roles
        )
        .is_err()
    );
}
