use assert_cmd::Command;
use aws_assume_role::cli::Cli;
use aws_sdk_sts as sts;

#[allow(dead_code)]
fn make_sts_test_credentials() -> sts::config::Credentials {
    sts::config::Credentials::new(
        "TESTCLIENT",
        "testsecretkey",
        Some("testsessiontoken".to_string()),
        None,
        "",
    )
}



#[test]
fn verify_cli() {
    use clap::CommandFactory;
    Cli::command().debug_assert()
}

#[test]
fn no_serial_number() {
    let assert = Command::cargo_bin("assume-role").unwrap().assert();
    assert.failure().code(2);
}
