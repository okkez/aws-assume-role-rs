use aws_assume_role::cli::Cli;
use clap::Parser;
use rstest::rstest;

#[test]
fn verify_cli() {
    use clap::CommandFactory;
    Cli::command().debug_assert()
}

#[test]
fn no_arguments() {
    let cli = Cli::parse_from(["assume-role"]);
    let r = cli.validate_arguments();
    assert!(!r.is_ok());
}

#[rstest]
#[case("", false)]
#[case("--totp-code=123456", true)]
#[case("--totp-secret=secret", true)]
fn serial_number(#[case] arg: &str, #[case] success: bool) {
    let cli = Cli::parse_from(["assume-role", "--serial-number=test_serial_number", arg]);
    let r = cli.validate_arguments();
    assert_eq!(r.is_ok(), success);
}
