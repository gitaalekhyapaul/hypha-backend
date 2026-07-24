//! Versioned PolicyIR validation and bounded authorization evaluation.
//!
//! This crate owns proof-critical policy semantics. It does not own relationship
//! storage, witness construction, Merkle verification, or canonical commitments.

#![forbid(unsafe_code)]

mod error;
mod evaluation;
#[cfg(feature = "strict-protobuf")]
mod ingress;
mod model;
mod validation;
#[cfg(feature = "wire")]
mod wire;

pub use error::{
    CollectionKind, EvaluationError, LimitKind, PolicyError, ReferenceKind, ResolverError,
};
pub use evaluation::{RelationshipResolver, evaluate, evaluate_with_usage};
#[cfg(feature = "strict-protobuf")]
pub use ingress::{PolicyIngressError, decode_policy_program_strict};
pub use model::{
    AllowedSubjectType, CaveatDefinition, CaveatExpr, CaveatInputSource, CaveatParameter,
    CaveatValue, CaveatValueType, ContextEntry, EvaluationOutcome, EvaluationRequest,
    ObjectDefinition, ObjectRef, PermissionDefinition, PermissionExpr, PolicyLimits, PolicyProgram,
    RelationDefinition, RelationshipCaveat, ResolvedRelationship, ValidationCeilings,
};
pub use validation::ValidatedPolicyProgram;

pub const SUPPORTED_POLICY_IR_VERSION: u32 = 1;
