#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyProgram {
    pub ir_version: u32,
    pub compiler_name: String,
    pub compiler_version: String,
    pub source_schema_digest: Vec<u8>,
    pub definitions: Vec<ObjectDefinition>,
    pub permissions: Vec<PermissionDefinition>,
    pub caveats: Vec<CaveatDefinition>,
    pub limits: PolicyLimits,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObjectDefinition {
    pub name: String,
    pub relations: Vec<RelationDefinition>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RelationDefinition {
    pub name: String,
    pub allowed_subject_types: Vec<AllowedSubjectType>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AllowedSubjectType {
    pub object_type: String,
    pub subject_relation: Option<String>,
    pub caveat_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PermissionDefinition {
    pub object_type: String,
    pub name: String,
    pub expression: PermissionExpr,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PermissionExpr {
    RelationRef {
        name: String,
    },
    SubjectRelationRef {
        relation: String,
        subject_relation: String,
    },
    AnyOf(Vec<PermissionExpr>),
    AllOf(Vec<PermissionExpr>),
    Arrow {
        source_relation: String,
        target_permission: String,
    },
    CaveatedRelation {
        relation: String,
        caveat_name: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CaveatDefinition {
    pub name: String,
    pub parameters: Vec<CaveatParameter>,
    pub expression: CaveatExpr,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CaveatParameter {
    pub source: CaveatInputSource,
    pub name: String,
    pub value_type: CaveatValueType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CaveatInputSource {
    RequestContext,
    RelationshipContext,
    CapabilityClaim,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CaveatValueType {
    Bool,
    Int,
    String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CaveatExpr {
    ConstBool(bool),
    ConstInt(i64),
    ConstString(String),
    InputRef {
        source: CaveatInputSource,
        name: String,
        value_type: CaveatValueType,
    },
    Equal(Box<CaveatExpr>, Box<CaveatExpr>),
    NotEqual(Box<CaveatExpr>, Box<CaveatExpr>),
    LessThan(Box<CaveatExpr>, Box<CaveatExpr>),
    LessThanOrEqual(Box<CaveatExpr>, Box<CaveatExpr>),
    GreaterThan(Box<CaveatExpr>, Box<CaveatExpr>),
    GreaterThanOrEqual(Box<CaveatExpr>, Box<CaveatExpr>),
    And(Vec<CaveatExpr>),
    Or(Vec<CaveatExpr>),
    Not(Box<CaveatExpr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolicyLimits {
    pub max_expression_nodes: u32,
    pub max_caveat_nodes: u32,
    pub max_path_depth: u32,
    pub max_relationship_witnesses: u32,
    pub max_merkle_depth: u32,
    pub max_string_bytes: u32,
    pub max_context_entries: u32,
    pub max_total_private_input_bytes: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidationCeilings {
    pub policy_limits: PolicyLimits,
    pub max_definitions: u32,
    pub max_relations_per_definition: u32,
    pub max_allowed_subject_types_per_relation: u32,
    pub max_permissions: u32,
    pub max_caveats: u32,
    pub max_parameters_per_caveat: u32,
    pub max_source_schema_digest_bytes: u32,
    pub max_syntax_depth: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObjectRef {
    pub object_type: String,
    pub object_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvaluationRequest {
    pub resource: ObjectRef,
    pub subject: ObjectRef,
    pub permission: String,
    pub request_context: Vec<ContextEntry>,
    pub capability_claims: Vec<ContextEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ContextEntry {
    pub name: String,
    pub value: CaveatValue,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CaveatValue {
    Bool(bool),
    Int(i64),
    String(String),
}

impl CaveatValue {
    pub(crate) const fn value_type(&self) -> CaveatValueType {
        match self {
            Self::Bool(_) => CaveatValueType::Bool,
            Self::Int(_) => CaveatValueType::Int,
            Self::String(_) => CaveatValueType::String,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RelationshipCaveat {
    pub name: String,
    pub context: Vec<ContextEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedRelationship {
    pub subject: ObjectRef,
    pub subject_relation: Option<String>,
    pub caveat: Option<RelationshipCaveat>,
    pub merkle_depth: u32,
    pub private_input_bytes: u32,
}
