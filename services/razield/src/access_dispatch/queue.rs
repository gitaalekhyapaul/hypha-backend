use bullmq::types::{JobState, KeepJobs, RemoveOnFinish, RetryOptions};
use bullmq::{Job, JobOptions, Queue};

use crate::access_request::{AccessError, DispatchReference};
use crate::queue::MAX_PUBLIC_JOB_PAYLOAD_BYTES;

pub(super) const NAME: &str = "access-dispatch";
const JOB_NAME: &str = "dispatch-access";

pub(super) fn options() -> JobOptions {
    let retention = RemoveOnFinish::Options(KeepJobs {
        age: Some(7 * 24 * 60 * 60),
        count: Some(100),
        limit: Some(100),
    });
    JobOptions::new()
        .attempts(1)
        .remove_on_complete(retention.clone())
        .remove_on_fail(retention)
}

pub(super) fn reference(job: &Job) -> Result<DispatchReference, AccessError> {
    if job.name() != JOB_NAME
        || serde_json::to_vec(job.data())
            .map_err(|_| AccessError::ServiceUnavailable)?
            .len()
            > MAX_PUBLIC_JOB_PAYLOAD_BYTES
        || serde_json::to_value(job.opts()).ok() != serde_json::to_value(options()).ok()
    {
        return Err(AccessError::ServiceUnavailable);
    }
    let reference: DispatchReference =
        serde_json::from_value(job.data().clone()).map_err(|_| AccessError::ServiceUnavailable)?;
    reference
        .validate()
        .map_err(|_| AccessError::ServiceUnavailable)?;
    if reference.job_id() != job.id() {
        return Err(AccessError::ServiceUnavailable);
    }
    Ok(reference)
}

pub(super) async fn enqueue(
    queue: &Queue,
    expected: &DispatchReference,
) -> Result<(), AccessError> {
    expected.validate()?;
    let existing = queue
        .get_job(&expected.job_id())
        .await
        .map_err(unavailable)?;
    let mut job = match existing {
        Some(job) => job,
        None => queue
            .add(JOB_NAME, expected)
            .options(options().size_limit(MAX_PUBLIC_JOB_PAYLOAD_BYTES))
            .job_id(expected.job_id())
            .await
            .map_err(unavailable)?,
    };
    // Read the persisted options, not the publisher's client-only fields.
    job = queue
        .get_job(job.id())
        .await
        .map_err(unavailable)?
        .ok_or(AccessError::ServiceUnavailable)?;
    if reference(&job)? != *expected {
        return Err(AccessError::ServiceUnavailable);
    }
    match job.get_state().await.map_err(unavailable)? {
        JobState::Waiting | JobState::Active => Ok(()),
        // Only the workflow queue is retried. Its pinned state is reconciled first.
        // The proof queue keeps its one-attempt, zero-stalled-retry policy.
        JobState::Failed => job
            .retry(
                "failed",
                Some(RetryOptions {
                    reset_attempts_made: true,
                    reset_attempts_started: true,
                }),
            )
            .await
            .map_err(unavailable),
        _ => Err(AccessError::ServiceUnavailable),
    }
}

fn unavailable(_: bullmq::Error) -> AccessError {
    AccessError::ServiceUnavailable
}
