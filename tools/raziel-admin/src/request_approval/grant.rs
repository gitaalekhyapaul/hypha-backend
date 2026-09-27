//! Pure materialization of the private visual-builder grant recipe.

use std::cmp::Ordering;

use raziel_authority::validate_agent_api_key;
use raziel_authorization_core::TRUSTED_SNAPSHOT_LIMITS;
use raziel_policy_ir::{
    CaveatInputSource, CaveatValue, CaveatValueType, PermissionExpr, ValidatedPolicyProgram,
};
use raziel_proto::raziel::v1 as proto;
use raziel_snapshot::{
    RelationshipRecord, SnapshotTree, StoredCaveat, StoredContextEntry,
    compare_relationship_identity,
};

use crate::policy_package::ApprovedPolicyPackage;

use super::{ApprovalError, recipe, validate_command, validation};

/// Materializes all supported rule leaves, including every ANY and ALL branch.
/// This performs no I/O and makes no ALLOW claim. Snapshot publication must recheck
/// current approval and enforce the aggregate bounds under the owner writer lock.
pub fn build_grant(
    package: &ApprovedPolicyPackage,
    original: &proto::CreateAccessRequestRequest,
    grant: &proto::AccessGrantInputs,
) -> Result<Vec<RelationshipRecord>, ApprovalError> {
    let owner = original.owner.as_ref().ok_or(ApprovalError::Invalid)?;
    let intent = original.intent.as_ref().ok_or(ApprovalError::Invalid)?;
    // Reuse the exact owner input contract without creating an approval transition.
    let command = proto::ApproveAccessRequestRequest {
        agent_id: owner.agent_id.clone(),
        request_id: intent.request_id.clone(),
        expected_generation: 1,
        policy_record_id: package.approval_record().record_id.clone(),
        policy_version_id: package.identity().version_id().into(),
        approval_epoch: package.key().approval_epoch(),
        dataset: original.dataset.clone(),
        grant: Some(grant.clone()),
    };
    validate_command(&command)?;
    validation::match_original(&command, original)?;
    recipe::validate_policy(package)?;
    let thumbprint =
        validate_agent_api_key(&owner.agent_public_key).map_err(|_| ApprovalError::Invalid)?;
    materialize(
        package.policy(),
        package.identity().version_id(),
        &intent.dataset_id,
        &crate::io::lowercase_hex(&thumbprint),
        grant,
    )
}

fn materialize(
    policy: &ValidatedPolicyProgram,
    version: &str,
    dataset: &str,
    agent: &str,
    grant: &proto::AccessGrantInputs,
) -> Result<Vec<RelationshipRecord>, ApprovalError> {
    let resource_type = format!("dataset_{version}");
    let permission = policy
        .program()
        .permissions
        .iter()
        .find(|p| p.object_type == resource_type && p.name == "query")
        .ok_or(ApprovalError::Invalid)?;
    let mut builder = GrantBuilder {
        policy,
        resource_type,
        dataset,
        agent,
        grant,
        team_type: format!("team_{version}"),
        organization_type: format!("organization_{version}"),
        records: Vec::new(),
    };
    builder.visit(&permission.expression)?;
    finish(policy, builder.records)
}

struct GrantBuilder<'a> {
    policy: &'a ValidatedPolicyProgram,
    resource_type: String,
    team_type: String,
    organization_type: String,
    dataset: &'a str,
    agent: &'a str,
    grant: &'a proto::AccessGrantInputs,
    records: Vec<RelationshipRecord>,
}

impl GrantBuilder<'_> {
    fn visit(&mut self, expression: &PermissionExpr) -> Result<(), ApprovalError> {
        match expression {
            PermissionExpr::AnyOf(children) | PermissionExpr::AllOf(children) => {
                for child in children {
                    self.visit(child)?;
                }
            }
            PermissionExpr::RelationRef { name } => {
                self.records
                    .push(self.root_record(name, "agent", self.agent, None, None));
            }
            PermissionExpr::CaveatedRelation {
                relation,
                caveat_name,
            } => {
                let caveat = self.stored_caveat(caveat_name)?;
                self.records.push(self.root_record(
                    relation,
                    "agent",
                    self.agent,
                    None,
                    Some(caveat),
                ));
            }
            PermissionExpr::SubjectRelationRef {
                relation,
                subject_relation,
            } if relation == "team_viewer" && subject_relation == "member" => {
                self.records.push(self.root_record(
                    relation,
                    &self.team_type,
                    &self.grant.team_id,
                    Some("member"),
                    None,
                ));
                self.records
                    .push(self.membership(&self.team_type, &self.grant.team_id, "member"));
            }
            PermissionExpr::Arrow {
                source_relation,
                target_permission,
            } if source_relation == "organization" && target_permission == "audit" => {
                self.records.push(self.root_record(
                    source_relation,
                    &self.organization_type,
                    &self.grant.organization_id,
                    None,
                    None,
                ));
                self.records.push(self.membership(
                    &self.organization_type,
                    &self.grant.organization_id,
                    "auditor",
                ));
            }
            _ => return Err(ApprovalError::Invalid),
        }
        if self.records.len() > TRUSTED_SNAPSHOT_LIMITS.max_relationships as usize {
            return Err(ApprovalError::Invalid);
        }
        Ok(())
    }

    fn root_record(
        &self,
        relation: &str,
        subject_type: &str,
        subject_id: &str,
        subject_relation: Option<&str>,
        stored_caveat: Option<StoredCaveat>,
    ) -> RelationshipRecord {
        RelationshipRecord {
            resource_type: self.resource_type.clone(),
            resource_id: self.dataset.into(),
            relation: relation.into(),
            subject_type: subject_type.into(),
            subject_id: subject_id.into(),
            subject_relation: subject_relation.map(Into::into),
            stored_caveat,
        }
    }

    fn membership(
        &self,
        resource_type: &str,
        resource_id: &str,
        relation: &str,
    ) -> RelationshipRecord {
        RelationshipRecord {
            resource_type: resource_type.into(),
            resource_id: resource_id.into(),
            relation: relation.into(),
            subject_type: "agent".into(),
            subject_id: self.agent.into(),
            subject_relation: None,
            stored_caveat: None,
        }
    }

    fn stored_caveat(&self, name: &str) -> Result<StoredCaveat, ApprovalError> {
        let definition = self
            .policy
            .program()
            .caveats
            .iter()
            .find(|c| c.name == name)
            .ok_or(ApprovalError::Invalid)?;
        let context = definition
            .parameters
            .iter()
            .filter(|parameter| parameter.source == CaveatInputSource::RelationshipContext)
            .map(|parameter| {
                let value = match (parameter.name.as_str(), parameter.value_type) {
                    ("approved_amount", CaveatValueType::Int) => {
                        CaveatValue::Int(self.grant.approved_amount)
                    }
                    ("is_approved", CaveatValueType::Bool) => {
                        CaveatValue::Bool(self.grant.is_approved)
                    }
                    ("is_suspended", CaveatValueType::Bool) => {
                        CaveatValue::Bool(self.grant.is_suspended)
                    }
                    ("is_override", CaveatValueType::Bool) => {
                        CaveatValue::Bool(self.grant.is_override)
                    }
                    _ => return Err(ApprovalError::Invalid),
                };
                Ok(StoredContextEntry {
                    name: parameter.name.clone(),
                    value,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(StoredCaveat {
            name: name.into(),
            context,
        })
    }
}

fn finish(
    policy: &ValidatedPolicyProgram,
    mut records: Vec<RelationshipRecord>,
) -> Result<Vec<RelationshipRecord>, ApprovalError> {
    if records.is_empty() || records.len() > TRUSTED_SNAPSHOT_LIMITS.max_relationships as usize {
        return Err(ApprovalError::Invalid);
    }
    records.sort_by(compare_relationship_identity);
    let mut unique: Vec<RelationshipRecord> = Vec::with_capacity(records.len());
    for record in records {
        if let Some(previous) = unique.last()
            && compare_relationship_identity(previous, &record) == Ordering::Equal
        {
            if *previous != record {
                return Err(ApprovalError::Conflict);
            }
            continue;
        }
        unique.push(record);
    }
    // The public snapshot validator is tree-based. Discard this validation-only
    // tree and root. Real snapshots still require a fresh private salt and exact
    // revision extraction; witness/private-input limits remain unchanged there.
    SnapshotTree::build(policy, unique.clone(), &[0; 32], TRUSTED_SNAPSHOT_LIMITS)
        .map_err(|_| ApprovalError::Invalid)?;
    Ok(unique)
}

#[cfg(test)]
#[path = "grant_tests.rs"]
mod tests;
