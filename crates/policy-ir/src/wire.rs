use raziel_proto::raziel::v1 as proto;

use crate::{
    AllowedSubjectType, CaveatDefinition, CaveatExpr, CaveatInputSource, CaveatParameter,
    CaveatValueType, CollectionKind, LimitKind, ObjectDefinition, PermissionDefinition,
    PermissionExpr, PolicyError, PolicyLimits, PolicyProgram, RelationDefinition,
    ValidatedPolicyProgram, ValidationCeilings,
};

impl ValidatedPolicyProgram {
    pub fn from_proto(
        value: proto::PolicyProgram,
        ceilings: &ValidationCeilings,
    ) -> Result<Self, PolicyError> {
        check_len(
            value.definitions.len(),
            ceilings.max_definitions,
            CollectionKind::Definitions,
        )?;
        check_len(
            value.permissions.len(),
            ceilings.max_permissions,
            CollectionKind::Permissions,
        )?;
        check_len(
            value.caveats.len(),
            ceilings.max_caveats,
            CollectionKind::Caveats,
        )?;

        let program = PolicyProgram {
            ir_version: value.ir_version,
            compiler_name: value.compiler_name,
            compiler_version: value.compiler_version,
            source_schema_digest: value.source_schema_digest,
            definitions: value
                .definitions
                .into_iter()
                .map(|definition| convert_definition(definition, ceilings))
                .collect::<Result<_, _>>()?,
            permissions: value
                .permissions
                .into_iter()
                .map(|permission| convert_permission(permission, ceilings))
                .collect::<Result<_, _>>()?,
            caveats: value
                .caveats
                .into_iter()
                .map(|caveat| convert_caveat(caveat, ceilings))
                .collect::<Result<_, _>>()?,
            limits: value
                .limits
                .map(PolicyLimits::from)
                .ok_or(PolicyError::MissingLimits)?,
        };

        Self::new(program, ceilings)
    }
}

impl From<proto::PolicyLimits> for PolicyLimits {
    fn from(value: proto::PolicyLimits) -> Self {
        Self {
            max_expression_nodes: value.max_expression_nodes,
            max_caveat_nodes: value.max_caveat_nodes,
            max_path_depth: value.max_path_depth,
            max_relationship_witnesses: value.max_relationship_witnesses,
            max_merkle_depth: value.max_merkle_depth,
            max_string_bytes: value.max_string_bytes,
            max_context_entries: value.max_context_entries,
            max_total_private_input_bytes: value.max_total_private_input_bytes,
        }
    }
}

fn convert_definition(
    value: proto::ObjectDefinition,
    ceilings: &ValidationCeilings,
) -> Result<ObjectDefinition, PolicyError> {
    check_len(
        value.relations.len(),
        ceilings.max_relations_per_definition,
        CollectionKind::Relations,
    )?;
    Ok(ObjectDefinition {
        name: value.name,
        relations: value
            .relations
            .into_iter()
            .map(|relation| convert_relation(relation, ceilings))
            .collect::<Result<_, _>>()?,
    })
}

fn convert_relation(
    value: proto::RelationDefinition,
    ceilings: &ValidationCeilings,
) -> Result<RelationDefinition, PolicyError> {
    check_len(
        value.allowed_subject_types.len(),
        ceilings.max_allowed_subject_types_per_relation,
        CollectionKind::AllowedSubjectTypes,
    )?;
    Ok(RelationDefinition {
        name: value.name,
        allowed_subject_types: value
            .allowed_subject_types
            .into_iter()
            .map(|allowed| AllowedSubjectType {
                object_type: allowed.object_type,
                subject_relation: allowed.subject_relation,
                caveat_name: allowed.caveat_name,
            })
            .collect(),
    })
}

fn convert_permission(
    value: proto::PermissionDefinition,
    ceilings: &ValidationCeilings,
) -> Result<PermissionDefinition, PolicyError> {
    let expression = value.expression.ok_or(PolicyError::MissingExpression)?;
    Ok(PermissionDefinition {
        object_type: value.object_type,
        name: value.name,
        expression: convert_permission_expr(expression, 1, ceilings)?,
    })
}

fn convert_permission_expr(
    value: proto::PermissionExpr,
    depth: u32,
    ceilings: &ValidationCeilings,
) -> Result<PermissionExpr, PolicyError> {
    check_syntax_depth(depth, ceilings)?;
    use proto::permission_expr::Node;
    match value.node.ok_or(PolicyError::MissingNode)? {
        Node::RelationRef(node) => Ok(PermissionExpr::RelationRef { name: node.name }),
        Node::SubjectRelationRef(node) => Ok(PermissionExpr::SubjectRelationRef {
            relation: node.relation,
            subject_relation: node.subject_relation,
        }),
        Node::AnyOf(node) => Ok(PermissionExpr::AnyOf(convert_permission_children(
            node.children,
            depth,
            ceilings,
        )?)),
        Node::AllOf(node) => Ok(PermissionExpr::AllOf(convert_permission_children(
            node.children,
            depth,
            ceilings,
        )?)),
        Node::Arrow(node) => Ok(PermissionExpr::Arrow {
            source_relation: node.source_relation,
            target_permission: node.target_permission,
        }),
        Node::CaveatedRelation(node) => Ok(PermissionExpr::CaveatedRelation {
            relation: node.relation,
            caveat_name: node.caveat_name,
        }),
    }
}

fn convert_permission_children(
    children: Vec<proto::PermissionExpr>,
    depth: u32,
    ceilings: &ValidationCeilings,
) -> Result<Vec<PermissionExpr>, PolicyError> {
    check_len(
        children.len(),
        ceilings.policy_limits.max_expression_nodes,
        CollectionKind::PermissionChildren,
    )?;
    children
        .into_iter()
        .map(|child| convert_permission_expr(child, depth.saturating_add(1), ceilings))
        .collect()
}

fn convert_caveat(
    value: proto::CaveatDefinition,
    ceilings: &ValidationCeilings,
) -> Result<CaveatDefinition, PolicyError> {
    check_len(
        value.parameters.len(),
        ceilings.max_parameters_per_caveat,
        CollectionKind::CaveatParameters,
    )?;
    Ok(CaveatDefinition {
        name: value.name,
        parameters: value
            .parameters
            .into_iter()
            .map(convert_parameter)
            .collect::<Result<_, _>>()?,
        expression: convert_caveat_expr(
            value.expression.ok_or(PolicyError::MissingExpression)?,
            1,
            ceilings,
        )?,
    })
}

fn convert_parameter(value: proto::CaveatParameter) -> Result<CaveatParameter, PolicyError> {
    Ok(CaveatParameter {
        source: convert_input_source(value.source)?,
        name: value.name,
        value_type: convert_value_type(value.value_type)?,
    })
}

fn convert_caveat_expr(
    value: proto::CaveatExpr,
    depth: u32,
    ceilings: &ValidationCeilings,
) -> Result<CaveatExpr, PolicyError> {
    check_syntax_depth(depth, ceilings)?;
    use proto::caveat_expr::Node;
    match value.node.ok_or(PolicyError::MissingNode)? {
        Node::ConstBool(value) => Ok(CaveatExpr::ConstBool(value)),
        Node::ConstInt(value) => Ok(CaveatExpr::ConstInt(value)),
        Node::ConstString(value) => Ok(CaveatExpr::ConstString(value)),
        Node::InputRef(node) => Ok(CaveatExpr::InputRef {
            source: convert_input_source(node.source)?,
            name: node.name,
            value_type: convert_value_type(node.value_type)?,
        }),
        Node::Equal(node) => {
            let (left, right) = convert_binary(*node, depth, ceilings)?;
            Ok(CaveatExpr::Equal(left, right))
        }
        Node::NotEqual(node) => {
            let (left, right) = convert_binary(*node, depth, ceilings)?;
            Ok(CaveatExpr::NotEqual(left, right))
        }
        Node::LessThan(node) => {
            let (left, right) = convert_binary(*node, depth, ceilings)?;
            Ok(CaveatExpr::LessThan(left, right))
        }
        Node::LessThanOrEqual(node) => {
            let (left, right) = convert_binary(*node, depth, ceilings)?;
            Ok(CaveatExpr::LessThanOrEqual(left, right))
        }
        Node::GreaterThan(node) => {
            let (left, right) = convert_binary(*node, depth, ceilings)?;
            Ok(CaveatExpr::GreaterThan(left, right))
        }
        Node::GreaterThanOrEqual(node) => {
            let (left, right) = convert_binary(*node, depth, ceilings)?;
            Ok(CaveatExpr::GreaterThanOrEqual(left, right))
        }
        Node::And(node) => Ok(CaveatExpr::And(convert_caveat_children(
            node.children,
            depth,
            ceilings,
        )?)),
        Node::Or(node) => Ok(CaveatExpr::Or(convert_caveat_children(
            node.children,
            depth,
            ceilings,
        )?)),
        Node::Not(node) => Ok(CaveatExpr::Not(Box::new(convert_caveat_expr(
            *node.child.ok_or(PolicyError::MissingOperand)?,
            depth.saturating_add(1),
            ceilings,
        )?))),
    }
}

fn convert_binary(
    value: proto::CaveatBinaryExpr,
    depth: u32,
    ceilings: &ValidationCeilings,
) -> Result<(Box<CaveatExpr>, Box<CaveatExpr>), PolicyError> {
    let left = value.left.ok_or(PolicyError::MissingOperand)?;
    let right = value.right.ok_or(PolicyError::MissingOperand)?;
    Ok((
        Box::new(convert_caveat_expr(
            *left,
            depth.saturating_add(1),
            ceilings,
        )?),
        Box::new(convert_caveat_expr(
            *right,
            depth.saturating_add(1),
            ceilings,
        )?),
    ))
}

fn convert_caveat_children(
    children: Vec<proto::CaveatExpr>,
    depth: u32,
    ceilings: &ValidationCeilings,
) -> Result<Vec<CaveatExpr>, PolicyError> {
    check_len(
        children.len(),
        ceilings.policy_limits.max_caveat_nodes,
        CollectionKind::CaveatChildren,
    )?;
    children
        .into_iter()
        .map(|child| convert_caveat_expr(child, depth.saturating_add(1), ceilings))
        .collect()
}

fn convert_input_source(value: i32) -> Result<CaveatInputSource, PolicyError> {
    match proto::CaveatInputSource::try_from(value).map_err(|_| PolicyError::UnknownInputSource)? {
        proto::CaveatInputSource::RequestContext => Ok(CaveatInputSource::RequestContext),
        proto::CaveatInputSource::RelationshipContext => Ok(CaveatInputSource::RelationshipContext),
        proto::CaveatInputSource::CapabilityClaim => Ok(CaveatInputSource::CapabilityClaim),
        proto::CaveatInputSource::Unspecified => Err(PolicyError::UnknownInputSource),
    }
}

fn convert_value_type(value: i32) -> Result<CaveatValueType, PolicyError> {
    match proto::CaveatValueType::try_from(value).map_err(|_| PolicyError::UnknownValueType)? {
        proto::CaveatValueType::Bool => Ok(CaveatValueType::Bool),
        proto::CaveatValueType::Int => Ok(CaveatValueType::Int),
        proto::CaveatValueType::String => Ok(CaveatValueType::String),
        proto::CaveatValueType::Unspecified => Err(PolicyError::UnknownValueType),
    }
}

fn check_len(length: usize, maximum: u32, kind: CollectionKind) -> Result<(), PolicyError> {
    if length > maximum as usize {
        return Err(PolicyError::CollectionTooLarge(kind));
    }
    Ok(())
}

fn check_syntax_depth(depth: u32, ceilings: &ValidationCeilings) -> Result<(), PolicyError> {
    if depth > ceilings.max_syntax_depth {
        return Err(PolicyError::ProgramLimitExceeded(LimitKind::SyntaxDepth));
    }
    Ok(())
}
