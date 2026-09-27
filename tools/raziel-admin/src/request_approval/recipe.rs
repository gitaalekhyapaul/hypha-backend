//! Checks that private grant inputs can represent this approved policy's graph.
//! It never evaluates the policy or decides whether the eventual request is allowed.

use std::collections::BTreeSet;

use raziel_policy_ir::{
    AllowedSubjectType, CaveatInputSource, CaveatValueType, ObjectDefinition, PermissionExpr,
    PolicyProgram,
};

use crate::policy_package::ApprovedPolicyPackage;

use super::ApprovalError;

pub(super) fn validate_policy(package: &ApprovedPolicyPackage) -> Result<(), ApprovalError> {
    validate_program(package.policy().program(), package.identity().version_id())
}

fn validate_program(policy: &PolicyProgram, version: &str) -> Result<(), ApprovalError> {
    let resource = format!("dataset_{version}");
    let team = format!("team_{version}");
    let organization = format!("organization_{version}");
    let root = policy
        .definitions
        .iter()
        .find(|d| d.name == resource)
        .ok_or(ApprovalError::Invalid)?;
    let permission = policy
        .permissions
        .iter()
        .find(|p| p.object_type == resource && p.name == "query")
        .ok_or(ApprovalError::Invalid)?;
    for definition in &policy.definitions {
        if definition.name == "agent" && definition.relations.is_empty()
            || definition.name == resource
        {
            continue;
        }
        let relation_name = if definition.name == team {
            "member"
        } else if definition.name == organization {
            "auditor"
        } else {
            return Err(ApprovalError::Invalid);
        };
        if definition.relations.len() != 1
            || definition.relations[0].name != relation_name
            || definition.relations[0].allowed_subject_types != vec![direct_subject(None)]
        {
            return Err(ApprovalError::Invalid);
        }
    }
    for p in &policy.permissions {
        if p.object_type == resource && p.name == "query" {
            continue;
        }
        if p.object_type != organization
            || p.name != "audit"
            || p.expression
                != (PermissionExpr::RelationRef {
                    name: "auditor".into(),
                })
        {
            return Err(ApprovalError::Invalid);
        }
    }
    let mut visited = BTreeSet::new();
    check_expression(
        &permission.expression,
        root,
        &team,
        &organization,
        &mut visited,
    )?;
    if visited.len() != root.relations.len() {
        return Err(ApprovalError::Invalid);
    }
    for caveat in &policy.caveats {
        for parameter in &caveat.parameters {
            if parameter.source == CaveatInputSource::RelationshipContext {
                let expected = match parameter.name.as_str() {
                    "approved_amount" => CaveatValueType::Int,
                    "is_approved" | "is_suspended" | "is_override" => CaveatValueType::Bool,
                    _ => return Err(ApprovalError::Invalid),
                };
                if parameter.value_type != expected {
                    return Err(ApprovalError::Invalid);
                }
            }
        }
    }
    Ok(())
}

fn check_expression(
    expression: &PermissionExpr,
    root: &ObjectDefinition,
    team: &str,
    organization: &str,
    visited: &mut BTreeSet<String>,
) -> Result<(), ApprovalError> {
    let (name, expected) = match expression {
        PermissionExpr::AnyOf(children) | PermissionExpr::AllOf(children) => {
            for child in children {
                check_expression(child, root, team, organization, visited)?;
            }
            return Ok(());
        }
        PermissionExpr::RelationRef { name } => (name, direct_subject(None)),
        PermissionExpr::CaveatedRelation {
            relation,
            caveat_name,
        } => (relation, direct_subject(Some(caveat_name.clone()))),
        PermissionExpr::SubjectRelationRef {
            relation,
            subject_relation,
        } if relation == "team_viewer" && subject_relation == "member" => (
            relation,
            AllowedSubjectType {
                object_type: team.into(),
                subject_relation: Some("member".into()),
                caveat_name: None,
            },
        ),
        PermissionExpr::Arrow {
            source_relation,
            target_permission,
        } if source_relation == "organization" && target_permission == "audit" => (
            source_relation,
            AllowedSubjectType {
                object_type: organization.into(),
                subject_relation: None,
                caveat_name: None,
            },
        ),
        _ => return Err(ApprovalError::Invalid),
    };
    if expected.object_type == "agent"
        && !matches!(name.as_str(), "approver" | "auditor" | "owner" | "viewer")
    {
        return Err(ApprovalError::Invalid);
    }
    let relation = root
        .relations
        .iter()
        .find(|r| r.name == *name)
        .ok_or(ApprovalError::Invalid)?;
    if relation.allowed_subject_types != vec![expected] || !visited.insert(name.clone()) {
        return Err(ApprovalError::Invalid);
    }
    Ok(())
}

fn direct_subject(caveat_name: Option<String>) -> AllowedSubjectType {
    AllowedSubjectType {
        object_type: "agent".into(),
        subject_relation: None,
        caveat_name,
    }
}

#[cfg(test)]
#[path = "recipe_tests.rs"]
mod tests;
