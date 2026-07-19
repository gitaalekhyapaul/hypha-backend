//! Versioned PolicyIR validation and bounded authorization evaluation.
//!
//! This crate owns proof-critical policy semantics. It does not own relationship
//! storage, witness construction, Merkle verification, or canonical commitments.

#![forbid(unsafe_code)]

mod error;
mod evaluation;
mod model;
mod validation;
mod wire;

pub use error::{
    CollectionKind, EvaluationError, LimitKind, PolicyError, ReferenceKind, ResolverError,
};
pub use evaluation::{RelationshipResolver, evaluate};
pub use model::{
    AllowedSubjectType, CaveatDefinition, CaveatExpr, CaveatInputSource, CaveatParameter,
    CaveatValue, CaveatValueType, ContextEntry, EvaluationRequest, ObjectDefinition, ObjectRef,
    PermissionDefinition, PermissionExpr, PolicyLimits, PolicyProgram, RelationDefinition,
    RelationshipCaveat, ResolvedRelationship, ValidationCeilings,
};
pub use validation::ValidatedPolicyProgram;

pub const SUPPORTED_POLICY_IR_VERSION: u32 = 1;
