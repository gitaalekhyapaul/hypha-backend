use raziel_policy_ir::{
    AllowedSubjectType, CaveatDefinition, CaveatExpr, CaveatParameter, ObjectDefinition,
    PermissionDefinition, PermissionExpr, PolicyLimits, RelationDefinition, ValidatedPolicyProgram,
};

use crate::{CanonicalEncoder, CanonicalError};

pub const CANONICAL_POLICY_ENCODING_VERSION: u32 = 1;

pub fn encode_policy(program: &ValidatedPolicyProgram) -> Result<Vec<u8>, CanonicalError> {
    let program = program.program();
    let mut encoder = CanonicalEncoder::new();
    encoder.write_u32(CANONICAL_POLICY_ENCODING_VERSION);
    encoder.write_u32(program.ir_version);
    encoder.write_str(&program.compiler_name)?;
    encoder.write_str(&program.compiler_version)?;
    encoder.write_bytes(&program.source_schema_digest)?;
    encoder.write_collection(&program.definitions, encode_definition)?;
    encoder.write_collection(&program.permissions, encode_permission)?;
    encoder.write_collection(&program.caveats, encode_caveat)?;
    encode_limits(&mut encoder, &program.limits);
    let payload = encoder.finish();
    u32::try_from(payload.len()).map_err(|_| CanonicalError::LengthOverflow)?;
    Ok(payload)
}

fn encode_definition(
    encoder: &mut CanonicalEncoder,
    definition: &ObjectDefinition,
) -> Result<(), CanonicalError> {
    encoder.write_str(&definition.name)?;
    encoder.write_collection(&definition.relations, encode_relation)
}

fn encode_relation(
    encoder: &mut CanonicalEncoder,
    relation: &RelationDefinition,
) -> Result<(), CanonicalError> {
    encoder.write_str(&relation.name)?;
    encoder.write_collection(&relation.allowed_subject_types, encode_allowed_subject)
}

fn encode_allowed_subject(
    encoder: &mut CanonicalEncoder,
    allowed: &AllowedSubjectType,
) -> Result<(), CanonicalError> {
    encoder.write_str(&allowed.object_type)?;
    encoder.write_optional_str(allowed.subject_relation.as_deref())?;
    encoder.write_optional_str(allowed.caveat_name.as_deref())
}

fn encode_permission(
    encoder: &mut CanonicalEncoder,
    permission: &PermissionDefinition,
) -> Result<(), CanonicalError> {
    encoder.write_str(&permission.object_type)?;
    encoder.write_str(&permission.name)?;
    encode_permission_expr(encoder, &permission.expression)
}

fn encode_permission_expr(
    encoder: &mut CanonicalEncoder,
    expression: &PermissionExpr,
) -> Result<(), CanonicalError> {
    encoder.write_u8(expression.protocol_tag());
    match expression {
        PermissionExpr::RelationRef { name } => encoder.write_str(name),
        PermissionExpr::SubjectRelationRef {
            relation,
            subject_relation,
        } => {
            encoder.write_str(relation)?;
            encoder.write_str(subject_relation)
        }
        PermissionExpr::AnyOf(children) | PermissionExpr::AllOf(children) => {
            encoder.write_collection(children, encode_permission_expr)
        }
        PermissionExpr::Arrow {
            source_relation,
            target_permission,
        } => {
            encoder.write_str(source_relation)?;
            encoder.write_str(target_permission)
        }
        PermissionExpr::CaveatedRelation {
            relation,
            caveat_name,
        } => {
            encoder.write_str(relation)?;
            encoder.write_str(caveat_name)
        }
    }
}

fn encode_caveat(
    encoder: &mut CanonicalEncoder,
    caveat: &CaveatDefinition,
) -> Result<(), CanonicalError> {
    encoder.write_str(&caveat.name)?;
    encoder.write_collection(&caveat.parameters, encode_caveat_parameter)?;
    encode_caveat_expr(encoder, &caveat.expression)
}

fn encode_caveat_parameter(
    encoder: &mut CanonicalEncoder,
    parameter: &CaveatParameter,
) -> Result<(), CanonicalError> {
    encoder.write_u8(parameter.source.protocol_tag());
    encoder.write_str(&parameter.name)?;
    encoder.write_u8(parameter.value_type.protocol_tag());
    Ok(())
}

fn encode_caveat_expr(
    encoder: &mut CanonicalEncoder,
    expression: &CaveatExpr,
) -> Result<(), CanonicalError> {
    encoder.write_u8(expression.protocol_tag());
    match expression {
        CaveatExpr::ConstBool(value) => {
            encoder.write_bool(*value);
            Ok(())
        }
        CaveatExpr::ConstInt(value) => {
            encoder.write_i64(*value);
            Ok(())
        }
        CaveatExpr::ConstString(value) => encoder.write_str(value),
        CaveatExpr::InputRef {
            source,
            name,
            value_type,
        } => {
            encoder.write_u8(source.protocol_tag());
            encoder.write_str(name)?;
            encoder.write_u8(value_type.protocol_tag());
            Ok(())
        }
        CaveatExpr::Equal(left, right)
        | CaveatExpr::NotEqual(left, right)
        | CaveatExpr::LessThan(left, right)
        | CaveatExpr::LessThanOrEqual(left, right)
        | CaveatExpr::GreaterThan(left, right)
        | CaveatExpr::GreaterThanOrEqual(left, right) => {
            encode_caveat_expr(encoder, left)?;
            encode_caveat_expr(encoder, right)
        }
        CaveatExpr::And(children) | CaveatExpr::Or(children) => {
            encoder.write_collection(children, encode_caveat_expr)
        }
        CaveatExpr::Not(child) => encode_caveat_expr(encoder, child),
    }
}

fn encode_limits(encoder: &mut CanonicalEncoder, limits: &PolicyLimits) {
    encoder.write_u32(limits.max_expression_nodes);
    encoder.write_u32(limits.max_caveat_nodes);
    encoder.write_u32(limits.max_path_depth);
    encoder.write_u32(limits.max_relationship_witnesses);
    encoder.write_u32(limits.max_merkle_depth);
    encoder.write_u32(limits.max_string_bytes);
    encoder.write_u32(limits.max_context_entries);
    encoder.write_u32(limits.max_total_private_input_bytes);
}
