use bullmq::JobProgress;
use bullmq::types::JobState;
use raziel_proto::raziel::v1::ProofCapability;
use serde_json::json;

use super::bullmq::RetainedQueueJob;
use super::lifecycle::{CapabilityLifecycleError, PublicCapabilityJobState, project_proof_job};
use super::{CAPABILITY_JOB_NAME, CapabilityJobPayload};
use crate::one_shot::OneShotMetrics;
use crate::private_envelope::PrivateEnvelopeRef;
use crate::worker::{SupervisorProgressPhase, TerminalCapabilityResult};

const SUBMITTED_AT: u64 = 1_800_000_000_000;

#[test]
fn projects_waiting_and_closed_active_progress() {
    let queued = project_proof_job(record(JobState::Waiting)).unwrap();
    assert_eq!(queued.state(), PublicCapabilityJobState::Queued);
    assert_eq!(queued.submitted_at_unix_millis(), SUBMITTED_AT);
    assert_eq!(queued.updated_at_unix_millis(), SUBMITTED_AT);

    for (phase, expected) in [
        (
            SupervisorProgressPhase::Executing,
            PublicCapabilityJobState::Executing,
        ),
        (
            SupervisorProgressPhase::Proving,
            PublicCapabilityJobState::Proving,
        ),
    ] {
        let mut active = record(JobState::Active);
        active.processed_on_ms = Some(SUBMITTED_AT + 10);
        active.progress = JobProgress::Object(phase.to_public_value());
        let projected = project_proof_job(active).unwrap();
        assert_eq!(projected.state(), expected);
        assert_eq!(projected.updated_at_unix_millis(), SUBMITTED_AT + 10);
    }
}

#[test]
fn rejects_open_or_malformed_active_progress() {
    let mut numeric = record(JobState::Active);
    numeric.processed_on_ms = Some(SUBMITTED_AT + 10);
    assert_eq!(
        project_proof_job(numeric),
        Err(CapabilityLifecycleError::QueueRecordMismatch)
    );

    let mut extra = record(JobState::Active);
    extra.processed_on_ms = Some(SUBMITTED_AT + 10);
    extra.progress = JobProgress::Object(json!({
        "format_version": 1,
        "phase": "EXECUTING",
        "private": "not-accepted",
    }));
    assert_eq!(
        project_proof_job(extra),
        Err(CapabilityLifecycleError::QueueRecordMismatch)
    );
}

#[test]
fn projects_only_the_closed_failure_classes() {
    for (reason, expected, has_failure) in [
        ("CANCELED", PublicCapabilityJobState::Canceled, false),
        ("INVALID_REQUEST", PublicCapabilityJobState::Failed, true),
        (
            "SERVICE_UNAVAILABLE",
            PublicCapabilityJobState::Failed,
            true,
        ),
    ] {
        let mut failed = record(JobState::Failed);
        failed.processed_on_ms = Some(SUBMITTED_AT + 10);
        failed.finished_on_ms = Some(SUBMITTED_AT + 20);
        failed.failed_reason = reason.to_owned();
        let projected = project_proof_job(failed).unwrap();
        assert_eq!(projected.state(), expected);
        assert_eq!(projected.failure().is_some(), has_failure);
    }

    let mut open = record(JobState::Failed);
    open.processed_on_ms = Some(SUBMITTED_AT + 10);
    open.finished_on_ms = Some(SUBMITTED_AT + 20);
    open.failed_reason = "database host and private details".to_owned();
    assert_eq!(
        project_proof_job(open),
        Err(CapabilityLifecycleError::QueueRecordMismatch)
    );
}

#[test]
fn completed_result_decodes_canonical_capability_and_safe_metrics() {
    let job_id = "11".repeat(32);
    let capability = ProofCapability {
        format_version: 1,
        capability_id: job_id,
        capability_request_commitment: vec![0x11; 32],
        zk_proof: vec![1, 2, 3],
        ..ProofCapability::default()
    };
    let result = TerminalCapabilityResult::for_test(
        capability.clone(),
        OneShotMetrics {
            execute_millis: 40,
            prove_millis: 1_000,
            artifact_bytes: 3,
            peak_memory_bytes: 1_024,
            peak_swap_bytes: 0,
        },
    )
    .unwrap();
    let mut completed = record(JobState::Completed);
    completed.processed_on_ms = Some(SUBMITTED_AT + 10);
    completed.finished_on_ms = Some(SUBMITTED_AT + 20);
    completed.return_value = serde_json::to_string(&result.to_public_value().unwrap()).unwrap();

    let projected = project_proof_job(completed).unwrap();
    assert_eq!(projected.state(), PublicCapabilityJobState::Succeeded);
    assert_eq!(projected.capability(), Some(&capability));
    assert_eq!(
        projected.metrics().unwrap().proof_size_bytes,
        capability.zk_proof.len() as u64
    );
}

#[test]
fn completed_result_rejects_extra_fields_and_job_binding_changes() {
    let mut completed = record(JobState::Completed);
    completed.processed_on_ms = Some(SUBMITTED_AT + 10);
    completed.finished_on_ms = Some(SUBMITTED_AT + 20);
    completed.return_value = json!({
        "format_version": 1,
        "capability_protobuf_hex": "00",
        "execute_millis": 1,
        "prove_millis": 1,
        "artifact_bytes": 1,
        "peak_memory_bytes": 1,
        "peak_swap_bytes": 0,
        "private": "not-accepted",
    })
    .to_string();
    assert_eq!(
        project_proof_job(completed),
        Err(CapabilityLifecycleError::QueueRecordMismatch)
    );
}

fn record(state: JobState) -> RetainedQueueJob {
    let job_id = "11".repeat(32);
    let reference = PrivateEnvelopeRef::from_queue_token(&format!("pe1_{job_id}")).unwrap();
    RetainedQueueJob {
        id: job_id,
        name: CAPABILITY_JOB_NAME.to_owned(),
        payload: Ok(CapabilityJobPayload::new(reference)),
        policy_matches: true,
        state,
        timestamp_ms: SUBMITTED_AT,
        processed_on_ms: None,
        finished_on_ms: None,
        progress: JobProgress::Number(0.0),
        failed_reason: String::new(),
        return_value: String::new(),
    }
}
