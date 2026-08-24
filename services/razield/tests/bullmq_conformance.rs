use std::error::Error;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use bullmq::types::{JobCounts, JobState, KeepJobs, RemoveOnFinish};
use bullmq::worker::CancellationToken;
use bullmq::{Job, JobProgress, Queue, QueueEvent, QueueEvents, QueueEventsOptions, Worker};
use razield::queue::{GLOBAL_CONCURRENCY, QueueConfig};
use tokio::time::Instant;

const QUEUE_NAME: &str = "proof-jobs";
const WAIT_TIMEOUT: Duration = Duration::from_secs(20);
const POLL_INTERVAL: Duration = Duration::from_millis(25);
const OBLITERATE_BATCH_SIZE: usize = 1_000;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

fn test_config(case_name: &str) -> TestResult<QueueConfig> {
    let redis_url = std::env::var("RAZIEL_QUEUE_TEST_REDIS_URL").map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "RAZIEL_QUEUE_TEST_REDIS_URL is required",
        )
    })?;
    let prefix = std::env::var("RAZIEL_QUEUE_TEST_PREFIX")
        .unwrap_or_else(|_| "raziel-conformance".to_string());

    Ok(QueueConfig::new(
        redis_url,
        format!("{prefix}-{case_name}"),
    )?)
}

async fn open_clean_queue(config: &QueueConfig) -> TestResult<Queue> {
    let queue = Queue::with_options(QUEUE_NAME, config.queue_options()).await?;
    queue.remove_global_concurrency().await?;
    queue.obliterate(true, OBLITERATE_BATCH_SIZE).await?;
    Ok(queue)
}

async fn wait_for_counts(
    queue: &Queue,
    description: &str,
    predicate: impl Fn(&JobCounts) -> bool,
) -> TestResult<JobCounts> {
    let deadline = Instant::now() + WAIT_TIMEOUT;
    loop {
        let counts = queue.get_job_counts().await?;
        if predicate(&counts) {
            return Ok(counts);
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("timed out waiting for {description}"),
            )
            .into());
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

async fn add_public_test_job(
    queue: &Queue,
    config: &QueueConfig,
    name: &str,
    job_id: &str,
) -> TestResult<()> {
    queue
        .add(name, "synthetic-public-test-payload")
        .options(config.job_options())
        .job_id(job_id)
        .await?;
    Ok(())
}

async fn verify_custom_id_and_queued_removal() -> TestResult {
    let config = test_config("identity")?;
    let queue = open_clean_queue(&config).await?;

    let first = queue
        .add("deduplicated", "synthetic-public-test-payload")
        .options(config.job_options())
        .job_id("request-commitment-1")
        .await?;
    let duplicate = queue
        .add("deduplicated", "synthetic-public-test-payload")
        .options(config.job_options())
        .job_id("request-commitment-1")
        .await?;

    assert_eq!(first.id(), "request-commitment-1");
    assert_eq!(duplicate.id(), first.id());
    assert_eq!(queue.get_job_counts().await?.waiting, 1);

    add_public_test_job(&queue, &config, "queued-removal", "queued-removal-1").await?;
    assert!(queue.remove("queued-removal-1").await?);
    assert!(queue.get_job("queued-removal-1").await?.is_none());

    queue.obliterate(true, OBLITERATE_BATCH_SIZE).await?;
    queue.close().await;
    Ok(())
}

async fn verify_concurrency_limit(
    case_name: &str,
    worker_concurrency: usize,
    global_concurrency: Option<u64>,
) -> TestResult {
    let config = test_config(case_name)?;
    let queue = open_clean_queue(&config).await?;
    if let Some(limit) = global_concurrency {
        queue.set_global_concurrency(limit).await?;
    }

    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let processor_active = Arc::clone(&active);
    let processor_peak = Arc::clone(&peak);
    let worker = Worker::with_options(
        QUEUE_NAME,
        move |job: Job, _token: CancellationToken| {
            let active = Arc::clone(&processor_active);
            let peak = Arc::clone(&processor_peak);
            async move {
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(current, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(150)).await;
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(job.data().clone())
            }
        },
        config.worker_options().concurrency(worker_concurrency),
    )
    .await?;

    for id in 1..=3 {
        add_public_test_job(
            &queue,
            &config,
            "concurrency",
            &format!("concurrency-{case_name}-{id}"),
        )
        .await?;
    }

    wait_for_counts(&queue, "three completed concurrency jobs", |counts| {
        counts.completed == 3 && counts.waiting == 0 && counts.active == 0
    })
    .await?;
    assert_eq!(peak.load(Ordering::SeqCst), 1);

    worker.close(5_000).await?;
    queue.remove_global_concurrency().await?;
    queue.obliterate(true, OBLITERATE_BATCH_SIZE).await?;
    queue.close().await;
    Ok(())
}

async fn verify_lock_cancellation_progress_and_retention() -> TestResult {
    let config = test_config("lifecycle")?;
    let queue = open_clean_queue(&config).await?;
    queue.set_global_concurrency(GLOBAL_CONCURRENCY).await?;
    let queue_options = config.queue_options();
    let events = QueueEvents::with_options(
        QUEUE_NAME,
        QueueEventsOptions {
            connection: queue_options.connection,
            prefix: queue_options.prefix,
            last_event_id: Some("0".to_string()),
            ..Default::default()
        },
    )
    .await?;

    let invocations = Arc::new(AtomicUsize::new(0));
    let cancellation_seen = Arc::new(AtomicBool::new(false));
    let processor_invocations = Arc::clone(&invocations);
    let processor_cancellation_seen = Arc::clone(&cancellation_seen);
    let test_retention = RemoveOnFinish::Options(KeepJobs {
        age: Some(60),
        count: Some(2),
        limit: Some(10),
    });
    let worker = Arc::new(
        Worker::with_options(
            QUEUE_NAME,
            move |mut job: Job, token: CancellationToken| {
                let invocations = Arc::clone(&processor_invocations);
                let cancellation_seen = Arc::clone(&processor_cancellation_seen);
                async move {
                    invocations.fetch_add(1, Ordering::SeqCst);
                    job.update_progress(JobProgress::Number(50.0)).await?;
                    if job.name() == "cancel-me" {
                        token.cancelled().await;
                        cancellation_seen.store(true, Ordering::SeqCst);
                    } else {
                        // This exceeds lock_duration and only succeeds with renewal.
                        tokio::time::sleep(Duration::from_millis(650)).await;
                    }
                    Ok(job.data().clone())
                }
            },
            config
                .worker_options()
                .lock_duration(Duration::from_millis(300))
                .lock_renew_time(Duration::from_millis(75))
                .remove_on_complete(test_retention.clone()),
        )
        .await?,
    );

    for id in 1..=3 {
        queue
            .add("lock-renewal", "synthetic-public-test-payload")
            .options(
                config
                    .job_options()
                    .remove_on_complete(test_retention.clone()),
            )
            .job_id(format!("retained-{id}"))
            .await?;
    }
    queue
        .add("cancel-me", "synthetic-public-test-payload")
        .options(config.job_options().remove_on_complete(test_retention))
        .job_id("cancel-active-1")
        .await?;

    let deadline = Instant::now() + WAIT_TIMEOUT;
    let mut cancellation_requested = false;
    while !cancellation_seen.load(Ordering::SeqCst) {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "timed out waiting for active cancellation",
            )
            .into());
        }
        if !cancellation_requested {
            let job = queue
                .get_job("cancel-active-1")
                .await?
                .ok_or_else(|| io::Error::other("active cancellation job is missing"))?;
            if job.get_state().await? == JobState::Active {
                assert!(worker.cancel_job("cancel-active-1").await);
                cancellation_requested = true;
            }
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }

    let counts = wait_for_counts(&queue, "bounded completed-job retention", |counts| {
        counts.completed == 2 && counts.waiting == 0 && counts.active == 0
    })
    .await?;
    assert_eq!(counts.completed, 2);
    assert_eq!(invocations.load(Ordering::SeqCst), 4);
    assert!(cancellation_requested);

    let progress_deadline = Instant::now() + WAIT_TIMEOUT;
    let mut progress_seen = false;
    while !progress_seen {
        let remaining = progress_deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "timed out waiting for a BullMQ progress event",
            )
            .into());
        }
        let entry = tokio::time::timeout(remaining, events.next_event())
            .await
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timed out waiting for a queue event",
                )
            })?
            .ok_or_else(|| io::Error::other("queue event channel closed"))?;
        match entry.event {
            QueueEvent::Progress { .. } => progress_seen = true,
            QueueEvent::Error { .. } => {
                return Err(io::Error::other("BullMQ queue-event reader failed").into());
            }
            _ => {}
        }
    }

    worker.close(5_000).await?;
    events.close().await;
    queue.remove_global_concurrency().await?;
    queue.obliterate(true, OBLITERATE_BATCH_SIZE).await?;
    queue.close().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires the digest-pinned Redis conformance service"]
async fn queue_lifecycle_conforms() -> TestResult {
    verify_custom_id_and_queued_removal().await?;
    verify_concurrency_limit("worker-limit", 1, None).await?;
    verify_concurrency_limit("global-limit", 4, Some(GLOBAL_CONCURRENCY)).await?;
    verify_lock_cancellation_progress_and_retention().await?;
    println!("bullmq_lifecycle_conformance=pass");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires the digest-pinned Redis conformance service"]
async fn stalled_job_never_reinvokes_processor() -> TestResult {
    let config = test_config("stalled")?;
    let queue = open_clean_queue(&config).await?;
    add_public_test_job(&queue, &config, "stall-once", "stall-once-1").await?;

    let processor_invocations = Arc::new(AtomicUsize::new(0));
    let worker_invocations = Arc::clone(&processor_invocations);
    let worker = Worker::with_options(
        QUEUE_NAME,
        move |job: Job, _token: CancellationToken| {
            let invocations = Arc::clone(&worker_invocations);
            async move {
                invocations.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(1_000)).await;
                Ok(job.data().clone())
            }
        },
        config
            .worker_options()
            .lock_duration(Duration::from_millis(200))
            .stalled_interval(Duration::from_millis(250))
            .skip_lock_renewal(),
    )
    .await?;

    wait_for_counts(&queue, "terminal stalled-job failure", |counts| {
        counts.failed == 1 && counts.waiting == 0 && counts.active == 0
    })
    .await?;
    let job = queue
        .get_job("stall-once-1")
        .await?
        .ok_or_else(|| io::Error::other("failed stalled job is missing"))?;
    assert_eq!(processor_invocations.load(Ordering::SeqCst), 1);
    assert!(job.attempts_started() >= 1);

    worker.close(3_000).await?;
    println!("stalled_processor_invocations=1");
    println!("stalled_queue_claims={}", job.attempts_started());
    println!("zero_proof_retry_stalled_failure=pass");
    queue.obliterate(true, OBLITERATE_BATCH_SIZE).await?;
    queue.close().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires the digest-pinned Redis conformance service"]
async fn prepare_restart_recovery() -> TestResult {
    let config = test_config("restart")?;
    let queue = open_clean_queue(&config).await?;
    add_public_test_job(&queue, &config, "restart-waiting", "restart-waiting-1").await?;
    let counts = queue.get_job_counts().await?;
    assert_eq!(counts.waiting, 1);
    assert_eq!(counts.completed, 0);
    queue.close().await;
    println!("redis_restart_fixture=prepared");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires the digest-pinned Redis conformance service"]
async fn verify_restart_recovery() -> TestResult {
    let config = test_config("restart")?;
    let queue = Queue::with_options(QUEUE_NAME, config.queue_options()).await?;
    let counts = queue.get_job_counts().await?;
    assert_eq!(counts.waiting, 1);
    assert_eq!(counts.completed, 0);
    assert!(queue.get_job("restart-waiting-1").await?.is_some());

    queue.obliterate(true, OBLITERATE_BATCH_SIZE).await?;
    queue.close().await;
    println!("redis_restart_recovery=pass");
    Ok(())
}
