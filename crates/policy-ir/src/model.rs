#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyProgram {
    pub ir_version: u32,
    pub compiler_name: String,
    pub compiler_version: String,
    pub source_schema_digest: [u8; 32],
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

impl PermissionExpr {
    pub const fn protocol_tag(&self) -> u8 {
        match self {
            Self::RelationRef { .. } => 0x01,
            Self::SubjectRelationRef { .. } => 0x02,
            Self::AnyOf(_) => 0x03,
            Self::AllOf(_) => 0x04,
            Self::Arrow { .. } => 0x05,
            Self::CaveatedRelation { .. } => 0x06,
        }
    }
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

impl CaveatInputSource {
    pub const fn protocol_tag(self) -> u8 {
        match self {
            Self::RequestContext => 0x01,
            Self::RelationshipContext => 0x02,
            Self::CapabilityClaim => 0x03,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CaveatValueType {
    Bool,
    Int,
    String,
}

impl CaveatValueType {
    pub const fn protocol_tag(self) -> u8 {
        match self {
            Self::Bool => 0x01,
            Self::Int => 0x02,
            Self::String => 0x03,
        }
    }
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

impl CaveatExpr {
    pub const fn protocol_tag(&self) -> u8 {
        match self {
            Self::ConstBool(_) => 0x01,
            Self::ConstInt(_) => 0x02,
            Self::ConstString(_) => 0x03,
            Self::InputRef { .. } => 0x04,
            Self::Equal(_, _) => 0x05,
            Self::NotEqual(_, _) => 0x06,
            Self::LessThan(_, _) => 0x07,
            Self::LessThanOrEqual(_, _) => 0x08,
            Self::GreaterThan(_, _) => 0x09,
            Self::GreaterThanOrEqual(_, _) => 0x0a,
            Self::And(_) => 0x0b,
            Self::Or(_) => 0x0c,
            Self::Not(_) => 0x0d,
        }
    }
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
