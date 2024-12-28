use anyhow::{Context, Result};
use assert_cmd::Command;
use aws_sdk_sts as sts;
use chrono::{DateTime, Utc};
use regex::Regex;
use rstest::rstest;
use serde::Deserialize;
use std::path::Path;
use testcontainers_modules::{
    localstack::LocalStack,
    testcontainers::{core::ContainerAsync, runners::AsyncRunner, ImageExt},
};

#[derive(Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
struct TemporaryCredentials {
    aws_access_key_id: String,
    aws_secret_access_key: String,
    aws_session_token: String,
    aws_expiration: DateTime<Utc>,
}

#[allow(dead_code)]
fn make_sts_test_credentials() -> sts::config::Credentials {
    sts::config::Credentials::new("fake", "fake", None, None, "test")
}

#[allow(dead_code)]
async fn run_localstack() -> Result<ContainerAsync<LocalStack>> {
    let image = LocalStack::default().with_env_var("SERVICES", "sts");
    image.start().await.context("")
}

#[allow(dead_code)]
async fn endpoint_url(container: &ContainerAsync<LocalStack>) -> Result<String> {
    let host_ip = container.get_host().await?;
    let host_port = container.get_host_port_ipv4(4566).await?;
    let endpoint_url = format!("http://{}:{}", host_ip, host_port);
    Ok(endpoint_url)
}

#[allow(dead_code)]
async fn make_sts_config(container: &ContainerAsync<LocalStack>) -> Result<sts::Config> {
    let endpoint_url = endpoint_url(container).await?;
    let credentials = make_sts_test_credentials();
    let config = sts::Config::builder()
        .behavior_version(aws_config::BehaviorVersion::latest())
        .region(aws_config::Region::new("ap-northeast-1"))
        .credentials_provider(credentials)
        .endpoint_url(endpoint_url)
        .build();
    Ok(config)
}

#[rstest]
#[case::version(vec!["--version"], true, 0)]
#[case::no_arguments(vec![], false, 2)]
#[case::no_such_profile(
    vec!["--config", "tests/fixtures/config.toml", "--profile-name", "no_such_profile"], false, 2)]
#[case::coflict_role_name_and_profile_name(
    vec!["--profile-name", "test", "--role-arn", "arn:aws:iam..."], false, 2)]
#[case::conflict_config_and_role_arn(
    vec!["--config", "tests/fixtures/config.toml", "--role-arn", "arn:aws:iam..."], false, 2)]
#[case::conflict_totp_secret_and_totp_code(
    vec!["--role-arn", "arn:aws:iam...", "--totp-secret", "secret", "--totp-code", "123456"], false, 2)]
fn test_arguments(#[case] args: Vec<&str>, #[case] success: bool, #[case] code: i32) {
    let assert = Command::cargo_bin("assume-role").unwrap().args(args).assert();
    if success {
        assert.success().code(code);
    } else {
        assert.failure().code(code);
    }
}

#[tokio::test]
#[ignore]
async fn format_json() -> Result<()> {
    let container = run_localstack().await?;
    let endpoint_url = endpoint_url(&container).await?;

    {
        let assert = Command::cargo_bin("assume-role")
            .unwrap()
            .env("AWS_ENDPOINT_URL", endpoint_url.clone())
            .env("AWS_ACCESS_KEY_ID", "fake")
            .env("AWS_SECRET_ACCESS_KEY", "fake")
            .env("AWS_DEFAULT_REGION", "ap-northeast-1")
            .env("SERIAL_NUMBER", "fake")
            .env("TOTP_CODE", "123456")
            .arg("--format=json")
            .arg("--role-arn=arn:aws:iam::123456789012:role/TestUser")
            .assert();
        println!("assertion start");
        let output = assert.get_output().to_owned();
        assert.success().code(0);
        let c: TemporaryCredentials = serde_json::from_str(&String::from_utf8(output.stdout)?)?;
        let re_aws_access_key_id = Regex::new(r"[A-Z0-9]{20}").unwrap();
        assert!(re_aws_access_key_id.is_match(&c.aws_access_key_id));
        let re_aws_secret_access_key = Regex::new(r"[a-zA-Z0-9]+").unwrap();
        assert!(re_aws_secret_access_key.is_match(&c.aws_secret_access_key));
        assert!(c.aws_expiration.to_rfc3339().starts_with("20"));
        assert!(c.aws_session_token.len() > 0);
    }

    {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(Path::new("tests/fixtures/config.toml"));
        let full_path = path.canonicalize()?;
        println!("{:?}", path);

        let assert = Command::cargo_bin("assume-role")
            .unwrap()
            .env("AWS_ENDPOINT_URL", endpoint_url.clone())
            .env("AWS_ACCESS_KEY_ID", "fake")
            .env("AWS_SECRET_ACCESS_KEY", "fake")
            .env("AWS_DEFAULT_REGION", "ap-northeast-1")
            .env("SERIAL_NUMBER", "fake")
            .env("TOTP_CODE", "123456")
            .arg("--format=json")
            .arg("--config")
            .arg(full_path)
            .arg("--profile-name=test")
            .assert();
        println!("assertion start");
        let output = assert.get_output().to_owned();
        assert.success().code(0);
        let c: TemporaryCredentials = serde_json::from_str(&String::from_utf8(output.stdout)?)?;
        let re_aws_access_key_id = Regex::new(r"[A-Z0-9]{20}").unwrap();
        assert!(re_aws_access_key_id.is_match(&c.aws_access_key_id));
        let re_aws_secret_access_key = Regex::new(r"[a-zA-Z0-9]+").unwrap();
        assert!(re_aws_secret_access_key.is_match(&c.aws_secret_access_key));
        assert!(c.aws_expiration.to_rfc3339().starts_with("20"));
        assert!(c.aws_session_token.len() > 0);
    }

    Ok(())
}

#[rstest]
#[case("bash", "export ")]
#[case("zsh", "export ")]
#[case("fish", "set -gx ")]
#[case("power-shell", "\\$env:")]
#[tokio::test]
#[ignore]
async fn format_shell(#[case] shell_type: String, #[case] prefix: String) -> Result<()> {
    let container = run_localstack().await?;
    let endpoint_url = endpoint_url(&container).await?;

    let assert = Command::cargo_bin("assume-role")
        .unwrap()
        .env("AWS_ENDPOINT_URL", endpoint_url)
        .env("AWS_ACCESS_KEY_ID", "fake")
        .env("AWS_SECRET_ACCESS_KEY", "fake")
        .env("AWS_DEFAULT_REGION", "ap-northeast-1")
        .env("SERIAL_NUMBER", "fake")
        .env("TOTP_CODE", "123456")
        .arg("--format")
        .arg(shell_type)
        .arg("--role-arn=arn:aws:iam::123456789012:role/TestUser")
        .assert();
    println!("assertion start");
    let output = assert.get_output().to_owned();
    assert.success().code(0);
    let re1 = Regex::new(&format!("{}AWS_ACCESS_KEY_ID", prefix)).unwrap();
    let re2 = Regex::new(&format!("{}AWS_SECRET_ACCESS_KEY", prefix)).unwrap();
    let re3 = Regex::new(&format!("{}AWS_SESSION_TOKEN", prefix)).unwrap();
    let re4 = Regex::new(&format!("{}AWS_EXPIRATION", prefix)).unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    debug_assert!(re1.is_match(&stdout));
    assert!(re2.is_match(&stdout));
    assert!(re3.is_match(&stdout));
    assert!(re4.is_match(&stdout));

    Ok(())
}
