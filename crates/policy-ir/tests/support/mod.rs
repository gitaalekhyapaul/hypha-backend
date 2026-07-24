#![allow(dead_code)]

use std::collections::BTreeMap;

use raziel_policy_ir::{
    AllowedSubjectType, CaveatDefinition, CaveatExpr, CaveatInputSource, CaveatParameter,
    CaveatValue, CaveatValueType, ContextEntry, ObjectDefinition, ObjectRef, PermissionDefinition,
    PermissionExpr, PolicyLimits, PolicyProgram, RelationDefinition, RelationshipCaveat,
    RelationshipResolver, ResolvedRelationship, ResolverError, ValidationCeilings,
};
use raziel_proto::raziel::v1 as proto;

pub const CAVEAT_NAME: &str = "within_limit";

pub fn limits() -> PolicyLimits {
    PolicyLimits {
        max_expression_nodes: 64,
        max_caveat_nodes: 64,
        max_path_depth: 4,
        max_relationship_witnesses: 64,
        max_merkle_depth: 32,
        max_string_bytes: 128,
        max_context_entries: 32,
        max_total_private_input_bytes: 4096,
    }
}

pub fn ceilings() -> ValidationCeilings {
    ValidationCeilings {
        policy_limits: limits(),
        max_definitions: 16,
        max_relations_per_definition: 16,
        max_allowed_subject_types_per_relation: 16,
        max_permissions: 32,
        max_caveats: 16,
        max_parameters_per_caveat: 16,
        max_syntax_depth: 16,
    }
}

pub fn base_program() -> PolicyProgram {
    PolicyProgram {
        ir_version: 1,
        compiler_name: "spicedb_adapter".into(),
        compiler_version: "0.1.0".into(),
        source_schema_digest: [7; 32],
        definitions: vec![
            ObjectDefinition {
                name: "document".into(),
                relations: vec![
                    RelationDefinition {
                        name: "editor".into(),
                        allowed_subject_types: vec![direct_allowed("user")],
                    },
                    RelationDefinition {
                        name: "parent".into(),
                        allowed_subject_types: vec![direct_allowed("folder")],
                    },
                    RelationDefinition {
                        name: "viewer".into(),
                        allowed_subject_types: vec![
                            AllowedSubjectType {
                                object_type: "group".into(),
                                subject_relation: Some("member".into()),
                                caveat_name: None,
                            },
                            direct_allowed("user"),
                            AllowedSubjectType {
                                object_type: "user".into(),
                                subject_relation: None,
                                caveat_name: Some(CAVEAT_NAME.into()),
                            },
                        ],
                    },
                ],
            },
            ObjectDefinition {
                name: "folder".into(),
                relations: vec![RelationDefinition {
                    name: "reader".into(),
                    allowed_subject_types: vec![direct_allowed("user")],
                }],
            },
            ObjectDefinition {
                name: "group".into(),
                relations: vec![RelationDefinition {
                    name: "member".into(),
                    allowed_subject_types: vec![direct_allowed("user")],
                }],
            },
            ObjectDefinition {
                name: "user".into(),
                relations: vec![],
            },
        ],
        permissions: vec![
            PermissionDefinition {
                object_type: "document".into(),
                name: "access".into(),
                expression: PermissionExpr::AnyOf(vec![
                    relation_ref("editor"),
                    PermissionExpr::SubjectRelationRef {
                        relation: "viewer".into(),
                        subject_relation: "member".into(),
                    },
                    PermissionExpr::Arrow {
                        source_relation: "parent".into(),
                        target_permission: "read".into(),
                    },
                    caveated_relation("viewer"),
                ]),
            },
            PermissionDefinition {
                object_type: "document".into(),
                name: "conditional".into(),
                expression: caveated_relation("viewer"),
            },
            PermissionDefinition {
                object_type: "document".into(),
                name: "constrained".into(),
                expression: PermissionExpr::AllOf(vec![
                    relation_ref("editor"),
                    relation_ref("viewer"),
                ]),
            },
            PermissionDefinition {
                object_type: "document".into(),
                name: "direct".into(),
                expression: relation_ref("editor"),
            },
            PermissionDefinition {
                object_type: "document".into(),
                name: "group_access".into(),
                expression: PermissionExpr::SubjectRelationRef {
                    relation: "viewer".into(),
                    subject_relation: "member".into(),
                },
            },
            PermissionDefinition {
                object_type: "document".into(),
                name: "parent_access".into(),
                expression: PermissionExpr::Arrow {
                    source_relation: "parent".into(),
                    target_permission: "read".into(),
                },
            },
            PermissionDefinition {
                object_type: "folder".into(),
                name: "read".into(),
                expression: relation_ref("reader"),
            },
        ],
        caveats: vec![CaveatDefinition {
            name: CAVEAT_NAME.into(),
            parameters: vec![
                CaveatParameter {
                    source: CaveatInputSource::RelationshipContext,
                    name: "amount".into(),
                    value_type: CaveatValueType::Int,
                },
                CaveatParameter {
                    source: CaveatInputSource::CapabilityClaim,
                    name: "max_amount".into(),
                    value_type: CaveatValueType::Int,
                },
                CaveatParameter {
                    source: CaveatInputSource::CapabilityClaim,
                    name: "purpose".into(),
                    value_type: CaveatValueType::String,
                },
            ],
            expression: CaveatExpr::And(vec![
                CaveatExpr::Equal(
                    Box::new(CaveatExpr::InputRef {
                        source: CaveatInputSource::CapabilityClaim,
                        name: "purpose".into(),
                        value_type: CaveatValueType::String,
                    }),
                    Box::new(CaveatExpr::ConstString("reconciliation".into())),
                ),
                CaveatExpr::LessThanOrEqual(
                    Box::new(CaveatExpr::InputRef {
                        source: CaveatInputSource::RelationshipContext,
                        name: "amount".into(),
                        value_type: CaveatValueType::Int,
                    }),
                    Box::new(CaveatExpr::InputRef {
                        source: CaveatInputSource::CapabilityClaim,
                        name: "max_amount".into(),
                        value_type: CaveatValueType::Int,
                    }),
                ),
            ]),
        }],
        limits: limits(),
    }
}

pub fn all_caveat_nodes_program() -> PolicyProgram {
    let mut program = base_program();
    program.caveats[0].parameters.clear();
    program.caveats[0].expression = CaveatExpr::And(vec![
        CaveatExpr::ConstBool(true),
        CaveatExpr::Equal(
            Box::new(CaveatExpr::ConstString("a".into())),
            Box::new(CaveatExpr::ConstString("a".into())),
        ),
        CaveatExpr::NotEqual(
            Box::new(CaveatExpr::ConstInt(1)),
            Box::new(CaveatExpr::ConstInt(2)),
        ),
        CaveatExpr::LessThan(
            Box::new(CaveatExpr::ConstInt(1)),
            Box::new(CaveatExpr::ConstInt(2)),
        ),
        CaveatExpr::LessThanOrEqual(
            Box::new(CaveatExpr::ConstInt(2)),
            Box::new(CaveatExpr::ConstInt(2)),
        ),
        CaveatExpr::GreaterThan(
            Box::new(CaveatExpr::ConstInt(2)),
            Box::new(CaveatExpr::ConstInt(1)),
        ),
        CaveatExpr::GreaterThanOrEqual(
            Box::new(CaveatExpr::ConstInt(2)),
            Box::new(CaveatExpr::ConstInt(2)),
        ),
        CaveatExpr::Or(vec![
            CaveatExpr::ConstBool(false),
            CaveatExpr::Equal(
                Box::new(CaveatExpr::ConstInt(1)),
                Box::new(CaveatExpr::ConstInt(1)),
            ),
        ]),
        CaveatExpr::Not(Box::new(CaveatExpr::ConstBool(false))),
    ]);
    program
}

pub fn direct_allowed(object_type: &str) -> AllowedSubjectType {
    AllowedSubjectType {
        object_type: object_type.into(),
        subject_relation: None,
        caveat_name: None,
    }
}

pub fn relation_ref(name: &str) -> PermissionExpr {
    PermissionExpr::RelationRef { name: name.into() }
}

pub fn caveated_relation(relation: &str) -> PermissionExpr {
    PermissionExpr::CaveatedRelation {
        relation: relation.into(),
        caveat_name: CAVEAT_NAME.into(),
    }
}

pub fn object(object_type: &str, object_id: &str) -> ObjectRef {
    ObjectRef {
        object_type: object_type.into(),
        object_id: object_id.into(),
    }
}

pub fn relationship(subject: ObjectRef) -> ResolvedRelationship {
    ResolvedRelationship {
        subject,
        subject_relation: None,
        caveat: None,
        witness_id: None,
    }
}

pub fn caveated_relationship(subject: ObjectRef, amount: i64) -> ResolvedRelationship {
    ResolvedRelationship {
        subject,
        subject_relation: None,
        caveat: Some(RelationshipCaveat {
            name: CAVEAT_NAME.into(),
            context: vec![ContextEntry {
                name: "amount".into(),
                value: CaveatValue::Int(amount),
            }],
        }),
        witness_id: None,
    }
}

#[derive(Default)]
pub struct Resolver {
    relationships: BTreeMap<(ObjectRef, String), Vec<ResolvedRelationship>>,
}

impl Resolver {
    pub fn insert(
        &mut self,
        resource: ObjectRef,
        relation: &str,
        relationships: Vec<ResolvedRelationship>,
    ) {
        self.relationships
            .insert((resource, relation.into()), relationships);
    }
}

impl RelationshipResolver for Resolver {
    fn resolve<'a>(
        &'a self,
        resource: &ObjectRef,
        relation: &str,
    ) -> Result<&'a [ResolvedRelationship], ResolverError> {
        Ok(self
            .relationships
            .get(&(resource.clone(), relation.to_owned()))
            .map(Vec::as_slice)
            .unwrap_or(&[]))
    }
}

pub fn to_proto(program: &PolicyProgram) -> proto::PolicyProgram {
    proto::PolicyProgram {
        ir_version: program.ir_version,
        compiler_name: program.compiler_name.clone(),
        compiler_version: program.compiler_version.clone(),
        source_schema_digest: program.source_schema_digest.to_vec(),
        definitions: program
            .definitions
            .iter()
            .map(definition_to_proto)
            .collect(),
        permissions: program
            .permissions
            .iter()
            .map(permission_to_proto)
            .collect(),
        caveats: program.caveats.iter().map(caveat_to_proto).collect(),
        limits: Some(limits_to_proto(program.limits)),
    }
}

fn definition_to_proto(value: &ObjectDefinition) -> proto::ObjectDefinition {
    proto::ObjectDefinition {
        name: value.name.clone(),
        relations: value
            .relations
            .iter()
            .map(|relation| proto::RelationDefinition {
                name: relation.name.clone(),
                allowed_subject_types: relation
                    .allowed_subject_types
                    .iter()
                    .map(|allowed| proto::AllowedSubjectType {
                        object_type: allowed.object_type.clone(),
                        subject_relation: allowed.subject_relation.clone(),
                        caveat_name: allowed.caveat_name.clone(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn permission_to_proto(value: &PermissionDefinition) -> proto::PermissionDefinition {
    proto::PermissionDefinition {
        object_type: value.object_type.clone(),
        name: value.name.clone(),
        expression: Some(permission_expr_to_proto(&value.expression)),
    }
}

fn permission_expr_to_proto(value: &PermissionExpr) -> proto::PermissionExpr {
    use proto::permission_expr::Node;
    let node = match value {
        PermissionExpr::RelationRef { name } => {
            Node::RelationRef(proto::PermissionRelationRef { name: name.clone() })
        }
        PermissionExpr::SubjectRelationRef {
            relation,
            subject_relation,
        } => Node::SubjectRelationRef(proto::PermissionSubjectRelationRef {
            relation: relation.clone(),
            subject_relation: subject_relation.clone(),
        }),
        PermissionExpr::AnyOf(children) => Node::AnyOf(proto::PermissionChildren {
            children: children.iter().map(permission_expr_to_proto).collect(),
        }),
        PermissionExpr::AllOf(children) => Node::AllOf(proto::PermissionChildren {
            children: children.iter().map(permission_expr_to_proto).collect(),
        }),
        PermissionExpr::Arrow {
            source_relation,
            target_permission,
        } => Node::Arrow(proto::PermissionArrow {
            source_relation: source_relation.clone(),
            target_permission: target_permission.clone(),
        }),
        PermissionExpr::CaveatedRelation {
            relation,
            caveat_name,
        } => Node::CaveatedRelation(proto::PermissionCaveatedRelation {
            relation: relation.clone(),
            caveat_name: caveat_name.clone(),
        }),
    };
    proto::PermissionExpr { node: Some(node) }
}

fn caveat_to_proto(value: &CaveatDefinition) -> proto::CaveatDefinition {
    proto::CaveatDefinition {
        name: value.name.clone(),
        parameters: value
            .parameters
            .iter()
            .map(|parameter| proto::CaveatParameter {
                source: input_source_to_proto(parameter.source) as i32,
                name: parameter.name.clone(),
                value_type: value_type_to_proto(parameter.value_type) as i32,
            })
            .collect(),
        expression: Some(caveat_expr_to_proto(&value.expression)),
    }
}

fn caveat_expr_to_proto(value: &CaveatExpr) -> proto::CaveatExpr {
    use proto::caveat_expr::Node;
    let node = match value {
        CaveatExpr::ConstBool(value) => Node::ConstBool(*value),
        CaveatExpr::ConstInt(value) => Node::ConstInt(*value),
        CaveatExpr::ConstString(value) => Node::ConstString(value.clone()),
        CaveatExpr::InputRef {
            source,
            name,
            value_type,
        } => Node::InputRef(proto::CaveatInputRef {
            source: input_source_to_proto(*source) as i32,
            name: name.clone(),
            value_type: value_type_to_proto(*value_type) as i32,
        }),
        CaveatExpr::Equal(left, right) => Node::Equal(Box::new(binary_to_proto(left, right))),
        CaveatExpr::NotEqual(left, right) => Node::NotEqual(Box::new(binary_to_proto(left, right))),
        CaveatExpr::LessThan(left, right) => Node::LessThan(Box::new(binary_to_proto(left, right))),
        CaveatExpr::LessThanOrEqual(left, right) => {
            Node::LessThanOrEqual(Box::new(binary_to_proto(left, right)))
        }
        CaveatExpr::GreaterThan(left, right) => {
            Node::GreaterThan(Box::new(binary_to_proto(left, right)))
        }
        CaveatExpr::GreaterThanOrEqual(left, right) => {
            Node::GreaterThanOrEqual(Box::new(binary_to_proto(left, right)))
        }
        CaveatExpr::And(children) => Node::And(proto::CaveatChildren {
            children: children.iter().map(caveat_expr_to_proto).collect(),
        }),
        CaveatExpr::Or(children) => Node::Or(proto::CaveatChildren {
            children: children.iter().map(caveat_expr_to_proto).collect(),
        }),
        CaveatExpr::Not(child) => Node::Not(Box::new(proto::CaveatUnaryExpr {
            child: Some(Box::new(caveat_expr_to_proto(child))),
        })),
    };
    proto::CaveatExpr { node: Some(node) }
}

fn binary_to_proto(left: &CaveatExpr, right: &CaveatExpr) -> proto::CaveatBinaryExpr {
    proto::CaveatBinaryExpr {
        left: Some(Box::new(caveat_expr_to_proto(left))),
        right: Some(Box::new(caveat_expr_to_proto(right))),
    }
}

fn input_source_to_proto(value: CaveatInputSource) -> proto::CaveatInputSource {
    match value {
        CaveatInputSource::RequestContext => proto::CaveatInputSource::RequestContext,
        CaveatInputSource::RelationshipContext => proto::CaveatInputSource::RelationshipContext,
        CaveatInputSource::CapabilityClaim => proto::CaveatInputSource::CapabilityClaim,
    }
}

fn value_type_to_proto(value: CaveatValueType) -> proto::CaveatValueType {
    match value {
        CaveatValueType::Bool => proto::CaveatValueType::Bool,
        CaveatValueType::Int => proto::CaveatValueType::Int,
        CaveatValueType::String => proto::CaveatValueType::String,
    }
}

fn limits_to_proto(value: PolicyLimits) -> proto::PolicyLimits {
    proto::PolicyLimits {
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
