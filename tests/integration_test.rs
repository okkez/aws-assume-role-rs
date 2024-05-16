use anyhow::Result;
use assert_cmd::Command;
use aws_sdk_sts as sts;
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use rstest::rstest;
use serde::Deserialize;
use std::path::Path;
use testcontainers::ContainerAsync;
use testcontainers_modules::localstack::LocalStack;
use testcontainers_modules::testcontainers::{runners::AsyncRunner, RunnableImage};
use tokio::sync::OnceCell;

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

async fn run_localstack() -> &'static ContainerAsync<LocalStack> {
    static CONTAINER: Lazy<OnceCell<ContainerAsync<LocalStack>>> = Lazy::new(OnceCell::new);
    let container = CONTAINER
        .get_or_init(|| async {
            let image = RunnableImage::from(LocalStack).with_env_var(("SERVICES", "sts"));
            image.start().await
        })
        .await;
    Box::leak(Box::new(container))
}

#[allow(dead_code)]
async fn endpoint_url(container: &ContainerAsync<LocalStack>) -> Result<String> {
    let host_ip = container.get_host().await;
    let host_port = container.get_host_port_ipv4(4566).await;
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

#[test]
fn no_serial_number() {
    let assert = Command::cargo_bin("assume-role").unwrap().assert();
    assert.failure().code(2);
}

#[test]
fn test_version() {
    let assert = Command::cargo_bin("assume-role").unwrap().arg("--version").assert();
    assert.success().code(0);
}

#[tokio::test]
async fn format_json() -> Result<()> {
    let container = run_localstack().await;
    let endpoint_url = endpoint_url(&container).await?;

    let assert = Command::cargo_bin("assume-role")
        .unwrap()
        .env("AWS_ENDPOINT_URL", endpoint_url)
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

    Ok(())
}

#[rstest]
#[case("bash", "export ")]
#[case("zsh", "export ")]
#[case("fish", "set -gx ")]
#[case("power-shell", "\\$env:")]
#[tokio::test]
async fn format_shell(#[case] shell_type: String, #[case] prefix: String) -> Result<()> {
    let container = run_localstack().await;
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

#[tokio::test]
async fn format_json_with_config_file() -> Result<()> {
    let container = run_localstack().await;
    let endpoint_url = endpoint_url(&container).await?;
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(Path::new("tests/fixtures/config.toml"));
    let full_path = path.canonicalize()?;
    println!("{:?}", path);

    let assert = Command::cargo_bin("assume-role")
        .unwrap()
        .env("AWS_ENDPOINT_URL", endpoint_url)
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

    Ok(())
}
