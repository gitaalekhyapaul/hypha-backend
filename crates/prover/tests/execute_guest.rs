use std::fs;

use raziel_policy_ir::{CaveatValue, ContextEntry};
use raziel_prover::{MAX_EXECUTION_CYCLE_LIMIT, ProverError, Sp1Executor, prepare_guest_input};
use sp1_sdk::{
    Elf, StatusCode,
    blocking::{Prover, ProverClient, SP1Stdin},
};

#[path = "support/authorization_fixture.rs"]
mod authorization_fixture;
mod support;

use authorization_fixture::authorization_inputs;
use support::guest_elf_path;

#[test]
#[ignore = "requires a prior pinned cargo prove build of the authorization guest"]
fn sp1_execute_matches_the_host_for_the_supported_positive_paths() {
    let guest_elf = fs::read(guest_elf_path()).expect("authorization guest ELF must exist");
    let executor = Sp1Executor::local();

    for (case_id, input) in authorization_inputs() {
        let prepared = prepare_guest_input(input).unwrap_or_else(|error| {
            panic!("{case_id}: host input preparation failed: {error}");
        });
        let report = executor
            .execute(&guest_elf, &prepared)
            .unwrap_or_else(|error| panic!("{case_id}: SP1 execution failed: {error}"));

        assert_eq!(
            report.public_values,
            prepared.expected_public_values_bytes(),
            "{case_id}: public journal mismatch"
        );
        assert!(
            report.instruction_count > 0,
            "{case_id}: missing instruction count"
        );
        eprintln!("{case_id}: instructions={}", report.instruction_count);
    }
}

#[test]
#[ignore = "requires a prior pinned cargo prove build of the authorization guest"]
fn sp1_guest_rejects_invalid_private_inputs_without_a_public_journal() {
    let guest_elf = fs::read(guest_elf_path()).expect("authorization guest ELF must exist");
    let valid = authorization_inputs();
    let case = |case_id: &str| {
        valid
            .iter()
            .find(|(id, _)| *id == case_id)
            .map(|(_, input)| input.clone())
            .unwrap()
    };

    let mut wrong_input_version = case("direct");
    wrong_input_version.format_version += 1;

    let mut wrong_policy_hash = case("direct");
    wrong_policy_hash.expected_policy_hash[0] ^= 1;

    let mut wrong_snapshot_root = case("direct");
    wrong_snapshot_root.relationship_snapshot_root[0] ^= 1;

    let mut invalid_time = case("direct");
    invalid_time.expires_at_unix_seconds = invalid_time.issued_at_unix_seconds;

    let mut duplicate_context = case("direct");
    duplicate_context.request.request_context = vec![
        ContextEntry {
            name: "duplicate".into(),
            value: CaveatValue::Bool(true),
        },
        ContextEntry {
            name: "duplicate".into(),
            value: CaveatValue::Bool(false),
        },
    ];

    let mut missing_context = case("caveated");
    missing_context.request.request_context.clear();

    let mut oversized_request = case("direct");
    oversized_request.request.task_id = "x".repeat(257);

    let mut unused_opening = case("direct");
    unused_opening.relationship_witness_set.entries.insert(
        0,
        case("caveated").relationship_witness_set.entries[0].clone(),
    );

    let mut duplicate_opening = case("direct");
    duplicate_opening
        .relationship_witness_set
        .entries
        .push(duplicate_opening.relationship_witness_set.entries[0].clone());

    let mut out_of_order_openings = case("intersection");
    out_of_order_openings
        .relationship_witness_set
        .entries
        .swap(0, 1);

    let mut cases = [
        ("wrong-input-version", wrong_input_version),
        ("wrong-policy-hash", wrong_policy_hash),
        ("wrong-snapshot-root", wrong_snapshot_root),
        ("invalid-time", invalid_time),
        ("duplicate-context", duplicate_context),
        ("missing-context", missing_context),
        ("oversized-request", oversized_request),
        ("unused-opening", unused_opening),
        ("duplicate-opening", duplicate_opening),
        ("out-of-order-openings", out_of_order_openings),
    ]
    .into_iter()
    .map(|(case_id, input)| {
        let mut stdin = SP1Stdin::new();
        stdin.write(&input);
        (case_id, stdin, StatusCode::PANIC)
    })
    .collect::<Vec<_>>();
    cases.push((
        "malformed-serialized-input",
        SP1Stdin::from(&[0xff]),
        StatusCode::INVALID_HINT,
    ));

    let prover = ProverClient::builder().light().build();
    for (case_id, stdin, expected_status) in cases {
        let (public_values, report) = prover
            .execute(Elf::from(guest_elf.as_slice()), stdin)
            .cycle_limit(MAX_EXECUTION_CYCLE_LIMIT)
            .calculate_gas(true)
            .expected_exit_code(StatusCode::ANY)
            .run()
            .unwrap_or_else(|error| {
                panic!("{case_id}: executor failed before returning the guest result: {error}")
            });
        assert_eq!(
            report.exit_code,
            u64::from(expected_status.as_u32()),
            "{case_id}: invalid input returned the wrong guest status"
        );
        assert!(
            public_values.as_slice().is_empty(),
            "{case_id}: invalid input produced a public journal"
        );
    }
}

#[test]
#[ignore = "requires a prior pinned cargo prove build of the authorization guest"]
fn executor_rejects_invalid_cycle_limits_and_enforces_a_low_limit() {
    assert!(matches!(
        Sp1Executor::local().with_cycle_limit(0),
        Err(ProverError::InvalidCycleLimit)
    ));
    assert!(matches!(
        Sp1Executor::local().with_cycle_limit(MAX_EXECUTION_CYCLE_LIMIT + 1),
        Err(ProverError::InvalidCycleLimit)
    ));
    assert!(
        Sp1Executor::local()
            .with_cycle_limit(MAX_EXECUTION_CYCLE_LIMIT)
            .is_ok()
    );

    let guest_elf = fs::read(guest_elf_path()).expect("authorization guest ELF must exist");
    let input = authorization_inputs().remove(0).1;
    let prepared = prepare_guest_input(input).unwrap();
    let result = Sp1Executor::local()
        .with_cycle_limit(1)
        .unwrap()
        .execute(&guest_elf, &prepared);
    assert_eq!(result, Err(ProverError::GuestExecutionFailed));
}
