use std::sync::{Arc, Mutex};

use prost::Message;
use raziel_proto::raziel::v1::{
    AccessRequest, AccessRequestState as State, ApproveAccessRequestRequest,
    ApproveAccessRequestResponse, CancelAccessRequestRequest, CancelAccessRequestResponse,
    CreateAccessRequestRequest, CreateAccessRequestResponse, GetAccessRequestRequest,
    GetAccessRequestResponse, ListAccessRequestsRequest, ListAccessRequestsResponse,
    RejectAccessRequestRequest, RejectAccessRequestResponse,
    access_request_service_server::{AccessRequestService, AccessRequestServiceServer},
};
use tonic::{Request, Response, Status};

use super::approval::ApprovalPreparer;
use super::model::{
    AccessError, MAX_INTENT_BYTES, NormalizedIntent, Phase, Record, identity, valid_identity,
    validate_agent_id,
};
use super::private::PrivateStore;
use super::redis_store::RedisStore;
use super::runtime::AccessServiceConfig;
use crate::service::{SystemClock, TrustedClock};

pub struct AccessCoordinator {
    pub(super) private: PrivateStore,
    pub(super) redis: RedisStore,
    pub(super) approval: Option<Box<dyn ApprovalPreparer>>,
}

impl AccessCoordinator {
    pub fn open(config: &AccessServiceConfig) -> Result<Self, AccessError> {
        let redis = RedisStore::new(config.redis_url(), config.prefix())?;
        redis.connect()?;
        let private = PrivateStore::open(config.private_root())?;
        let approval = config
            .approval()
            .cloned()
            .map(|config| Box::new(config) as Box<dyn ApprovalPreparer>);
        Ok(Self {
            private,
            redis,
            approval,
        })
    }

    pub fn create(
        &mut self,
        input: CreateAccessRequestRequest,
        now: i64,
    ) -> Result<CreateAccessRequestResponse, AccessError> {
        let input = NormalizedIntent::parse(input)?;
        let _lock = self.private.lock()?;
        let mut connection = self.redis.connect()?;
        let id = input.identity();
        let (mut record, raw, admitted_new) = match self.redis.get(&mut connection, &id)? {
            Some((record, raw)) => (record, raw, false),
            None => {
                if input.0.dataset.is_none() {
                    return Err(AccessError::NotFound);
                }
                self.reconcile_locked(&mut connection, now)?;
                let record = Record::reserved(&input, now)?;
                if self.redis.reserve(&mut connection, &record)? {
                    let raw = record.encode()?;
                    (record, raw, true)
                } else {
                    let (record, raw) = self
                        .redis
                        .get(&mut connection, &id)?
                        .ok_or(AccessError::ServiceUnavailable)?;
                    (record, raw, false)
                }
            }
        };
        match record.phase {
            Phase::Deleting => Err(AccessError::Expired),
            Phase::Published => {
                if !record.matches_intent(&input) {
                    return Err(AccessError::Conflict);
                }
                if !self.validated_intent(&record)?.matches_retry(&input) {
                    return Err(AccessError::Conflict);
                }
                if record.terminal()? && now >= record.due {
                    return Err(AccessError::Expired);
                }
                record = self.expire(&mut connection, record, raw, now)?;
                Ok(CreateAccessRequestResponse {
                    format_version: 1,
                    request: Some(record.public()),
                    created: false,
                })
            }
            Phase::Reserved => {
                // Recovery must compare private input, never reconstruct missing context.
                if !record.matches_intent(&input) {
                    return Err(AccessError::Conflict);
                }
                if now >= record.due {
                    return Err(AccessError::ServiceUnavailable);
                }
                if !admitted_new {
                    if !self
                        .private
                        .read(&record.private_ref)?
                        .matches_retry(&input)
                    {
                        return Err(AccessError::Conflict);
                    }
                } else {
                    self.private.persist(&record.private_ref, &input)?;
                }
                record.phase = Phase::Published;
                record.due = record.deadline;
                // Never delete the private file on an ambiguous Redis outcome.
                self.redis.cas(&mut connection, &id, &raw, Some(&record))?;
                Ok(CreateAccessRequestResponse {
                    format_version: 1,
                    request: Some(record.public()),
                    created: admitted_new,
                })
            }
        }
    }

    pub fn get(
        &mut self,
        agent: &str,
        request: &str,
        now: i64,
    ) -> Result<AccessRequest, AccessError> {
        let id = identity(agent, request)?;
        let _lock = self.private.lock()?;
        let mut connection = self.redis.connect()?;
        let (record, raw) = self.visible(&mut connection, &id, now)?;
        Ok(self.expire(&mut connection, record, raw, now)?.public())
    }

    pub fn list(
        &mut self,
        input: ListAccessRequestsRequest,
        now: i64,
    ) -> Result<ListAccessRequestsResponse, AccessError> {
        if let Some(owner) = &input.agent_id {
            validate_agent_id(owner)?;
        }
        if input.limit > 50 {
            return Err(AccessError::InvalidRequest);
        }
        let limit = if input.limit == 0 {
            50
        } else {
            input.limit as usize
        };
        let after = parse_cursor(input.agent_id.as_deref(), input.cursor.as_deref())?;
        let _lock = self.private.lock()?;
        let mut connection = self.redis.connect()?;
        self.reconcile_locked(&mut connection, now)?;
        let ids = self.redis.list(
            &mut connection,
            input.agent_id.as_deref(),
            after.as_deref(),
            limit + 1,
        )?;
        let has_more = ids.len() > limit;
        let mut requests = Vec::new();
        let mut last = None;
        for id in ids.into_iter().take(limit) {
            last = Some(id.clone());
            match self.visible(&mut connection, &id, now) {
                Ok((record, raw)) => {
                    requests.push(self.expire(&mut connection, record, raw, now)?.public())
                }
                Err(AccessError::NotFound | AccessError::Expired) => {}
                Err(e) => return Err(e),
            }
        }
        let next_cursor = if has_more {
            last.map(|id| cursor(input.agent_id.as_deref(), &id))
        } else {
            None
        };
        Ok(ListAccessRequestsResponse {
            format_version: 1,
            requests,
            next_cursor,
        })
    }

    pub fn reject(
        &mut self,
        agent: &str,
        request: &str,
        generation: u64,
        now: i64,
    ) -> Result<AccessRequest, AccessError> {
        self.mutate(agent, request, generation, now, State::Rejected)
    }

    pub fn cancel(
        &mut self,
        agent: &str,
        request: &str,
        generation: u64,
        now: i64,
    ) -> Result<AccessRequest, AccessError> {
        self.mutate(agent, request, generation, now, State::Canceled)
    }

    fn mutate(
        &mut self,
        agent: &str,
        request: &str,
        generation: u64,
        now: i64,
        target: State,
    ) -> Result<AccessRequest, AccessError> {
        let id = identity(agent, request)?;
        if generation == 0 {
            return Err(AccessError::InvalidRequest);
        }
        let _lock = self.private.lock()?;
        let mut connection = self.redis.connect()?;
        let (record, raw) = self.visible(&mut connection, &id, now)?;
        let mut record = self.expire(&mut connection, record, raw, now)?;
        // The retained command result wins over its now-stale pre-transition generation.
        if record.state()? == target {
            return Ok(record.public());
        }
        if record.state()? == State::Expired {
            return Err(AccessError::Expired);
        }
        if record.terminal()? || generation != record.generation {
            return Err(AccessError::Conflict);
        }
        if !record.predispatch()? {
            return Err(AccessError::ServiceUnavailable);
        }
        if target == State::Rejected && record.state()? != State::PendingReview {
            return Err(AccessError::Conflict);
        }
        let raw = record.encode()?;
        record.finish(target, now)?;
        self.redis.cas(&mut connection, &id, &raw, Some(&record))?;
        Ok(record.public())
    }

    pub(super) fn visible(
        &self,
        connection: &mut redis::Connection,
        id: &str,
        now: i64,
    ) -> Result<(Record, String), AccessError> {
        let (record, raw) = self
            .redis
            .get(connection, id)?
            .ok_or(AccessError::NotFound)?;
        match record.phase {
            Phase::Reserved => return Err(AccessError::NotFound),
            Phase::Deleting => return Err(AccessError::Expired),
            Phase::Published => {}
        }
        if record.terminal()? && now >= record.due {
            return Err(AccessError::Expired);
        }
        // Missing comparison state is never replaced by a public-only success.
        self.validated_intent(&record)?;
        Ok((record, raw))
    }

    fn validated_intent(&self, record: &Record) -> Result<NormalizedIntent, AccessError> {
        let private = self.private.read(&record.private_ref)?;
        if !record.matches_intent(&private) {
            return Err(AccessError::ServiceUnavailable);
        }
        if record.policy_binding.is_some() {
            let approval = self
                .private
                .read_approval(&record.private_ref, &private)?
                .ok_or(AccessError::ServiceUnavailable)?;
            if !approval.matches_published(record) {
                return Err(AccessError::ServiceUnavailable);
            }
        }
        Ok(private)
    }

    pub(super) fn expire(
        &self,
        connection: &mut redis::Connection,
        mut record: Record,
        raw: String,
        now: i64,
    ) -> Result<Record, AccessError> {
        if now < record.updated {
            return Err(AccessError::ServiceUnavailable);
        }
        if !record.terminal()? && now >= record.deadline {
            if !record.predispatch()? {
                return Err(AccessError::ServiceUnavailable);
            }
            let deadline = record.deadline;
            record.finish(State::Expired, deadline)?;
            self.redis
                .cas(connection, &record.identity(), &raw, Some(&record))?;
        }
        if record.terminal()? && now >= record.due {
            return Err(AccessError::Expired);
        }
        Ok(record)
    }

    /// One bounded maintenance batch. A timer repeats it even without API traffic.
    pub fn reconcile(&mut self, now: i64) -> Result<(), AccessError> {
        let _lock = self.private.lock()?;
        let mut connection = self.redis.connect()?;
        self.reconcile_locked(&mut connection, now)
    }

    fn reconcile_locked(
        &self,
        connection: &mut redis::Connection,
        now: i64,
    ) -> Result<(), AccessError> {
        for id in self.redis.due(connection, now)? {
            let Some((mut record, raw)) = self.redis.get(connection, &id)? else {
                return Err(AccessError::ServiceUnavailable);
            };
            if record.due > now {
                continue;
            }
            if record.phase == Phase::Published && !record.terminal()? {
                self.validated_intent(&record)?;
                // Later workflow states belong to B-0066, never infer proof termination.
                if record.predispatch()? {
                    match self.expire(connection, record, raw, now) {
                        Ok(_) | Err(AccessError::Expired) => {}
                        Err(error) => return Err(error),
                    }
                }
                continue;
            }
            if record.phase != Phase::Deleting {
                record.phase = Phase::Deleting;
                self.redis.cas(connection, &id, &raw, Some(&record))?;
            }
            // A durable deleting marker fences all publishers before filesystem cleanup.
            self.private.delete(&record.private_ref)?;
            self.redis.cas(connection, &id, &record.encode()?, None)?;
        }
        Ok(())
    }
}

fn cursor(owner: Option<&str>, id: &str) -> String {
    let value = format!("v1|{}|{id}", owner.unwrap_or("operator"));
    value.bytes().map(|b| format!("{b:02x}")).collect()
}

fn parse_cursor(owner: Option<&str>, cursor: Option<&str>) -> Result<Option<String>, AccessError> {
    let Some(encoded) = cursor else {
        return Ok(None);
    };
    if encoded.len() > 256 || encoded.len() % 2 != 0 || encoded.is_empty() {
        return Err(AccessError::InvalidRequest);
    }
    let bytes = encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|p| {
            let text = std::str::from_utf8(p).map_err(|_| AccessError::InvalidRequest)?;
            u8::from_str_radix(text, 16).map_err(|_| AccessError::InvalidRequest)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let decoded = String::from_utf8(bytes).map_err(|_| AccessError::InvalidRequest)?;
    let scope = format!("v1|{}|", owner.unwrap_or("operator"));
    let id = decoded
        .strip_prefix(&scope)
        .ok_or(AccessError::InvalidRequest)?;
    if !valid_identity(id) || owner.is_some_and(|o| !id.starts_with(&format!("{o}:"))) {
        return Err(AccessError::InvalidRequest);
    }
    Ok(Some(id.to_owned()))
}

#[derive(Clone)]
pub struct AccessRequestHandler {
    coordinator: Arc<Mutex<AccessCoordinator>>,
    clock: Arc<dyn TrustedClock>,
}

impl AccessRequestHandler {
    pub fn new(coordinator: AccessCoordinator) -> Self {
        Self::with_clock(coordinator, SystemClock)
    }

    pub fn with_clock(coordinator: AccessCoordinator, clock: impl TrustedClock) -> Self {
        Self {
            coordinator: Arc::new(Mutex::new(coordinator)),
            clock: Arc::new(clock),
        }
    }

    async fn call<T: Send + 'static>(
        &self,
        action: impl FnOnce(&mut AccessCoordinator, i64) -> Result<T, AccessError> + Send + 'static,
    ) -> Result<T, Status> {
        let coordinator = self.coordinator.clone();
        let clock = self.clock.clone();
        tokio::task::spawn_blocking(move || {
            let mut coordinator = coordinator
                .lock()
                .map_err(|_| AccessError::ServiceUnavailable)?;
            let now = clock
                .now_unix_seconds()
                .map_err(|_| AccessError::ServiceUnavailable)?;
            action(&mut coordinator, now)
        })
        .await
        .map_err(|_| Status::from(AccessError::ServiceUnavailable))?
        .map_err(Status::from)
    }

    pub async fn reconcile(&self) -> Result<(), Status> {
        self.call(|c, now| c.reconcile(now)).await
    }
}

pub fn bounded_access_service(
    handler: AccessRequestHandler,
) -> AccessRequestServiceServer<AccessRequestHandler> {
    AccessRequestServiceServer::new(handler)
        .max_decoding_message_size(MAX_INTENT_BYTES)
        .max_encoding_message_size(2 * 1024 * 1024)
}

#[tonic::async_trait]
impl AccessRequestService for AccessRequestHandler {
    async fn create_access_request(
        &self,
        request: Request<CreateAccessRequestRequest>,
    ) -> Result<Response<CreateAccessRequestResponse>, Status> {
        let input = request.into_inner();
        if input.encoded_len() > MAX_INTENT_BYTES {
            return Err(AccessError::InvalidRequest.into());
        }
        self.call(move |c, now| c.create(input, now))
            .await
            .map(Response::new)
    }
    async fn get_access_request(
        &self,
        request: Request<GetAccessRequestRequest>,
    ) -> Result<Response<GetAccessRequestResponse>, Status> {
        let input = request.into_inner();
        self.call(move |c, now| {
            c.get(&input.agent_id, &input.request_id, now)
                .map(|request| GetAccessRequestResponse {
                    request: Some(request),
                })
        })
        .await
        .map(Response::new)
    }
    async fn list_access_requests(
        &self,
        request: Request<ListAccessRequestsRequest>,
    ) -> Result<Response<ListAccessRequestsResponse>, Status> {
        let input = request.into_inner();
        self.call(move |c, now| c.list(input, now))
            .await
            .map(Response::new)
    }
    async fn approve_access_request(
        &self,
        request: Request<ApproveAccessRequestRequest>,
    ) -> Result<Response<ApproveAccessRequestResponse>, Status> {
        let input = request.into_inner();
        if input.encoded_len() > MAX_INTENT_BYTES {
            return Err(AccessError::InvalidRequest.into());
        }
        self.call(move |c, now| {
            c.approve(input, now)
                .map(|request| ApproveAccessRequestResponse {
                    request: Some(request),
                })
        })
        .await
        .map(Response::new)
    }
    async fn reject_access_request(
        &self,
        request: Request<RejectAccessRequestRequest>,
    ) -> Result<Response<RejectAccessRequestResponse>, Status> {
        let input = request.into_inner();
        self.call(move |c, now| {
            c.reject(
                &input.agent_id,
                &input.request_id,
                input.expected_generation,
                now,
            )
            .map(|request| RejectAccessRequestResponse {
                request: Some(request),
            })
        })
        .await
        .map(Response::new)
    }
    async fn cancel_access_request(
        &self,
        request: Request<CancelAccessRequestRequest>,
    ) -> Result<Response<CancelAccessRequestResponse>, Status> {
        let input = request.into_inner();
        self.call(move |c, now| {
            c.cancel(
                &input.agent_id,
                &input.request_id,
                input.expected_generation,
                now,
            )
            .map(|request| CancelAccessRequestResponse {
                request: Some(request),
            })
        })
        .await
        .map(Response::new)
    }
}
