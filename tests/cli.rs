use aws_assume_role::cli::Cli;
use clap::Parser;

#[test]
fn verify_cli() {
    use clap::CommandFactory;
    Cli::command().debug_assert()
}

#[test]
fn no_arguments() {
    let cli = Cli::try_parse_from(["assume-role"]);
    assert!(!cli.is_ok());
}

#[test]
fn serial_number() {
    let cli = Cli::try_parse_from(["assume-role", "--serial-number=test_serial_number"]);
    assert!(cli.is_ok());
}
