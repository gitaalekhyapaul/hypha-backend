use std::collections::BTreeMap;

use raziel_policy_ir::{
    ContextEntry, EvaluationRequest, ObjectRef, RelationshipCaveat, RelationshipResolver,
    ResolvedRelationship, ResolverError, ValidatedPolicyProgram, evaluate_with_usage,
};
use raziel_snapshot::{
    Digest, RELATIONSHIP_WITNESS_SET_VERSION, RelationshipRecord, RelationshipWitnessSetV1,
    SnapshotError, SnapshotLimit, SnapshotLimits, SnapshotTree, verify_witness_set,
};

use crate::WitnessBuildError;

/// Builds the minimum verified witness for the evaluator's first positive path.
///
/// The relationship vector must be the exact, strictly ordered contents of the
/// selected snapshot. Ownership is consumed so the private records can be dropped
/// with the temporary tree and resolver at the end of this call.
pub fn build_positive_witness(
    policy: &ValidatedPolicyProgram,
    request: &EvaluationRequest,
    relationships: Vec<RelationshipRecord>,
    snapshot_secret_salt: Digest,
    expected_root: Digest,
    snapshot_limits: SnapshotLimits,
) -> Result<RelationshipWitnessSetV1, WitnessBuildError> {
    let leaf_count = u32::try_from(relationships.len()).map_err(|_| {
        WitnessBuildError::InvalidSnapshot(SnapshotError::LimitExceeded(
            SnapshotLimit::Relationships,
        ))
    })?;
    if leaf_count > snapshot_limits.max_relationships {
        return Err(WitnessBuildError::InvalidSnapshot(
            SnapshotError::LimitExceeded(SnapshotLimit::Relationships),
        ));
    }
    let resolver = SnapshotResolver::from_relationships(&relationships)?;
    let tree = SnapshotTree::build(
        policy,
        relationships,
        &snapshot_secret_salt,
        snapshot_limits,
    )
    .map_err(WitnessBuildError::InvalidSnapshot)?;

    if tree.root() != expected_root {
        return Err(WitnessBuildError::SnapshotRootMismatch);
    }

    let outcome = evaluate_with_usage(policy, request, &resolver)
        .map_err(WitnessBuildError::EvaluationFailed)?;
    if !outcome.allowed {
        return Err(WitnessBuildError::AuthorizationDenied);
    }
    if outcome.used_witness_ids.is_empty()
        || outcome
            .used_witness_ids
            .windows(2)
            .any(|ids| ids[0] >= ids[1])
    {
        return Err(WitnessBuildError::InvalidEvaluationTrace);
    }

    let entries = outcome
        .used_witness_ids
        .into_iter()
        .map(|leaf_index| {
            tree.opening(leaf_index)
                .map_err(|_| WitnessBuildError::InvalidEvaluationTrace)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let witness = RelationshipWitnessSetV1 {
        format_version: RELATIONSHIP_WITNESS_SET_VERSION,
        snapshot_secret_salt,
        leaf_count,
        entries,
    };
    verify_witness_set(policy, &witness, &expected_root, snapshot_limits)
        .map_err(WitnessBuildError::InvalidConstructedWitness)?;
    Ok(witness)
}

#[derive(Default)]
struct SnapshotResolver {
    relationships: BTreeMap<(ObjectRef, String), Vec<ResolvedRelationship>>,
}

impl SnapshotResolver {
    fn from_relationships(relationships: &[RelationshipRecord]) -> Result<Self, WitnessBuildError> {
        let mut resolver = Self::default();
        for (leaf_index, relationship) in relationships.iter().enumerate() {
            let witness_id = u32::try_from(leaf_index).map_err(|_| {
                WitnessBuildError::InvalidSnapshot(SnapshotError::LimitExceeded(
                    SnapshotLimit::Relationships,
                ))
            })?;
            let resolved = ResolvedRelationship {
                subject: ObjectRef {
                    object_type: relationship.subject_type.clone(),
                    object_id: relationship.subject_id.clone(),
                },
                subject_relation: relationship.subject_relation.clone(),
                caveat: relationship
                    .stored_caveat
                    .as_ref()
                    .map(|caveat| RelationshipCaveat {
                        name: caveat.name.clone(),
                        context: caveat
                            .context
                            .iter()
                            .map(|entry| ContextEntry {
                                name: entry.name.clone(),
                                value: entry.value.clone(),
                            })
                            .collect(),
                    }),
                witness_id: Some(witness_id),
            };
            resolver
                .relationships
                .entry((
                    ObjectRef {
                        object_type: relationship.resource_type.clone(),
                        object_id: relationship.resource_id.clone(),
                    },
                    relationship.relation.clone(),
                ))
                .or_default()
                .push(resolved);
        }
        Ok(resolver)
    }
}

impl RelationshipResolver for SnapshotResolver {
    fn resolve<'a>(
        &'a self,
        resource: &ObjectRef,
        relation: &str,
    ) -> Result<&'a [ResolvedRelationship], ResolverError> {
        Ok(self
            .relationships
            .get(&(resource.clone(), relation.to_owned()))
            .map_or(&[], Vec::as_slice))
    }
}
