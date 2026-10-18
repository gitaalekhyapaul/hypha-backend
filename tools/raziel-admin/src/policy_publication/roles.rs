//! Bounded operator-only role administration under the existing owner writer lock.

use std::collections::BTreeSet;
use std::fs::File;
use std::path::Path;
use std::time::Duration;

use raziel_policy_ir::PermissionExpr;
use serde::{Deserialize, Serialize};
use spicedb_rs_proto::authzed::api::v1 as api;

use super::compiler::Compiler;
use super::installation::Installation;
use super::provisioning::network::{GrantApi, GrantClient, Observed};
use super::{PolicyPublisher, files};
use crate::io::lowercase_hex;
use crate::policy_activation::store::StoreActivationError as Error;
use crate::policy_package::{ApprovedPolicyPackage, PolicyPackageKey, PolicyPackageStore};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Viewer,
    Approver,
    Auditor,
    Owner,
    TeamMember,
}

impl AgentRole {
    fn relation(self) -> &'static str {
        match self {
            Self::Viewer => "viewer",
            Self::Approver => "approver",
            Self::Auditor => "auditor",
            Self::Owner => "owner",
            Self::TeamMember => "team_viewer",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleOperation {
    List,
    Enable,
    Add,
    Remove,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentRolesRequest {
    pub format_version: u32,
    pub operation: RoleOperation,
    pub policy_version_id: String,
    pub approval_epoch: String,
    pub dataset_id: String,
    pub agent_key_thumbprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<AgentRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
}

impl AgentRolesRequest {
    pub(crate) fn validate(&self) -> Result<u64, Error> {
        let epoch = self
            .approval_epoch
            .parse::<u64>()
            .map_err(|_| Error::InvalidRequest)?;
        if self.format_version != 1
            || !hexadecimal(&self.policy_version_id, 32)
            || !hexadecimal(&self.agent_key_thumbprint, 64)
            || !safe_id(&self.dataset_id)
            || epoch == 0
            || self.approval_epoch != epoch.to_string()
        {
            return Err(Error::InvalidRequest);
        }
        match self.operation {
            RoleOperation::List | RoleOperation::Enable => {
                if self.role.is_some() || self.team_id.is_some() {
                    return Err(Error::InvalidRequest);
                }
            }
            RoleOperation::Add | RoleOperation::Remove => match self.role {
                Some(AgentRole::TeamMember) if self.team_id.as_deref().is_some_and(safe_id) => {}
                Some(role) if role != AgentRole::TeamMember && self.team_id.is_none() => {}
                _ => return Err(Error::InvalidRequest),
            },
        }
        Ok(epoch)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentRoleAssignment {
    pub role: AgentRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentRolesProjection {
    pub format_version: u32,
    pub policy_version_id: String,
    pub approval_epoch: String,
    pub dataset_id: String,
    pub agent_key_thumbprint: String,
    pub managed: bool,
    pub can_enable: bool,
    pub available_roles: Vec<AgentRole>,
    pub assignments: Vec<AgentRoleAssignment>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManagedMode {
    format_version: u32,
    version_id: String,
    policy_hash: String,
}

pub(super) fn is_managed(root: &Path, package: &ApprovedPolicyPackage) -> Result<bool, Error> {
    let directory = root.join("managed-roles");
    match std::fs::symlink_metadata(&directory) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(Error::Unavailable),
        Ok(_) => {
            files::private_directory(&directory)?;
        }
    }
    let version = package.identity().version_id();
    let name = format!("{version}.json");
    match std::fs::symlink_metadata(directory.join(&name)) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(Error::Unavailable),
        Ok(_) => {}
    }
    let mode: ManagedMode = serde_json::from_slice(&files::read(&directory, &name, 1024)?)
        .map_err(|_| Error::Unavailable)?;
    if mode.format_version != 1
        || mode.version_id != version
        || mode.policy_hash != lowercase_hex(package.policy_hash())
    {
        return Err(Error::Unavailable);
    }
    Ok(true)
}

fn enable(root: &Path, package: &ApprovedPolicyPackage) -> Result<(), Error> {
    let directory = root.join("managed-roles");
    files::create_private_directory(&directory)?;
    let mode = ManagedMode {
        format_version: 1,
        version_id: package.identity().version_id().into(),
        policy_hash: lowercase_hex(package.policy_hash()),
    };
    files::write_new(
        &directory,
        &format!("{}.json", mode.version_id),
        &serde_json::to_vec(&mode).map_err(|_| Error::Unavailable)?,
    )?;
    crate::io::sync_directory(root).map_err(|_| Error::Unavailable)
}

impl PolicyPublisher {
    pub(crate) fn agent_roles(
        &self,
        request: AgentRolesRequest,
    ) -> Result<AgentRolesProjection, Error> {
        self.agent_roles_at(request, crate::policy_activation::store::trusted_now()?)
    }

    pub(super) fn agent_roles_at(
        &self,
        request: AgentRolesRequest,
        now: i64,
    ) -> Result<AgentRolesProjection, Error> {
        let epoch = request.validate()?;
        let c = &self.config;
        let lock = File::open(&c.root).map_err(|_| Error::Unavailable)?;
        lock.try_lock().map_err(|_| Error::Unavailable)?;
        let compiler = Compiler::new(&c.compiler, &c.root)?;
        let mut packages = PolicyPackageStore::new(
            c.package_root.clone(),
            c.registry_root.clone(),
            &c.trust_domain_id,
        )
        .map_err(|_| Error::Unavailable)?;
        let installed = Installation::load_for_request(
            &c.root,
            &c.registry_root,
            &c.trust_domain_id,
            &mut packages,
            &compiler,
            now,
        )?;
        let key = PolicyPackageKey::new(request.policy_version_id.clone(), epoch)
            .map_err(|_| Error::InvalidRequest)?;
        if !installed.contains(&key) {
            return Err(Error::Conflict);
        }
        let package = packages
            .read_approved(&key, now)
            .map_err(|_| Error::Conflict)?;
        let roles = available_roles(&package).unwrap_or_default();
        let mut managed = is_managed(&c.root, &package)?;
        let types = package
            .policy()
            .program()
            .definitions
            .iter()
            .map(|d| d.name.clone())
            .collect::<Vec<_>>();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Error::Unavailable)?;
        runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(90), async {
                let mut client = GrantClient::connect(&c.endpoint, &c.token_path)
                    .await
                    .map_err(|_| Error::Unavailable)?;
                installed.check_live(&mut client, &compiler).await?;
                let mut observed = client
                    .snapshot(&types)
                    .await
                    .map_err(|_| Error::Unavailable)?;
                let can_enable = !managed && !roles.is_empty() && observed.relationships.is_empty();
                match request.operation {
                    RoleOperation::List => {}
                    RoleOperation::Enable => {
                        if !managed {
                            if !can_enable {
                                return Err(Error::Conflict);
                            }
                            enable(&c.root, &package)?;
                            managed = true;
                        }
                    }
                    RoleOperation::Add | RoleOperation::Remove => {
                        if !managed || !request.role.is_some_and(|r| roles.contains(&r)) {
                            return Err(Error::Conflict);
                        }
                        let updates = role_updates(&request)?;
                        let missing = updates
                            .iter()
                            .filter(|u| {
                                !observed
                                    .relationships
                                    .iter()
                                    .any(|r| Some(r) == u.relationship.as_ref())
                            })
                            .count();
                        if request.operation == RoleOperation::Add
                            && observed.relationships.len() + missing > 256
                        {
                            return Err(Error::InvalidRequest);
                        }
                        client
                            .touch(&updates)
                            .await
                            .map_err(|_| Error::Unavailable)?;
                        observed = client
                            .snapshot(&types)
                            .await
                            .map_err(|_| Error::Unavailable)?;
                    }
                }
                Ok(projection(
                    &request,
                    managed,
                    can_enable && !managed,
                    roles,
                    &observed,
                ))
            })
            .await
            .map_err(|_| Error::Unavailable)?
        })
    }
}

fn available_roles(package: &ApprovedPolicyPackage) -> Result<Vec<AgentRole>, Error> {
    let resource = format!("dataset_{}", package.identity().version_id());
    let permission = package
        .policy()
        .program()
        .permissions
        .iter()
        .find(|p| p.object_type == resource && p.name == "query")
        .ok_or(Error::InvalidRequest)?;
    let mut roles = BTreeSet::new();
    collect_roles(&permission.expression, &mut roles)?;
    Ok(roles.into_iter().collect())
}

fn collect_roles(
    expression: &PermissionExpr,
    roles: &mut BTreeSet<AgentRole>,
) -> Result<(), Error> {
    match expression {
        PermissionExpr::AnyOf(children) | PermissionExpr::AllOf(children) => {
            for child in children {
                collect_roles(child, roles)?;
            }
        }
        PermissionExpr::RelationRef { name } => {
            let role = match name.as_str() {
                "viewer" => AgentRole::Viewer,
                "approver" => AgentRole::Approver,
                "auditor" => AgentRole::Auditor,
                "owner" => AgentRole::Owner,
                _ => return Err(Error::InvalidRequest),
            };
            roles.insert(role);
        }
        PermissionExpr::SubjectRelationRef {
            relation,
            subject_relation,
        } if relation == "team_viewer" && subject_relation == "member" => {
            roles.insert(AgentRole::TeamMember);
        }
        _ => return Err(Error::InvalidRequest),
    }
    Ok(())
}

fn role_updates(request: &AgentRolesRequest) -> Result<Vec<api::RelationshipUpdate>, Error> {
    request.validate()?;
    let role = request.role.ok_or(Error::InvalidRequest)?;
    let operation = if request.operation == RoleOperation::Add {
        api::relationship_update::Operation::Touch
    } else {
        api::relationship_update::Operation::Delete
    } as i32;
    let dataset_type = format!("dataset_{}", request.policy_version_id);
    if role != AgentRole::TeamMember {
        return Ok(vec![update(
            operation,
            relation(
                &dataset_type,
                &request.dataset_id,
                role.relation(),
                "agent",
                &request.agent_key_thumbprint,
                "",
            ),
        )]);
    }
    let team_type = format!("team_{}", request.policy_version_id);
    let team = request.team_id.as_deref().ok_or(Error::InvalidRequest)?;
    let mut updates = Vec::new();
    if request.operation == RoleOperation::Add {
        updates.push(update(
            operation,
            relation(
                &dataset_type,
                &request.dataset_id,
                "team_viewer",
                &team_type,
                team,
                "member",
            ),
        ));
    }
    updates.push(update(
        operation,
        relation(
            &team_type,
            team,
            "member",
            "agent",
            &request.agent_key_thumbprint,
            "",
        ),
    ));
    Ok(updates)
}

fn relation(
    resource_type: &str,
    resource_id: &str,
    relation: &str,
    subject_type: &str,
    subject_id: &str,
    subject_relation: &str,
) -> api::Relationship {
    api::Relationship {
        resource: Some(api::ObjectReference {
            object_type: resource_type.into(),
            object_id: resource_id.into(),
        }),
        relation: relation.into(),
        subject: Some(api::SubjectReference {
            object: Some(api::ObjectReference {
                object_type: subject_type.into(),
                object_id: subject_id.into(),
            }),
            optional_relation: subject_relation.into(),
        }),
        optional_caveat: None,
        optional_expires_at: None,
    }
}

fn update(operation: i32, relationship: api::Relationship) -> api::RelationshipUpdate {
    api::RelationshipUpdate {
        operation,
        relationship: Some(relationship),
    }
}

fn projection(
    request: &AgentRolesRequest,
    managed: bool,
    can_enable: bool,
    available_roles: Vec<AgentRole>,
    observed: &Observed,
) -> AgentRolesProjection {
    let dataset_type = format!("dataset_{}", request.policy_version_id);
    let team_type = format!("team_{}", request.policy_version_id);
    let mut assignments = Vec::new();
    for role in &available_roles {
        if *role != AgentRole::TeamMember {
            let wanted = relation(
                &dataset_type,
                &request.dataset_id,
                role.relation(),
                "agent",
                &request.agent_key_thumbprint,
                "",
            );
            if observed.relationships.contains(&wanted) {
                assignments.push(AgentRoleAssignment {
                    role: *role,
                    team_id: None,
                });
            }
        } else {
            let teams = observed
                .relationships
                .iter()
                .filter_map(|r| {
                    let resource = r.resource.as_ref()?;
                    let subject = r.subject.as_ref()?;
                    let object = subject.object.as_ref()?;
                    (resource.object_type == dataset_type
                        && resource.object_id == request.dataset_id
                        && r.relation == "team_viewer"
                        && object.object_type == team_type
                        && subject.optional_relation == "member"
                        && r.optional_caveat.is_none())
                    .then_some(object.object_id.clone())
                })
                .collect::<BTreeSet<_>>();
            for team in teams {
                if observed.relationships.contains(&relation(
                    &team_type,
                    &team,
                    "member",
                    "agent",
                    &request.agent_key_thumbprint,
                    "",
                )) {
                    assignments.push(AgentRoleAssignment {
                        role: *role,
                        team_id: Some(team),
                    });
                }
            }
        }
    }
    AgentRolesProjection {
        format_version: 1,
        policy_version_id: request.policy_version_id.clone(),
        approval_epoch: request.approval_epoch.clone(),
        dataset_id: request.dataset_id.clone(),
        agent_key_thumbprint: request.agent_key_thumbprint.clone(),
        managed,
        can_enable,
        available_roles,
        assignments,
    }
}

fn hexadecimal(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
}

#[cfg(test)]
mod tests;
