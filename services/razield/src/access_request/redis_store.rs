use std::time::Duration;

use super::model::{AccessError, Phase, Record, valid_identity};

const ADMIT: &str = r#"
if redis.call('HEXISTS', KEYS[1], ARGV[1]) == 1 then return 0 end
if redis.call('ZCARD', KEYS[2]) >= 100 then return -1 end
if redis.call('ZLEXCOUNT', KEYS[2], '[' .. ARGV[3] .. ':', '[' .. ARGV[3] .. ';') >= 16 then return -1 end
redis.call('HSET', KEYS[1], ARGV[1], ARGV[2])
redis.call('ZADD', KEYS[2], 0, ARGV[1])
redis.call('ZADD', KEYS[3], 0, ARGV[1])
redis.call('ZADD', KEYS[4], ARGV[4], ARGV[1])
return 1
"#;

const CAS: &str = r#"
if redis.call('HGET', KEYS[1], ARGV[1]) ~= ARGV[2] then return 0 end
if ARGV[3] == '' then
  redis.call('HDEL', KEYS[1], ARGV[1])
  redis.call('ZREM', KEYS[2], ARGV[1])
  redis.call('ZREM', KEYS[3], ARGV[1])
  redis.call('ZREM', KEYS[4], ARGV[1])
  redis.call('HDEL', KEYS[5], ARGV[1])
else
  redis.call('HSET', KEYS[1], ARGV[1], ARGV[3])
  if ARGV[4] == '0' then redis.call('ZREM', KEYS[2], ARGV[1]) end
  redis.call('ZADD', KEYS[4], ARGV[5], ARGV[1])
  if ARGV[6] ~= '' then redis.call('HSET', KEYS[5], ARGV[1], ARGV[6]) end
end
return 1
"#;

pub(super) struct RedisStore {
    client: redis::Client,
    keys: [String; 5],
}

impl RedisStore {
    pub fn new(url: &str, prefix: &str) -> Result<Self, AccessError> {
        Ok(Self {
            client: redis::Client::open(url).map_err(unavailable)?,
            keys: ["records", "active", "index", "due", "results"]
                .map(|part| format!("{prefix}:access:v1:{part}")),
        })
    }

    // Fresh bounded connections make reconnection after Redis restart explicit.
    pub fn connect(&self) -> Result<redis::Connection, AccessError> {
        let connection = self
            .client
            .get_connection_with_timeout(Duration::from_secs(2))
            .map_err(unavailable)?;
        connection
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(unavailable)?;
        connection
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(unavailable)?;
        Ok(connection)
    }

    pub fn get(
        &self,
        connection: &mut redis::Connection,
        id: &str,
    ) -> Result<Option<(Record, String)>, AccessError> {
        let raw: Option<String> = redis::cmd("HGET")
            .arg(&self.keys[0])
            .arg(id)
            .query(connection)
            .map_err(unavailable)?;
        raw.map(|raw| Ok((Record::decode(id, &raw)?, raw)))
            .transpose()
    }

    pub fn reserve(
        &self,
        connection: &mut redis::Connection,
        record: &Record,
    ) -> Result<bool, AccessError> {
        let outcome: i32 = redis::cmd("EVAL")
            .arg(ADMIT)
            .arg(self.keys.len())
            .arg(&self.keys)
            .arg(record.identity())
            .arg(record.encode()?)
            .arg(&record.agent_id)
            .arg(record.due)
            .query(connection)
            .map_err(unavailable)?;
        match outcome {
            1 => Ok(true),
            0 => Ok(false),
            -1 => Err(AccessError::CapacityExceeded),
            _ => Err(AccessError::ServiceUnavailable),
        }
    }

    pub fn cas(
        &self,
        connection: &mut redis::Connection,
        id: &str,
        old: &str,
        next: Option<&Record>,
    ) -> Result<(), AccessError> {
        self.cas_with_result(connection, id, old, next, &[])
    }

    #[cfg(feature = "access-dispatch")]
    pub fn cas_terminal(
        &self,
        connection: &mut redis::Connection,
        old: &str,
        record: &Record,
        result: &[u8],
    ) -> Result<(), AccessError> {
        if !record.terminal()?
            || result.is_empty()
            || result.len() > super::proof_result::MAX_RESULT_BYTES
        {
            return Err(AccessError::ServiceUnavailable);
        }
        self.cas_with_result(connection, &record.identity(), old, Some(record), result)
    }

    pub fn result(
        &self,
        connection: &mut redis::Connection,
        id: &str,
    ) -> Result<Option<Vec<u8>>, AccessError> {
        let bytes: Option<Vec<u8>> = redis::cmd("HGET")
            .arg(&self.keys[4])
            .arg(id)
            .query(connection)
            .map_err(unavailable)?;
        if bytes.as_ref().is_some_and(|bytes| {
            bytes.is_empty() || bytes.len() > super::proof_result::MAX_RESULT_BYTES
        }) {
            return Err(AccessError::ServiceUnavailable);
        }
        Ok(bytes)
    }

    fn cas_with_result(
        &self,
        connection: &mut redis::Connection,
        id: &str,
        old: &str,
        next: Option<&Record>,
        result_bytes: &[u8],
    ) -> Result<(), AccessError> {
        let (encoded, active, due) = match next {
            Some(record) => (
                record.encode()?,
                record.phase != Phase::Deleting && !record.terminal()?,
                record.due,
            ),
            None => (String::new(), false, 0),
        };
        let result: i32 = redis::cmd("EVAL")
            .arg(CAS)
            .arg(self.keys.len())
            .arg(&self.keys)
            .arg(id)
            .arg(old)
            .arg(encoded)
            .arg(i32::from(active))
            .arg(due)
            .arg(result_bytes)
            .query(connection)
            .map_err(unavailable)?;
        if result == 1 {
            Ok(())
        } else {
            Err(AccessError::Conflict)
        }
    }

    pub fn due(
        &self,
        connection: &mut redis::Connection,
        now: i64,
    ) -> Result<Vec<String>, AccessError> {
        redis::cmd("ZRANGEBYSCORE")
            .arg(&self.keys[3])
            .arg("-inf")
            .arg(now)
            .arg("LIMIT")
            .arg(0)
            .arg(100)
            .query(connection)
            .map_err(unavailable)
    }

    pub fn active(&self, connection: &mut redis::Connection) -> Result<Vec<String>, AccessError> {
        let ids: Vec<String> = redis::cmd("ZRANGE")
            .arg(&self.keys[1])
            .arg(0)
            .arg(100)
            .query(connection)
            .map_err(unavailable)?;
        if ids.len() > 100 || ids.iter().any(|id| !valid_identity(id)) {
            return Err(AccessError::ServiceUnavailable);
        }
        Ok(ids)
    }

    pub fn list(
        &self,
        connection: &mut redis::Connection,
        owner: Option<&str>,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>, AccessError> {
        let lower = after
            .map(|id| format!("({id}"))
            .unwrap_or_else(|| owner.map_or_else(|| "-".to_owned(), |o| format!("[{o}:")));
        let upper = owner.map_or_else(|| "+".to_owned(), |o| format!("[{o};"));
        let ids: Vec<String> = redis::cmd("ZRANGEBYLEX")
            .arg(&self.keys[2])
            .arg(lower)
            .arg(upper)
            .arg("LIMIT")
            .arg(0)
            .arg(limit)
            .query(connection)
            .map_err(unavailable)?;
        if ids.iter().any(|id| !valid_identity(id)) {
            return Err(AccessError::ServiceUnavailable);
        }
        Ok(ids)
    }
}

fn unavailable(_: redis::RedisError) -> AccessError {
    AccessError::ServiceUnavailable
}
