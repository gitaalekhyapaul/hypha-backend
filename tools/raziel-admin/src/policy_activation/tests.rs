use super::*;

#[test]
fn catalogue_cli_is_explicit_and_accepts_no_activation_fields() {
    let parse = |args: &[&str]| parse_client_arguments(args.iter().map(|value| (*value).into()));
    assert_eq!(
        parse(&["--list-approved"]),
        Ok(ClientOperation::ListCatalogue)
    );
    assert_eq!(
        parse(&["--candidate-id", "candidate-one", "--approved-by", "owner"]),
        Ok(ClientOperation::Activate(PolicyActivationRequest {
            candidate_id: "candidate-one".into(),
            approved_by: "owner".into(),
        })),
    );
    for args in [
        vec![],
        vec!["list-catalogue"],
        vec!["--list-approved", "--list-approved"],
        vec!["--list-approved", "--candidate-id", "candidate-one"],
        vec!["--candidate-id", "candidate-one", "--list-approved"],
        vec!["--list-approved", "--approved-by", "owner"],
    ] {
        assert_eq!(
            parse(&args),
            Err(PolicyActivationClientError::InvalidRequest)
        );
    }
}

#[test]
fn roles_cli_accepts_only_its_explicit_stdin_operation() {
    let parse = |args: &[&str]| parse_client_arguments(args.iter().map(|v| (*v).into()));
    assert_eq!(parse(&["--agent-roles"]), Ok(ClientOperation::AgentRoles));
    assert_eq!(
        parse(&["--agent-roles", "--candidate-id", "x"]),
        Err(PolicyActivationClientError::InvalidRequest)
    );
    assert_eq!(
        parse(&["--agent-roles", "--list-approved"]),
        Err(PolicyActivationClientError::InvalidRequest)
    );
}
