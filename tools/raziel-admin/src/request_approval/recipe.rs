//! Checks that private grant inputs can represent this approved policy's graph.
//! It never evaluates the policy or decides whether the eventual request is allowed.

use std::collections::BTreeSet;

use raziel_policy_ir::{
    AllowedSubjectType, CaveatInputSource, CaveatValueType, ObjectDefinition, PermissionExpr,
    PolicyProgram, ValidatedPolicyProgram,
};
use serde::Serialize;

use crate::policy_package::ApprovedPolicyPackage;

use super::ApprovalError;

/// Operator-only input names, never policy structure or stored context values.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq, PartialOrd, Ord, prost::Enumeration)]
#[serde(rename_all = "camelCase")]
#[repr(i32)]
pub enum GrantInputField {
    TeamId = 1,
    OrganizationId = 2,
    ApprovedAmount = 3,
    IsApproved = 4,
    IsSuspended = 5,
    IsOverride = 6,
}

pub(crate) fn required_grant_fields(
    policy: &ValidatedPolicyProgram,
    version: &str,
) -> Result<Vec<GrantInputField>, ApprovalError> {
    validate_program(policy.program(), version)
}

pub(super) fn validate_policy(package: &ApprovedPolicyPackage) -> Result<(), ApprovalError> {
    required_grant_fields(package.policy(), package.identity().version_id()).map(|_| ())
}

fn validate_program(
    policy: &PolicyProgram,
    version: &str,
) -> Result<Vec<GrantInputField>, ApprovalError> {
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
    let mut fields = BTreeSet::new();
    let mut used_caveats = BTreeSet::new();
    check_expression(
        &permission.expression,
        root,
        &team,
        &organization,
        &mut visited,
        &mut fields,
        &mut used_caveats,
    )?;
    if visited.len() != root.relations.len() {
        return Err(ApprovalError::Invalid);
    }
    for caveat in &policy.caveats {
        for parameter in &caveat.parameters {
            if parameter.source == CaveatInputSource::RelationshipContext {
                let (field, expected) = match parameter.name.as_str() {
                    "approved_amount" => (GrantInputField::ApprovedAmount, CaveatValueType::Int),
                    "is_approved" => (GrantInputField::IsApproved, CaveatValueType::Bool),
                    "is_suspended" => (GrantInputField::IsSuspended, CaveatValueType::Bool),
                    "is_override" => (GrantInputField::IsOverride, CaveatValueType::Bool),
                    _ => return Err(ApprovalError::Invalid),
                };
                if parameter.value_type != expected {
                    return Err(ApprovalError::Invalid);
                }
                if used_caveats.contains(&caveat.name) {
                    fields.insert(field);
                }
            }
        }
    }
    if used_caveats
        .iter()
        .any(|name| !policy.caveats.iter().any(|caveat| caveat.name == *name))
    {
        return Err(ApprovalError::Invalid);
    }
    Ok(fields.into_iter().collect())
}

fn check_expression(
    expression: &PermissionExpr,
    root: &ObjectDefinition,
    team: &str,
    organization: &str,
    visited: &mut BTreeSet<String>,
    fields: &mut BTreeSet<GrantInputField>,
    used_caveats: &mut BTreeSet<String>,
) -> Result<(), ApprovalError> {
    let (name, expected) = match expression {
        PermissionExpr::AnyOf(children) | PermissionExpr::AllOf(children) => {
            for child in children {
                check_expression(
                    child,
                    root,
                    team,
                    organization,
                    visited,
                    fields,
                    used_caveats,
                )?;
            }
            return Ok(());
        }
        PermissionExpr::RelationRef { name } => (name, direct_subject(None)),
        PermissionExpr::CaveatedRelation {
            relation,
            caveat_name,
        } => {
            used_caveats.insert(caveat_name.clone());
            (relation, direct_subject(Some(caveat_name.clone())))
        }
        PermissionExpr::SubjectRelationRef {
            relation,
            subject_relation,
        } if relation == "team_viewer" && subject_relation == "member" => {
            fields.insert(GrantInputField::TeamId);
            (
                relation,
                AllowedSubjectType {
                    object_type: team.into(),
                    subject_relation: Some("member".into()),
                    caveat_name: None,
                },
            )
        }
        PermissionExpr::Arrow {
            source_relation,
            target_permission,
        } if source_relation == "organization" && target_permission == "audit" => {
            fields.insert(GrantInputField::OrganizationId);
            (
                source_relation,
                AllowedSubjectType {
                    object_type: organization.into(),
                    subject_relation: None,
                    caveat_name: None,
                },
            )
        }
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
