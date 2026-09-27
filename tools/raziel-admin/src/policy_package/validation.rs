use std::collections::BTreeMap;

use prost::Message;
use raziel_authorization_core::TRUSTED_POLICY_CEILINGS;
use raziel_canonical::policy_hash;
use raziel_policy_ir::{
    CaveatInputSource, PolicyLimits, ValidatedPolicyProgram, decode_policy_program_strict,
};
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, Visitor},
};
use sha2::{Digest, Sha256};

use crate::config::{MAX_ARTIFACT_LIFETIME_SECONDS, MAX_RECORD_BYTES};
use crate::policy_authority::PolicyCandidateManifestWire;

use super::{
    MAX_COMPILER_CONFIG_BYTES, MAX_SOURCE_BYTES, PackageError, PolicyInventory, PolicyPackageInput,
};

pub(super) struct ValidatedArtifacts {
    pub manifest: PolicyCandidateManifestWire,
    pub policy: ValidatedPolicyProgram,
    pub inventory: PolicyInventory,
    pub policy_hash: [u8; 32],
}

pub(super) fn artifacts(
    input: &PolicyPackageInput<'_>,
) -> Result<ValidatedArtifacts, PackageError> {
    if input.candidate_manifest.len() as u64 > MAX_RECORD_BYTES
        || input.policy_pb.len() as u64 > MAX_RECORD_BYTES
        || input.source_schema.is_empty()
        || input.source_schema.len() > MAX_SOURCE_BYTES
        || input.source_schema.contains(&0)
        || std::str::from_utf8(input.source_schema).is_err()
        || input.compiler_config.len() > MAX_COMPILER_CONFIG_BYTES
    {
        return Err(PackageError::InvalidPackage);
    }
    let manifest: PolicyCandidateManifestWire = canonical(input.candidate_manifest)?;
    let policy =
        decode_policy_program_strict(input.policy_pb, MAX_RECORD_BYTES, &TRUSTED_POLICY_CEILINGS)
            .map_err(|_| PackageError::InvalidPackage)?;
    let program = policy.program();
    let hash = policy_hash(&policy).map_err(|_| PackageError::InvalidPackage)?;
    let source_digest: [u8; 32] = Sha256::digest(input.source_schema).into();
    if manifest.format_version != 1
        || !public_identifier(&manifest.candidate_id)
        || !public_identifier(&manifest.record_id)
        || manifest.policy_name != input.identity.signed_policy_name()
        || manifest.policy_version != input.identity.version().to_string()
        || manifest.resource_type != input.identity.resource_type()
        || manifest.permission != "query"
        || manifest.rule_summary.is_empty()
        || manifest.rule_summary.len() > 512
        || !manifest.rule_summary.is_ascii()
        || manifest.rule_summary.bytes().any(|b| b.is_ascii_control())
        || manifest.source_schema_digest != source_digest
        || program.source_schema_digest != source_digest
        || manifest.policy_hash != hash
        || manifest.expected_approval_epoch == 0
        || !window(
            manifest.valid_from_unix_seconds,
            manifest.valid_until_unix_seconds,
            MAX_ARTIFACT_LIFETIME_SECONDS,
        )
        || !window(
            manifest.created_at_unix_seconds,
            manifest.expires_at_unix_seconds,
            86_400,
        )
        || !program
            .permissions
            .iter()
            .any(|p| p.object_type == manifest.resource_type && p.name == manifest.permission)
        || program.compiler_name != "authzed/spicedb"
        || Some(program.compiler_version.as_str())
            != include_str!("../../../toolchain-versions.env")
                .lines()
                .find_map(|line| line.strip_prefix("SPICEDB_VERSION="))
    {
        return Err(PackageError::InvalidPackage);
    }
    let inventory = inventory(&policy, input.identity.version_id())?;
    compiler_config(input.compiler_config, &policy)?;
    Ok(ValidatedArtifacts {
        manifest,
        policy,
        inventory,
        policy_hash: hash,
    })
}

fn inventory(policy: &ValidatedPolicyProgram, id: &str) -> Result<PolicyInventory, PackageError> {
    let program = policy.program();
    let owned = [
        format!("dataset_{id}"),
        format!("team_{id}"),
        format!("organization_{id}"),
    ];
    if !program
        .definitions
        .iter()
        .any(|d| d.name == "agent" && d.relations.is_empty())
        || !program.definitions.iter().any(|d| d.name == owned[0])
        || program
            .definitions
            .iter()
            .any(|d| d.name != "agent" && !owned.contains(&d.name))
        || program.permissions.iter().any(|p| p.object_type == "agent")
    {
        return Err(PackageError::NamespaceRejected);
    }
    let caveat_prefix = format!("caveat_{id}_");
    for caveat in &program.caveats {
        let index = caveat.name.strip_prefix(&caveat_prefix).and_then(|suffix| {
            suffix
                .parse::<u32>()
                .ok()
                .filter(|n| n.to_string() == suffix)
        });
        if caveat.name.len() > 63 || index.is_none_or(|n| n >= TRUSTED_POLICY_CEILINGS.max_caveats)
        {
            return Err(PackageError::NamespaceRejected);
        }
    }
    // Strict PolicyIR validation already resolves every relation, subject-set,
    // arrow, and caveat reference. All of its definitions are owned above.
    Ok(PolicyInventory {
        definitions: program.definitions.iter().map(|d| d.name.clone()).collect(),
        caveats: program.caveats.iter().map(|c| c.name.clone()).collect(),
    })
}

pub(super) fn canonical<M: Message + Default>(bytes: &[u8]) -> Result<M, PackageError> {
    let value = M::decode(bytes).map_err(|_| PackageError::InvalidPackage)?;
    if value.encode_to_vec() != bytes {
        return Err(PackageError::InvalidPackage);
    }
    Ok(value)
}

pub(super) fn immutable_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn public_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.bytes().enumerate().all(|(i, b)| {
            b.is_ascii_alphanumeric() || (i > 0 && matches!(b, b'.' | b'_' | b':' | b'-'))
        })
}

fn window(start: i64, end: i64, max: u64) -> bool {
    end.checked_sub(start)
        .and_then(|n| u64::try_from(n).ok())
        .is_some_and(|n| n > 0 && n <= max)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CompilerConfig {
    config_version: u32,
    limits: CompilerLimits,
    caveats: UniqueMap<UniqueMap<ParameterSource>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CompilerLimits {
    max_expression_nodes: u32,
    max_caveat_nodes: u32,
    max_path_depth: u32,
    max_relationship_witnesses: u32,
    max_merkle_depth: u32,
    max_string_bytes: u32,
    max_context_entries: u32,
    max_total_private_input_bytes: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ParameterSource {
    CapabilityClaim,
    RelationshipContext,
    RequestContext,
}

struct UniqueMap<V>(BTreeMap<String, V>);

impl<'de, V: Deserialize<'de>> Deserialize<'de> for UniqueMap<V> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MapVisitor<V>(std::marker::PhantomData<V>);
        impl<'de, V: Deserialize<'de>> Visitor<'de> for MapVisitor<V> {
            type Value = UniqueMap<V>;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("unique map keys")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut output = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<String, V>()? {
                    if output.insert(key, value).is_some() {
                        return Err(de::Error::custom("duplicate map key"));
                    }
                }
                Ok(UniqueMap(output))
            }
        }
        deserializer.deserialize_map(MapVisitor(std::marker::PhantomData))
    }
}

fn compiler_config(bytes: &[u8], policy: &ValidatedPolicyProgram) -> Result<(), PackageError> {
    let config: CompilerConfig =
        serde_json::from_slice(bytes).map_err(|_| PackageError::InvalidPackage)?;
    let l = config.limits;
    let limits = PolicyLimits {
        max_expression_nodes: l.max_expression_nodes,
        max_caveat_nodes: l.max_caveat_nodes,
        max_path_depth: l.max_path_depth,
        max_relationship_witnesses: l.max_relationship_witnesses,
        max_merkle_depth: l.max_merkle_depth,
        max_string_bytes: l.max_string_bytes,
        max_context_entries: l.max_context_entries,
        max_total_private_input_bytes: l.max_total_private_input_bytes,
    };
    let program = policy.program();
    if config.config_version != 1
        || limits != program.limits
        || config.caveats.0.len() != program.caveats.len()
    {
        return Err(PackageError::InvalidPackage);
    }
    for caveat in &program.caveats {
        let parameters = &config
            .caveats
            .0
            .get(&caveat.name)
            .ok_or(PackageError::InvalidPackage)?
            .0;
        if parameters.len() != caveat.parameters.len() {
            return Err(PackageError::InvalidPackage);
        }
        for parameter in &caveat.parameters {
            let source = match parameters
                .get(&parameter.name)
                .ok_or(PackageError::InvalidPackage)?
            {
                ParameterSource::CapabilityClaim => CaveatInputSource::CapabilityClaim,
                ParameterSource::RelationshipContext => CaveatInputSource::RelationshipContext,
                ParameterSource::RequestContext => CaveatInputSource::RequestContext,
            };
            if parameter.source != source {
                return Err(PackageError::InvalidPackage);
            }
        }
    }
    Ok(())
}
