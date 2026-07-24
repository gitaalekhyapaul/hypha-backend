use std::cmp::Ordering;

use crate::validation::{
    find_caveat, find_definition, find_permission, find_relation, resolve_arrow_target,
    resolve_subject_set_target,
};
use crate::{
    CaveatExpr, CaveatInputSource, CaveatValue, ContextEntry, EvaluationError, EvaluationOutcome,
    EvaluationRequest, LimitKind, ObjectRef, PermissionDefinition, PermissionExpr, PolicyLimits,
    PolicyProgram, RelationDefinition, ResolvedRelationship, ResolverError, ValidatedPolicyProgram,
};

pub trait RelationshipResolver {
    fn resolve<'a>(
        &'a self,
        resource: &ObjectRef,
        relation: &str,
    ) -> Result<&'a [ResolvedRelationship], ResolverError>;
}

pub fn evaluate<R: RelationshipResolver>(
    program: &ValidatedPolicyProgram,
    request: &EvaluationRequest,
    resolver: &R,
) -> Result<bool, EvaluationError> {
    evaluate_with_usage(program, request, resolver).map(|outcome| outcome.allowed)
}

pub fn evaluate_with_usage<R: RelationshipResolver>(
    program: &ValidatedPolicyProgram,
    request: &EvaluationRequest,
    resolver: &R,
) -> Result<EvaluationOutcome, EvaluationError> {
    let program = program.program();
    validate_request(request, program.limits)?;
    let permission = find_permission(program, &request.resource.object_type, &request.permission)
        .ok_or(EvaluationError::MissingPermission)?;
    let mut budget = EvaluationBudget::new(program.limits);
    budget.consume_context_entries(
        request
            .request_context
            .len()
            .checked_add(request.capability_claims.len())
            .ok_or(EvaluationError::BudgetExceeded(LimitKind::ContextEntries))?,
    )?;
    let outcome = evaluate_permission(
        program,
        request,
        resolver,
        &mut budget,
        &request.resource,
        permission,
        0,
    )?;
    Ok(EvaluationOutcome {
        allowed: outcome.allowed,
        used_witness_ids: outcome.used_witness_ids,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TraceOutcome {
    allowed: bool,
    used_witness_ids: Vec<u32>,
}

impl TraceOutcome {
    const fn denied() -> Self {
        Self {
            allowed: false,
            used_witness_ids: Vec::new(),
        }
    }

    fn allowed(witness_id: Option<u32>) -> Self {
        Self {
            allowed: true,
            used_witness_ids: witness_id.into_iter().collect(),
        }
    }
}

fn evaluate_permission<R: RelationshipResolver>(
    program: &PolicyProgram,
    request: &EvaluationRequest,
    resolver: &R,
    budget: &mut EvaluationBudget,
    resource: &ObjectRef,
    permission: &PermissionDefinition,
    path_depth: u32,
) -> Result<TraceOutcome, EvaluationError> {
    evaluate_permission_expr(
        program,
        request,
        resolver,
        budget,
        resource,
        &permission.expression,
        path_depth,
    )
}

fn evaluate_permission_expr<R: RelationshipResolver>(
    program: &PolicyProgram,
    request: &EvaluationRequest,
    resolver: &R,
    budget: &mut EvaluationBudget,
    resource: &ObjectRef,
    expression: &PermissionExpr,
    path_depth: u32,
) -> Result<TraceOutcome, EvaluationError> {
    budget.consume_expression_node()?;
    let definition =
        find_definition(program, &resource.object_type).ok_or(EvaluationError::InvalidRequest)?;
    match expression {
        PermissionExpr::RelationRef { name } => {
            let relation =
                find_relation(definition, name).ok_or(EvaluationError::InvalidRequest)?;
            let relationships = resolve_relationships(
                resolver,
                budget,
                resource,
                name,
                relation,
                path_depth.saturating_add(1),
            )?;
            Ok(relationships
                .iter()
                .find(|relationship| {
                    relationship.subject == request.subject
                        && relationship.subject_relation.is_none()
                        && relationship.caveat.is_none()
                })
                .map_or_else(TraceOutcome::denied, |relationship| {
                    TraceOutcome::allowed(relationship.witness_id)
                }))
        }
        PermissionExpr::SubjectRelationRef {
            relation,
            subject_relation,
        } => {
            let source =
                find_relation(definition, relation).ok_or(EvaluationError::InvalidRequest)?;
            let target_type = resolve_subject_set_target(source, subject_relation, program)
                .map_err(|_| EvaluationError::InvalidRequest)?;
            let relationships = resolve_relationships(
                resolver,
                budget,
                resource,
                relation,
                source,
                path_depth.saturating_add(1),
            )?;
            let mut selected = None;
            for relationship in relationships.iter().filter(|relationship| {
                relationship.subject.object_type == target_type
                    && relationship.subject_relation.as_deref() == Some(subject_relation)
                    && relationship.caveat.is_none()
            }) {
                let target_definition =
                    find_definition(program, target_type).ok_or(EvaluationError::InvalidRequest)?;
                let target_relation = find_relation(target_definition, subject_relation)
                    .ok_or(EvaluationError::InvalidRequest)?;
                let target_relationships = resolve_relationships(
                    resolver,
                    budget,
                    &relationship.subject,
                    subject_relation,
                    target_relation,
                    path_depth.saturating_add(2),
                )?;
                if let Some(target) = target_relationships.iter().find(|target| {
                    target.subject == request.subject
                        && target.subject_relation.is_none()
                        && target.caveat.is_none()
                }) && selected.is_none()
                {
                    let mut outcome = TraceOutcome::allowed(relationship.witness_id);
                    push_witness_id(&mut outcome.used_witness_ids, target.witness_id);
                    selected = Some(outcome);
                }
            }
            Ok(selected.unwrap_or_else(TraceOutcome::denied))
        }
        PermissionExpr::AnyOf(children) => {
            let mut selected = None;
            for child in children {
                let outcome = evaluate_permission_expr(
                    program, request, resolver, budget, resource, child, path_depth,
                )?;
                if outcome.allowed && selected.is_none() {
                    selected = Some(outcome);
                }
            }
            Ok(selected.unwrap_or_else(TraceOutcome::denied))
        }
        PermissionExpr::AllOf(children) => {
            let mut allowed = true;
            let mut used_witness_ids = Vec::new();
            for child in children {
                let outcome = evaluate_permission_expr(
                    program, request, resolver, budget, resource, child, path_depth,
                )?;
                allowed &= outcome.allowed;
                merge_witness_ids(&mut used_witness_ids, outcome.used_witness_ids);
            }
            Ok(if allowed {
                TraceOutcome {
                    allowed,
                    used_witness_ids,
                }
            } else {
                TraceOutcome::denied()
            })
        }
        PermissionExpr::Arrow {
            source_relation,
            target_permission,
        } => {
            let source = find_relation(definition, source_relation)
                .ok_or(EvaluationError::InvalidRequest)?;
            let target_type =
                resolve_arrow_target(source).map_err(|_| EvaluationError::InvalidRequest)?;
            let relationships = resolve_relationships(
                resolver,
                budget,
                resource,
                source_relation,
                source,
                path_depth.saturating_add(1),
            )?;
            let mut selected = None;
            for relationship in relationships.iter().filter(|relationship| {
                relationship.subject.object_type == target_type
                    && relationship.subject_relation.is_none()
                    && relationship.caveat.is_none()
            }) {
                let target = find_permission(program, target_type, target_permission)
                    .ok_or(EvaluationError::MissingPermission)?;
                let outcome = evaluate_permission(
                    program,
                    request,
                    resolver,
                    budget,
                    &relationship.subject,
                    target,
                    path_depth.saturating_add(1),
                )?;
                if outcome.allowed && selected.is_none() {
                    let mut outcome = outcome;
                    push_witness_id(&mut outcome.used_witness_ids, relationship.witness_id);
                    selected = Some(outcome);
                }
            }
            Ok(selected.unwrap_or_else(TraceOutcome::denied))
        }
        PermissionExpr::CaveatedRelation {
            relation,
            caveat_name,
        } => {
            let source =
                find_relation(definition, relation).ok_or(EvaluationError::InvalidRequest)?;
            let caveat =
                find_caveat(program, caveat_name).ok_or(EvaluationError::InvalidRequest)?;
            let relationships = resolve_relationships(
                resolver,
                budget,
                resource,
                relation,
                source,
                path_depth.saturating_add(1),
            )?;
            let mut selected = None;
            for relationship in relationships.iter().filter(|relationship| {
                relationship.subject == request.subject
                    && relationship.subject_relation.is_none()
                    && relationship
                        .caveat
                        .as_ref()
                        .map(|value| value.name.as_str())
                        == Some(caveat_name)
            }) {
                let relationship_context = &relationship
                    .caveat
                    .as_ref()
                    .ok_or(EvaluationError::InvalidRelationshipData)?
                    .context;
                let result = evaluate_caveat_expr(
                    &caveat.expression,
                    request,
                    relationship_context,
                    budget,
                )?;
                let allowed = as_bool(result)?;
                if allowed && selected.is_none() {
                    selected = Some(TraceOutcome::allowed(relationship.witness_id));
                }
            }
            Ok(selected.unwrap_or_else(TraceOutcome::denied))
        }
    }
}

fn push_witness_id(ids: &mut Vec<u32>, witness_id: Option<u32>) {
    if let Some(witness_id) = witness_id {
        match ids.binary_search(&witness_id) {
            Ok(_) => {}
            Err(index) => ids.insert(index, witness_id),
        }
    }
}

fn merge_witness_ids(ids: &mut Vec<u32>, incoming: Vec<u32>) {
    for witness_id in incoming {
        push_witness_id(ids, Some(witness_id));
    }
}

fn resolve_relationships<'a, R: RelationshipResolver>(
    resolver: &'a R,
    budget: &mut EvaluationBudget,
    resource: &ObjectRef,
    relation_name: &str,
    relation: &RelationDefinition,
    path_depth: u32,
) -> Result<&'a [ResolvedRelationship], EvaluationError> {
    budget.check_path_depth(path_depth)?;
    let relationships = resolver.resolve(resource, relation_name)?;
    if relationships
        .windows(2)
        .any(|pair| compare_relationship_key(&pair[0], &pair[1]) != Ordering::Less)
    {
        return Err(EvaluationError::InvalidRelationshipData);
    }
    for relationship in relationships {
        validate_relationship(relationship, relation, budget)?;
    }
    Ok(relationships)
}

fn validate_relationship(
    relationship: &ResolvedRelationship,
    relation: &RelationDefinition,
    budget: &mut EvaluationBudget,
) -> Result<(), EvaluationError> {
    check_runtime_string(&relationship.subject.object_type, budget.limits)?;
    check_runtime_string(&relationship.subject.object_id, budget.limits)?;
    if relationship.subject.object_type.is_empty() || relationship.subject.object_id.is_empty() {
        return Err(EvaluationError::InvalidRelationshipData);
    }
    if let Some(subject_relation) = &relationship.subject_relation {
        check_runtime_identifier(subject_relation, budget.limits)?;
    }
    if let Some(caveat) = &relationship.caveat {
        check_runtime_identifier(&caveat.name, budget.limits)?;
        validate_context(&caveat.context, budget.limits)?;
    }
    if relationship.subject_relation.is_some() && relationship.caveat.is_some() {
        return Err(EvaluationError::InvalidRelationshipData);
    }
    let caveat_name = relationship
        .caveat
        .as_ref()
        .map(|caveat| caveat.name.as_str());
    if !relation.allowed_subject_types.iter().any(|allowed| {
        allowed.object_type == relationship.subject.object_type
            && allowed.subject_relation.as_deref() == relationship.subject_relation.as_deref()
            && allowed.caveat_name.as_deref() == caveat_name
    }) {
        return Err(EvaluationError::InvalidRelationshipData);
    }
    let first_witness_use = budget.consume_relationship(relationship)?;
    if first_witness_use && let Some(caveat) = &relationship.caveat {
        budget.consume_context_entries(caveat.context.len())?;
    }
    Ok(())
}

fn compare_relationship_key(left: &ResolvedRelationship, right: &ResolvedRelationship) -> Ordering {
    left.subject
        .cmp(&right.subject)
        .then_with(|| left.subject_relation.cmp(&right.subject_relation))
}

fn evaluate_caveat_expr(
    expression: &CaveatExpr,
    request: &EvaluationRequest,
    relationship_context: &[ContextEntry],
    budget: &mut EvaluationBudget,
) -> Result<CaveatValue, EvaluationError> {
    budget.consume_caveat_node()?;
    match expression {
        CaveatExpr::ConstBool(value) => Ok(CaveatValue::Bool(*value)),
        CaveatExpr::ConstInt(value) => Ok(CaveatValue::Int(*value)),
        CaveatExpr::ConstString(value) => Ok(CaveatValue::String(value.clone())),
        CaveatExpr::InputRef {
            source,
            name,
            value_type,
        } => {
            let context = match source {
                CaveatInputSource::RequestContext => &request.request_context,
                CaveatInputSource::RelationshipContext => relationship_context,
                CaveatInputSource::CapabilityClaim => &request.capability_claims,
            };
            let value = context
                .iter()
                .find(|entry| entry.name == *name)
                .ok_or(EvaluationError::MissingCaveatInput)?
                .value
                .clone();
            if value.value_type() != *value_type {
                return Err(EvaluationError::CaveatInputTypeMismatch);
            }
            Ok(value)
        }
        CaveatExpr::Equal(left, right) => Ok(CaveatValue::Bool(
            evaluate_caveat_expr(left, request, relationship_context, budget)?
                == evaluate_caveat_expr(right, request, relationship_context, budget)?,
        )),
        CaveatExpr::NotEqual(left, right) => Ok(CaveatValue::Bool(
            evaluate_caveat_expr(left, request, relationship_context, budget)?
                != evaluate_caveat_expr(right, request, relationship_context, budget)?,
        )),
        CaveatExpr::LessThan(left, right) => compare_ints(
            left,
            right,
            request,
            relationship_context,
            budget,
            |left, right| left < right,
        ),
        CaveatExpr::LessThanOrEqual(left, right) => compare_ints(
            left,
            right,
            request,
            relationship_context,
            budget,
            |left, right| left <= right,
        ),
        CaveatExpr::GreaterThan(left, right) => compare_ints(
            left,
            right,
            request,
            relationship_context,
            budget,
            |left, right| left > right,
        ),
        CaveatExpr::GreaterThanOrEqual(left, right) => compare_ints(
            left,
            right,
            request,
            relationship_context,
            budget,
            |left, right| left >= right,
        ),
        CaveatExpr::And(children) => {
            evaluate_boolean_children(children, request, relationship_context, budget, false)
        }
        CaveatExpr::Or(children) => {
            evaluate_boolean_children(children, request, relationship_context, budget, true)
        }
        CaveatExpr::Not(child) => Ok(CaveatValue::Bool(!as_bool(evaluate_caveat_expr(
            child,
            request,
            relationship_context,
            budget,
        )?)?)),
    }
}

fn evaluate_boolean_children(
    children: &[CaveatExpr],
    request: &EvaluationRequest,
    relationship_context: &[ContextEntry],
    budget: &mut EvaluationBudget,
    decisive_value: bool,
) -> Result<CaveatValue, EvaluationError> {
    let mut found_decisive_value = false;
    let mut found_missing_input = false;
    let mut first_non_missing_error = None;
    for child in children {
        match evaluate_caveat_expr(child, request, relationship_context, budget).and_then(as_bool) {
            Ok(value) => found_decisive_value |= value == decisive_value,
            Err(EvaluationError::MissingCaveatInput) => found_missing_input = true,
            Err(error) => {
                first_non_missing_error.get_or_insert(error);
            }
        }
    }

    if let Some(error) = first_non_missing_error {
        Err(error)
    } else if found_decisive_value {
        Ok(CaveatValue::Bool(decisive_value))
    } else if found_missing_input {
        Err(EvaluationError::MissingCaveatInput)
    } else {
        Ok(CaveatValue::Bool(!decisive_value))
    }
}

fn compare_ints(
    left: &CaveatExpr,
    right: &CaveatExpr,
    request: &EvaluationRequest,
    relationship_context: &[ContextEntry],
    budget: &mut EvaluationBudget,
    compare: impl Fn(i64, i64) -> bool,
) -> Result<CaveatValue, EvaluationError> {
    let CaveatValue::Int(left) = evaluate_caveat_expr(left, request, relationship_context, budget)?
    else {
        return Err(EvaluationError::CaveatInputTypeMismatch);
    };
    let CaveatValue::Int(right) =
        evaluate_caveat_expr(right, request, relationship_context, budget)?
    else {
        return Err(EvaluationError::CaveatInputTypeMismatch);
    };
    Ok(CaveatValue::Bool(compare(left, right)))
}

fn as_bool(value: CaveatValue) -> Result<bool, EvaluationError> {
    match value {
        CaveatValue::Bool(value) => Ok(value),
        CaveatValue::Int(_) | CaveatValue::String(_) => {
            Err(EvaluationError::CaveatInputTypeMismatch)
        }
    }
}

fn validate_request(
    request: &EvaluationRequest,
    limits: PolicyLimits,
) -> Result<(), EvaluationError> {
    check_runtime_identifier(&request.resource.object_type, limits)?;
    check_runtime_identifier(&request.subject.object_type, limits)?;
    check_runtime_identifier(&request.permission, limits)?;
    check_runtime_string(&request.resource.object_id, limits)?;
    check_runtime_string(&request.subject.object_id, limits)?;
    if request.resource.object_id.is_empty() || request.subject.object_id.is_empty() {
        return Err(EvaluationError::InvalidRequest);
    }
    validate_context(&request.request_context, limits)?;
    validate_context(&request.capability_claims, limits)
}

fn validate_context(context: &[ContextEntry], limits: PolicyLimits) -> Result<(), EvaluationError> {
    if context.windows(2).any(|pair| pair[0].name >= pair[1].name) {
        return Err(EvaluationError::InvalidRequest);
    }
    for entry in context {
        check_runtime_identifier(&entry.name, limits)?;
        if let CaveatValue::String(value) = &entry.value {
            check_runtime_string(value, limits)?;
        }
    }
    Ok(())
}

fn check_runtime_identifier(value: &str, limits: PolicyLimits) -> Result<(), EvaluationError> {
    check_runtime_string(value, limits)?;
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return Err(EvaluationError::InvalidRequest);
    };
    if !first.is_ascii_lowercase()
        || !bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(EvaluationError::InvalidRequest);
    }
    Ok(())
}

fn check_runtime_string(value: &str, limits: PolicyLimits) -> Result<(), EvaluationError> {
    if value.len() > limits.max_string_bytes as usize {
        return Err(EvaluationError::BudgetExceeded(LimitKind::StringBytes));
    }
    Ok(())
}

struct EvaluationBudget {
    limits: PolicyLimits,
    expression_nodes: u32,
    caveat_nodes: u32,
    relationship_witnesses: u32,
    counted_witness_ids: Vec<u32>,
    context_entries: u32,
}

impl EvaluationBudget {
    const fn new(limits: PolicyLimits) -> Self {
        Self {
            limits,
            expression_nodes: 0,
            caveat_nodes: 0,
            relationship_witnesses: 0,
            counted_witness_ids: Vec::new(),
            context_entries: 0,
        }
    }

    fn consume_expression_node(&mut self) -> Result<(), EvaluationError> {
        consume(
            &mut self.expression_nodes,
            1,
            self.limits.max_expression_nodes,
            LimitKind::ExpressionNodes,
        )
    }

    fn consume_caveat_node(&mut self) -> Result<(), EvaluationError> {
        consume(
            &mut self.caveat_nodes,
            1,
            self.limits.max_caveat_nodes,
            LimitKind::CaveatNodes,
        )
    }

    fn consume_context_entries(&mut self, count: usize) -> Result<(), EvaluationError> {
        let count = u32::try_from(count)
            .map_err(|_| EvaluationError::BudgetExceeded(LimitKind::ContextEntries))?;
        consume(
            &mut self.context_entries,
            count,
            self.limits.max_context_entries,
            LimitKind::ContextEntries,
        )
    }

    fn consume_relationship(
        &mut self,
        relationship: &ResolvedRelationship,
    ) -> Result<bool, EvaluationError> {
        if let Some(witness_id) = relationship.witness_id {
            match self.counted_witness_ids.binary_search(&witness_id) {
                Ok(_) => return Ok(false),
                Err(index) => self.counted_witness_ids.insert(index, witness_id),
            }
        }
        consume(
            &mut self.relationship_witnesses,
            1,
            self.limits.max_relationship_witnesses,
            LimitKind::RelationshipWitnesses,
        )?;
        Ok(true)
    }

    fn check_path_depth(&self, path_depth: u32) -> Result<(), EvaluationError> {
        if path_depth > self.limits.max_path_depth {
            return Err(EvaluationError::BudgetExceeded(LimitKind::PathDepth));
        }
        Ok(())
    }
}

fn consume(
    current: &mut u32,
    amount: u32,
    maximum: u32,
    kind: LimitKind,
) -> Result<(), EvaluationError> {
    let next = current
        .checked_add(amount)
        .ok_or(EvaluationError::BudgetExceeded(kind))?;
    if next > maximum {
        return Err(EvaluationError::BudgetExceeded(kind));
    }
    *current = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_LIMITS: PolicyLimits = PolicyLimits {
        max_expression_nodes: 8,
        max_caveat_nodes: 8,
        max_path_depth: 2,
        max_relationship_witnesses: 2,
        max_merkle_depth: 2,
        max_string_bytes: 32,
        max_context_entries: 4,
        max_total_private_input_bytes: 128,
    };

    #[test]
    fn boolean_composites_visit_later_children_after_non_missing_error() {
        let children = [
            CaveatExpr::InputRef {
                source: CaveatInputSource::CapabilityClaim,
                name: "flag".into(),
                value_type: crate::CaveatValueType::Bool,
            },
            CaveatExpr::ConstBool(false),
        ];
        let request = EvaluationRequest {
            resource: ObjectRef {
                object_type: "document".into(),
                object_id: "one".into(),
            },
            subject: ObjectRef {
                object_type: "user".into(),
                object_id: "alice".into(),
            },
            permission: "read".into(),
            request_context: vec![],
            capability_claims: vec![ContextEntry {
                name: "flag".into(),
                value: CaveatValue::String("wrong-type".into()),
            }],
        };

        for decisive_value in [false, true] {
            let mut budget = EvaluationBudget::new(TEST_LIMITS);
            assert_eq!(
                evaluate_boolean_children(&children, &request, &[], &mut budget, decisive_value),
                Err(EvaluationError::CaveatInputTypeMismatch)
            );
            assert_eq!(budget.caveat_nodes, 2);
        }
    }

    #[test]
    fn witness_budget_counts_a_present_leaf_id_once() {
        let relationship = ResolvedRelationship {
            subject: ObjectRef {
                object_type: "user".into(),
                object_id: "alice".into(),
            },
            subject_relation: None,
            caveat: None,
            witness_id: Some(7),
        };
        let mut budget = EvaluationBudget::new(PolicyLimits {
            max_relationship_witnesses: 1,
            ..TEST_LIMITS
        });

        assert_eq!(budget.consume_relationship(&relationship), Ok(true));
        assert_eq!(budget.consume_relationship(&relationship), Ok(false));

        let mut another = relationship;
        another.witness_id = Some(8);
        assert_eq!(
            budget.consume_relationship(&another),
            Err(EvaluationError::BudgetExceeded(
                LimitKind::RelationshipWitnesses
            ))
        );
    }
}
