use std::cmp::Ordering;

use crate::{
    AllowedSubjectType, CaveatDefinition, CaveatExpr, CaveatValueType, CollectionKind, LimitKind,
    ObjectDefinition, PermissionDefinition, PermissionExpr, PolicyError, PolicyLimits,
    PolicyProgram, ReferenceKind, RelationDefinition, SUPPORTED_POLICY_IR_VERSION,
    ValidationCeilings,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedPolicyProgram {
    program: PolicyProgram,
}

impl ValidatedPolicyProgram {
    pub fn new(program: PolicyProgram, ceilings: &ValidationCeilings) -> Result<Self, PolicyError> {
        validate_program(&program, ceilings)?;
        Ok(Self { program })
    }

    pub const fn program(&self) -> &PolicyProgram {
        &self.program
    }
}

fn validate_program(
    program: &PolicyProgram,
    ceilings: &ValidationCeilings,
) -> Result<(), PolicyError> {
    if program.ir_version != SUPPORTED_POLICY_IR_VERSION {
        return Err(PolicyError::UnsupportedVersion);
    }
    validate_limits(program.limits, ceilings.policy_limits)?;
    validate_metadata(program, ceilings)?;
    check_collection(
        program.definitions.len(),
        ceilings.max_definitions,
        CollectionKind::Definitions,
    )?;
    check_collection(
        program.permissions.len(),
        ceilings.max_permissions,
        CollectionKind::Permissions,
    )?;
    check_collection(
        program.caveats.len(),
        ceilings.max_caveats,
        CollectionKind::Caveats,
    )?;
    if program.definitions.is_empty() {
        return Err(PolicyError::InvalidChildCount(CollectionKind::Definitions));
    }
    if program.permissions.is_empty() {
        return Err(PolicyError::InvalidChildCount(CollectionKind::Permissions));
    }
    require_sorted_by(
        &program.definitions,
        CollectionKind::Definitions,
        |left, right| left.name.cmp(&right.name),
    )?;
    require_sorted_by(
        &program.permissions,
        CollectionKind::Permissions,
        |left, right| (&left.object_type, &left.name).cmp(&(&right.object_type, &right.name)),
    )?;
    require_sorted_by(&program.caveats, CollectionKind::Caveats, |left, right| {
        left.name.cmp(&right.name)
    })?;

    for definition in &program.definitions {
        validate_definition(definition, program, ceilings)?;
    }
    for caveat in &program.caveats {
        validate_caveat(caveat, program.limits, ceilings)?;
    }
    for permission in &program.permissions {
        validate_permission(permission, program, ceilings)?;
    }

    let expression_nodes = program
        .permissions
        .iter()
        .map(|permission| count_permission_nodes(&permission.expression))
        .try_fold(0_u32, |total, next| {
            checked_sum(total, next?, LimitKind::ExpressionNodes)
        })?;
    if expression_nodes > program.limits.max_expression_nodes {
        return Err(PolicyError::ProgramLimitExceeded(
            LimitKind::ExpressionNodes,
        ));
    }

    let caveat_nodes = program
        .caveats
        .iter()
        .map(|caveat| count_caveat_nodes(&caveat.expression))
        .try_fold(0_u32, |total, next| {
            checked_sum(total, next?, LimitKind::CaveatNodes)
        })?;
    if caveat_nodes > program.limits.max_caveat_nodes {
        return Err(PolicyError::ProgramLimitExceeded(LimitKind::CaveatNodes));
    }

    for permission in &program.permissions {
        let mut stack = Vec::new();
        let (arrow_depth, path_depth) = permission_metrics(program, permission, &mut stack)?;
        if arrow_depth > 1 {
            return Err(PolicyError::ArrowDepthExceeded);
        }
        if path_depth > program.limits.max_path_depth {
            return Err(PolicyError::ProgramLimitExceeded(LimitKind::PathDepth));
        }
    }

    Ok(())
}

fn validate_metadata(
    program: &PolicyProgram,
    ceilings: &ValidationCeilings,
) -> Result<(), PolicyError> {
    if program.compiler_name.is_empty() || program.compiler_version.is_empty() {
        return Err(PolicyError::InvalidMetadata);
    }
    check_string(&program.compiler_name, program.limits)?;
    check_string(&program.compiler_version, program.limits)?;
    if program.source_schema_digest.is_empty()
        || program.source_schema_digest.len() > ceilings.max_source_schema_digest_bytes as usize
    {
        return Err(PolicyError::InvalidSourceSchemaDigest);
    }
    Ok(())
}

fn validate_definition(
    definition: &ObjectDefinition,
    program: &PolicyProgram,
    ceilings: &ValidationCeilings,
) -> Result<(), PolicyError> {
    validate_identifier(&definition.name, program.limits)?;
    check_collection(
        definition.relations.len(),
        ceilings.max_relations_per_definition,
        CollectionKind::Relations,
    )?;
    require_sorted_by(
        &definition.relations,
        CollectionKind::Relations,
        |left, right| left.name.cmp(&right.name),
    )?;
    for relation in &definition.relations {
        validate_relation(relation, program, ceilings)?;
    }
    Ok(())
}

fn validate_relation(
    relation: &RelationDefinition,
    program: &PolicyProgram,
    ceilings: &ValidationCeilings,
) -> Result<(), PolicyError> {
    validate_identifier(&relation.name, program.limits)?;
    check_collection(
        relation.allowed_subject_types.len(),
        ceilings.max_allowed_subject_types_per_relation,
        CollectionKind::AllowedSubjectTypes,
    )?;
    if relation.allowed_subject_types.is_empty() {
        return Err(PolicyError::InvalidChildCount(
            CollectionKind::AllowedSubjectTypes,
        ));
    }
    require_sorted(
        &relation.allowed_subject_types,
        CollectionKind::AllowedSubjectTypes,
    )?;
    for allowed in &relation.allowed_subject_types {
        validate_allowed_subject(allowed, program)?;
    }
    Ok(())
}

fn validate_allowed_subject(
    allowed: &AllowedSubjectType,
    program: &PolicyProgram,
) -> Result<(), PolicyError> {
    validate_identifier(&allowed.object_type, program.limits)?;
    let target = find_definition(program, &allowed.object_type)
        .ok_or(PolicyError::MissingReference(ReferenceKind::Object))?;
    if let Some(subject_relation) = &allowed.subject_relation {
        validate_identifier(subject_relation, program.limits)?;
        find_relation(target, subject_relation)
            .ok_or(PolicyError::MissingReference(ReferenceKind::Relation))?;
    }
    if let Some(caveat_name) = &allowed.caveat_name {
        validate_identifier(caveat_name, program.limits)?;
        find_caveat(program, caveat_name)
            .ok_or(PolicyError::MissingReference(ReferenceKind::Caveat))?;
    }
    if allowed.subject_relation.is_some() && allowed.caveat_name.is_some() {
        return Err(PolicyError::UnsupportedRelationshipShape);
    }
    Ok(())
}

fn validate_permission(
    permission: &PermissionDefinition,
    program: &PolicyProgram,
    ceilings: &ValidationCeilings,
) -> Result<(), PolicyError> {
    validate_identifier(&permission.object_type, program.limits)?;
    validate_identifier(&permission.name, program.limits)?;
    find_definition(program, &permission.object_type)
        .ok_or(PolicyError::MissingReference(ReferenceKind::Object))?;
    validate_permission_expr(
        &permission.expression,
        &permission.object_type,
        program,
        ceilings,
        1,
    )
}

fn validate_permission_expr(
    expression: &PermissionExpr,
    owner_type: &str,
    program: &PolicyProgram,
    ceilings: &ValidationCeilings,
    syntax_depth: u32,
) -> Result<(), PolicyError> {
    check_syntax_depth(syntax_depth, ceilings)?;
    let owner = find_definition(program, owner_type)
        .ok_or(PolicyError::MissingReference(ReferenceKind::Object))?;
    match expression {
        PermissionExpr::RelationRef { name } => {
            validate_identifier(name, program.limits)?;
            find_relation(owner, name)
                .ok_or(PolicyError::MissingReference(ReferenceKind::Relation))?;
        }
        PermissionExpr::SubjectRelationRef {
            relation,
            subject_relation,
        } => {
            validate_identifier(relation, program.limits)?;
            validate_identifier(subject_relation, program.limits)?;
            let source = find_relation(owner, relation)
                .ok_or(PolicyError::MissingReference(ReferenceKind::Relation))?;
            resolve_subject_set_target(source, subject_relation, program)?;
        }
        PermissionExpr::AnyOf(children) | PermissionExpr::AllOf(children) => {
            validate_permission_children(children)?;
            for child in children {
                validate_permission_expr(
                    child,
                    owner_type,
                    program,
                    ceilings,
                    syntax_depth.saturating_add(1),
                )?;
            }
        }
        PermissionExpr::Arrow {
            source_relation,
            target_permission,
        } => {
            validate_identifier(source_relation, program.limits)?;
            validate_identifier(target_permission, program.limits)?;
            let source = find_relation(owner, source_relation)
                .ok_or(PolicyError::MissingReference(ReferenceKind::Relation))?;
            let target_type = resolve_arrow_target(source)?;
            find_permission(program, target_type, target_permission)
                .ok_or(PolicyError::MissingReference(ReferenceKind::Permission))?;
        }
        PermissionExpr::CaveatedRelation {
            relation,
            caveat_name,
        } => {
            validate_identifier(relation, program.limits)?;
            validate_identifier(caveat_name, program.limits)?;
            find_caveat(program, caveat_name)
                .ok_or(PolicyError::MissingReference(ReferenceKind::Caveat))?;
            let source = find_relation(owner, relation)
                .ok_or(PolicyError::MissingReference(ReferenceKind::Relation))?;
            if !source.allowed_subject_types.iter().any(|allowed| {
                allowed.subject_relation.is_none()
                    && allowed.caveat_name.as_deref() == Some(caveat_name)
            }) {
                return Err(PolicyError::MissingReference(ReferenceKind::Caveat));
            }
        }
    }
    Ok(())
}

fn validate_permission_children(children: &[PermissionExpr]) -> Result<(), PolicyError> {
    if children.len() < 2 {
        return Err(PolicyError::InvalidChildCount(
            CollectionKind::PermissionChildren,
        ));
    }
    require_sorted(children, CollectionKind::PermissionChildren)
}

fn validate_caveat(
    caveat: &CaveatDefinition,
    limits: PolicyLimits,
    ceilings: &ValidationCeilings,
) -> Result<(), PolicyError> {
    validate_identifier(&caveat.name, limits)?;
    check_collection(
        caveat.parameters.len(),
        ceilings.max_parameters_per_caveat,
        CollectionKind::CaveatParameters,
    )?;
    require_sorted_by(
        &caveat.parameters,
        CollectionKind::CaveatParameters,
        |left, right| (&left.source, &left.name).cmp(&(&right.source, &right.name)),
    )?;
    for parameter in &caveat.parameters {
        validate_identifier(&parameter.name, limits)?;
    }
    let root_type = validate_caveat_expr(&caveat.expression, caveat, limits, ceilings, 1)?;
    if root_type != CaveatValueType::Bool {
        return Err(PolicyError::CaveatRootNotBoolean);
    }
    Ok(())
}

fn validate_caveat_expr(
    expression: &CaveatExpr,
    caveat: &CaveatDefinition,
    limits: PolicyLimits,
    ceilings: &ValidationCeilings,
    syntax_depth: u32,
) -> Result<CaveatValueType, PolicyError> {
    check_syntax_depth(syntax_depth, ceilings)?;
    let child_depth = syntax_depth.saturating_add(1);
    match expression {
        CaveatExpr::ConstBool(_) => Ok(CaveatValueType::Bool),
        CaveatExpr::ConstInt(_) => Ok(CaveatValueType::Int),
        CaveatExpr::ConstString(value) => {
            check_string(value, limits)?;
            Ok(CaveatValueType::String)
        }
        CaveatExpr::InputRef {
            source,
            name,
            value_type,
        } => {
            validate_identifier(name, limits)?;
            let parameter = caveat
                .parameters
                .iter()
                .find(|parameter| parameter.source == *source && parameter.name == *name)
                .ok_or(PolicyError::MissingReference(
                    ReferenceKind::CaveatParameter,
                ))?;
            if parameter.value_type != *value_type {
                return Err(PolicyError::CaveatTypeMismatch);
            }
            Ok(*value_type)
        }
        CaveatExpr::Equal(left, right) | CaveatExpr::NotEqual(left, right) => {
            let left_type = validate_caveat_expr(left, caveat, limits, ceilings, child_depth)?;
            let right_type = validate_caveat_expr(right, caveat, limits, ceilings, child_depth)?;
            if left_type != right_type {
                return Err(PolicyError::CaveatTypeMismatch);
            }
            Ok(CaveatValueType::Bool)
        }
        CaveatExpr::LessThan(left, right)
        | CaveatExpr::LessThanOrEqual(left, right)
        | CaveatExpr::GreaterThan(left, right)
        | CaveatExpr::GreaterThanOrEqual(left, right) => {
            let left_type = validate_caveat_expr(left, caveat, limits, ceilings, child_depth)?;
            let right_type = validate_caveat_expr(right, caveat, limits, ceilings, child_depth)?;
            if left_type != CaveatValueType::Int || right_type != CaveatValueType::Int {
                return Err(PolicyError::CaveatTypeMismatch);
            }
            Ok(CaveatValueType::Bool)
        }
        CaveatExpr::And(children) | CaveatExpr::Or(children) => {
            if children.len() < 2 {
                return Err(PolicyError::InvalidChildCount(
                    CollectionKind::CaveatChildren,
                ));
            }
            require_sorted(children, CollectionKind::CaveatChildren)?;
            for child in children {
                if validate_caveat_expr(child, caveat, limits, ceilings, child_depth)?
                    != CaveatValueType::Bool
                {
                    return Err(PolicyError::CaveatTypeMismatch);
                }
            }
            Ok(CaveatValueType::Bool)
        }
        CaveatExpr::Not(child) => {
            if validate_caveat_expr(child, caveat, limits, ceilings, child_depth)?
                != CaveatValueType::Bool
            {
                return Err(PolicyError::CaveatTypeMismatch);
            }
            Ok(CaveatValueType::Bool)
        }
    }
}

fn permission_metrics(
    program: &PolicyProgram,
    permission: &PermissionDefinition,
    stack: &mut Vec<(String, String)>,
) -> Result<(u32, u32), PolicyError> {
    let key = (permission.object_type.clone(), permission.name.clone());
    if stack.contains(&key) {
        return Err(PolicyError::PermissionCycle);
    }
    stack.push(key);
    let result = expression_metrics(
        program,
        &permission.object_type,
        &permission.expression,
        stack,
    );
    stack.pop();
    result
}

fn expression_metrics(
    program: &PolicyProgram,
    owner_type: &str,
    expression: &PermissionExpr,
    stack: &mut Vec<(String, String)>,
) -> Result<(u32, u32), PolicyError> {
    match expression {
        PermissionExpr::RelationRef { .. } | PermissionExpr::CaveatedRelation { .. } => Ok((0, 1)),
        PermissionExpr::SubjectRelationRef { .. } => Ok((0, 2)),
        PermissionExpr::AnyOf(children) | PermissionExpr::AllOf(children) => children
            .iter()
            .map(|child| expression_metrics(program, owner_type, child, stack))
            .try_fold((0, 0), |current, next| {
                let next = next?;
                Ok((current.0.max(next.0), current.1.max(next.1)))
            }),
        PermissionExpr::Arrow {
            source_relation,
            target_permission,
        } => {
            let owner = find_definition(program, owner_type)
                .ok_or(PolicyError::MissingReference(ReferenceKind::Object))?;
            let source = find_relation(owner, source_relation)
                .ok_or(PolicyError::MissingReference(ReferenceKind::Relation))?;
            let target_type = resolve_arrow_target(source)?;
            let target = find_permission(program, target_type, target_permission)
                .ok_or(PolicyError::MissingReference(ReferenceKind::Permission))?;
            let (arrows, path) = permission_metrics(program, target, stack)?;
            Ok((
                arrows
                    .checked_add(1)
                    .ok_or(PolicyError::ArrowDepthExceeded)?,
                path.checked_add(1)
                    .ok_or(PolicyError::ProgramLimitExceeded(LimitKind::PathDepth))?,
            ))
        }
    }
}

pub(crate) fn find_definition<'a>(
    program: &'a PolicyProgram,
    name: &str,
) -> Option<&'a ObjectDefinition> {
    program
        .definitions
        .iter()
        .find(|definition| definition.name == name)
}

pub(crate) fn find_relation<'a>(
    definition: &'a ObjectDefinition,
    name: &str,
) -> Option<&'a RelationDefinition> {
    definition
        .relations
        .iter()
        .find(|relation| relation.name == name)
}

pub(crate) fn find_permission<'a>(
    program: &'a PolicyProgram,
    object_type: &str,
    name: &str,
) -> Option<&'a PermissionDefinition> {
    program
        .permissions
        .iter()
        .find(|permission| permission.object_type == object_type && permission.name == name)
}

pub(crate) fn find_caveat<'a>(
    program: &'a PolicyProgram,
    name: &str,
) -> Option<&'a CaveatDefinition> {
    program.caveats.iter().find(|caveat| caveat.name == name)
}

pub(crate) fn resolve_arrow_target(relation: &RelationDefinition) -> Result<&str, PolicyError> {
    if relation.allowed_subject_types.len() != 1 {
        return Err(PolicyError::AmbiguousReference(ReferenceKind::Object));
    }
    let target = &relation.allowed_subject_types[0];
    if target.subject_relation.is_some() || target.caveat_name.is_some() {
        return Err(PolicyError::UnsupportedRelationshipShape);
    }
    Ok(&target.object_type)
}

pub(crate) fn resolve_subject_set_target<'a>(
    relation: &'a RelationDefinition,
    subject_relation: &str,
    program: &PolicyProgram,
) -> Result<&'a str, PolicyError> {
    let mut matches = relation.allowed_subject_types.iter().filter(|allowed| {
        allowed.subject_relation.as_deref() == Some(subject_relation)
            && allowed.caveat_name.is_none()
    });
    let target = matches
        .next()
        .ok_or(PolicyError::MissingReference(ReferenceKind::Relation))?;
    if matches.next().is_some() {
        return Err(PolicyError::AmbiguousReference(ReferenceKind::Object));
    }
    let definition = find_definition(program, &target.object_type)
        .ok_or(PolicyError::MissingReference(ReferenceKind::Object))?;
    find_relation(definition, subject_relation)
        .ok_or(PolicyError::MissingReference(ReferenceKind::Relation))?;
    Ok(&target.object_type)
}

fn validate_limits(limits: PolicyLimits, ceilings: PolicyLimits) -> Result<(), PolicyError> {
    for (kind, value, ceiling) in [
        (
            LimitKind::ExpressionNodes,
            limits.max_expression_nodes,
            ceilings.max_expression_nodes,
        ),
        (
            LimitKind::CaveatNodes,
            limits.max_caveat_nodes,
            ceilings.max_caveat_nodes,
        ),
        (
            LimitKind::PathDepth,
            limits.max_path_depth,
            ceilings.max_path_depth,
        ),
        (
            LimitKind::RelationshipWitnesses,
            limits.max_relationship_witnesses,
            ceilings.max_relationship_witnesses,
        ),
        (
            LimitKind::MerkleDepth,
            limits.max_merkle_depth,
            ceilings.max_merkle_depth,
        ),
        (
            LimitKind::StringBytes,
            limits.max_string_bytes,
            ceilings.max_string_bytes,
        ),
        (
            LimitKind::ContextEntries,
            limits.max_context_entries,
            ceilings.max_context_entries,
        ),
        (
            LimitKind::TotalPrivateInputBytes,
            limits.max_total_private_input_bytes,
            ceilings.max_total_private_input_bytes,
        ),
    ] {
        if value == 0 {
            return Err(PolicyError::LimitIsZero(kind));
        }
        if value > ceiling {
            return Err(PolicyError::LimitAboveCeiling(kind));
        }
    }
    Ok(())
}

fn validate_identifier(value: &str, limits: PolicyLimits) -> Result<(), PolicyError> {
    check_string(value, limits)?;
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return Err(PolicyError::InvalidIdentifier);
    };
    if !first.is_ascii_lowercase()
        || !bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(PolicyError::InvalidIdentifier);
    }
    Ok(())
}

fn check_string(value: &str, limits: PolicyLimits) -> Result<(), PolicyError> {
    if value.len() > limits.max_string_bytes as usize {
        return Err(PolicyError::ProgramLimitExceeded(LimitKind::StringBytes));
    }
    Ok(())
}

fn check_collection(length: usize, maximum: u32, kind: CollectionKind) -> Result<(), PolicyError> {
    if length > maximum as usize {
        return Err(PolicyError::CollectionTooLarge(kind));
    }
    Ok(())
}

fn require_sorted<T: Ord>(values: &[T], kind: CollectionKind) -> Result<(), PolicyError> {
    require_sorted_by(values, kind, Ord::cmp)
}

fn require_sorted_by<T>(
    values: &[T],
    kind: CollectionKind,
    compare: impl Fn(&T, &T) -> Ordering,
) -> Result<(), PolicyError> {
    if values
        .windows(2)
        .any(|pair| compare(&pair[0], &pair[1]) != Ordering::Less)
    {
        return Err(PolicyError::NotStrictlySorted(kind));
    }
    Ok(())
}

fn check_syntax_depth(depth: u32, ceilings: &ValidationCeilings) -> Result<(), PolicyError> {
    if depth > ceilings.max_syntax_depth {
        return Err(PolicyError::ProgramLimitExceeded(LimitKind::SyntaxDepth));
    }
    Ok(())
}

fn count_permission_nodes(expression: &PermissionExpr) -> Result<u32, PolicyError> {
    let descendants: u32 = match expression {
        PermissionExpr::AnyOf(children) | PermissionExpr::AllOf(children) => children
            .iter()
            .map(count_permission_nodes)
            .try_fold(0_u32, |total, next| {
                checked_sum(total, next?, LimitKind::ExpressionNodes)
            })?,
        _ => 0,
    };
    descendants
        .checked_add(1)
        .ok_or(PolicyError::ProgramLimitExceeded(
            LimitKind::ExpressionNodes,
        ))
}

fn count_caveat_nodes(expression: &CaveatExpr) -> Result<u32, PolicyError> {
    let descendants: u32 = match expression {
        CaveatExpr::Equal(left, right)
        | CaveatExpr::NotEqual(left, right)
        | CaveatExpr::LessThan(left, right)
        | CaveatExpr::LessThanOrEqual(left, right)
        | CaveatExpr::GreaterThan(left, right)
        | CaveatExpr::GreaterThanOrEqual(left, right) => count_caveat_nodes(left)?
            .checked_add(count_caveat_nodes(right)?)
            .ok_or(PolicyError::ProgramLimitExceeded(LimitKind::CaveatNodes))?,
        CaveatExpr::And(children) | CaveatExpr::Or(children) => children
            .iter()
            .map(count_caveat_nodes)
            .try_fold(0_u32, |total, next| {
                checked_sum(total, next?, LimitKind::CaveatNodes)
            })?,
        CaveatExpr::Not(child) => count_caveat_nodes(child)?,
        CaveatExpr::ConstBool(_)
        | CaveatExpr::ConstInt(_)
        | CaveatExpr::ConstString(_)
        | CaveatExpr::InputRef { .. } => 0,
    };
    descendants
        .checked_add(1)
        .ok_or(PolicyError::ProgramLimitExceeded(LimitKind::CaveatNodes))
}

fn checked_sum(total: u32, value: u32, kind: LimitKind) -> Result<u32, PolicyError> {
    total
        .checked_add(value)
        .ok_or(PolicyError::ProgramLimitExceeded(kind))
}
