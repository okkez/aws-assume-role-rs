use anyhow::{anyhow, bail, ensure, Context, Result};
use aws_sdk_sts as sts;
use backon::{ExponentialBuilder, Retryable};
use chrono::{DateTime, Local, SecondsFormat};
use clap::{Parser, ValueEnum};
use regex::Regex;
use serde::Deserialize;
use skim::prelude::*;
use skim::{Skim, SkimItemReceiver, SkimItemSender};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::Command;
use totp_rs::{Algorithm, Secret, TOTP};

#[allow(unused_imports)]
use mockall::automock;
use sts::operation::assume_role::AssumeRoleOutput;
use sts::operation::get_caller_identity::GetCallerIdentityOutput;

#[cfg(test)]
use MockStsImpl as Sts;
#[cfg(not(test))]
use StsImpl as Sts;

#[allow(dead_code)]
pub struct StsImpl {
    inner: sts::Client,
}

#[cfg_attr(test, automock)]
impl StsImpl {
    #[allow(dead_code)]
    pub fn new(inner: sts::Client) -> Self {
        Self { inner }
    }

    #[allow(dead_code)]
    pub async fn get_caller_identity(&self) -> Result<GetCallerIdentityOutput> {
        self.inner
            .get_caller_identity()
            .send()
            .await
            .context("Failed to call get_caller_identity")
    }

    #[allow(dead_code)]
    pub async fn assume_role(
        &self,
        role_arn: String,
        duration_seconds: i32,
        serial_number: String,
        token_code: String,
    ) -> Result<AssumeRoleOutput> {
        let now = Local::now().timestamp_millis();
        self.inner
            .assume_role()
            .role_session_name(format!("{}-session", now))
            .role_arn(role_arn)
            .duration_seconds(duration_seconds)
            .serial_number(serial_number)
            .token_code(token_code)
            .send()
            .await
            .context("Failed to call assume_role")
    }
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// The profile name
    #[arg(short, long)]
    profile_name: Option<String>,
    /// The IAM Role ARN to assume
    #[arg(short, long, env)]
    role_arn: Option<String>,
    /// The config file. default: $HOME/.aws/config.toml
    #[arg(short, long)]
    config: Option<String>,
    /// The duration, in seconds, of the role session. (900-43200)
    #[arg(short, long, default_value = "1h")]
    duration: String,
    /// MFA device ARN such as arn:aws:iam::123456789012/mfa/user
    #[arg(short = 'n', long, env)]
    serial_number: String,
    /// The base32 format TOTP secret
    #[arg(short = 's', long, env)]
    totp_secret: Option<String>,
    /// The TOTP code generated by other tool
    #[arg(short, long, env)]
    totp_code: Option<String>,
    /// Output format
    #[arg(short, long, value_enum)]
    format: Option<Format>,
    /// Print verbose logs
    #[arg(short, long)]
    verbose: bool,
    /// Commands to execute
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

#[derive(Clone, Debug, ValueEnum)]
enum Format {
    Json,
    Bash,
    Zsh,
    Fish,
    PowerShell,
}

#[derive(Debug, Deserialize)]
struct Config {
    profile: HashMap<String, Profile>,
}

#[derive(Debug, Deserialize)]
struct Profile {
    role_arn: String,
}

struct Item {
    label: String,
    role_arn: String,
}

impl<'a> Cli {
    pub async fn execute(&self, sts_client: sts::Client) -> Result<()> {
        let sts = Sts::new(sts_client);
        if self.verbose {
            println!("{}", self.get_caller_identity(&sts).await?);
        }

        let credentials = self.assume_role(&sts).await?;
        let dt = DateTime::from_timestamp_millis(credentials.expiration().to_millis()?)
            .context("Unable to built DateTime")?;
        let envs = HashMap::from([
            ("AWS_ACCESS_KEY_ID", credentials.access_key_id.clone()),
            ("AWS_SECRET_ACCESS_KEY", credentials.secret_access_key.clone()),
            ("AWS_SESSION_TOKEN", credentials.session_token.clone()),
            ("AWS_EXPIRATION", dt.to_rfc3339_opts(SecondsFormat::Millis, false)),
        ]);
        match &self.format {
            Some(format) => println!("{}", self.output(format, &envs)?),
            None => self.exec_command(&envs)?,
        };
        Ok(())
    }

    pub async fn get_caller_identity(&self, sts: &Sts) -> Result<String> {
        let response = sts.get_caller_identity().await?;
        Ok(format!(
            "UserId:  {}\nAccount: {}\nArn:     {}",
            response.user_id().unwrap_or_default(),
            response.account().unwrap_or_default(),
            response.arn().unwrap_or_default()
        ))
    }

    pub async fn assume_role(&self, sts: &Sts) -> Result<sts::types::Credentials> {
        let role_arn = self.role_arn().context("Unable to set role_arn")?;
        let duration_seconds = self.duration_seconds().context("Invalid duration")?;
        let output = (|| async {
            sts.assume_role(
                role_arn.clone(),
                duration_seconds,
                self.serial_number.clone(),
                self.totp_code().context("Unable to generate TOTP code")?,
            )
            .await
            .context("retryable")
        })
        .retry(&ExponentialBuilder::default())
        .when(|e| e.to_string() == "retryable")
        .await?;
        match output.credentials() {
            Some(credentials) => Ok(credentials.clone()),
            None => bail!("Unable to fetch temporary credentials"),
        }
    }

    fn output(&self, format: &Format, envs: &HashMap<&str, String>) -> Result<String> {
        let result = match format {
            Format::Json => serde_json::to_string(envs)?,
            Format::Bash | Format::Zsh => envs
                .iter()
                .map(|(k, v)| format!(r#"export {}="{}""#, k, v))
                .collect::<Vec<_>>()
                .join("\n"),
            Format::Fish => envs
                .iter()
                .map(|(k, v)| format!(r#"set -gx {} "{}""#, k, v))
                .collect::<Vec<_>>()
                .join("\n"),
            Format::PowerShell => envs
                .iter()
                .map(|(k, v)| format!(r#"$env:{}="{}""#, k, v))
                .collect::<Vec<_>>()
                .join("\n"),
        };
        Ok(result)
    }

    #[cfg(unix)]
    fn exec_command(&self, envs: &HashMap<&str, String>) -> Result<()> {
        let (exe, args) = self.args.split_at(1);
        Command::new(exe[0].clone()).args(args).envs(envs).exec();
        Ok(())
    }

    #[cfg(windows)]
    fn exec_command(&self, envs: &HashMap<&str, String>) -> Result<()> {
        let (exe, args) = self.args.split_at(1);
        let mut child = Command::new(exe[0].clone())
            .args(args)
            .envs(envs)
            .spawn()
            .context("Failed to spawn command")?;
        let status = child.wait().context("Fail waiting child process")?;
        match status.code() {
            Some(code) => ::std::process::exit(code),
            None => println!("Child process terminated by signal"),
        };
        Ok(())
    }

    fn duration_seconds(&self) -> Result<i32> {
        let re = Regex::new(r"(\d+)(s|m|h)?").unwrap();
        let duration = match re.captures(&self.duration) {
            Some(caps) => match (caps[1].parse::<i32>(), &caps[2]) {
                (Ok(seconds), "s") => seconds,
                (Ok(minutes), "m") => minutes * 60,
                (Ok(hours), "h") => hours * 60 * 60,
                (_, _) => 900,
            },
            None => 900,
        };
        ensure!(
            duration >= 900 && duration <= 43200,
            "duration ({}) must be between 900 seconds (15 minutes) and 43200 seconds (12 hours)",
            duration
        );
        Ok(duration)
    }

    fn totp_code(&self) -> Result<String> {
        if let Some(totp_code) = self.totp_code.clone() {
            return Ok(totp_code);
        }
        let secret = match self.totp_secret.clone() {
            Some(s) => Secret::Encoded(s).to_bytes().unwrap(),
            None => bail!("TOTP_SECRET is required"),
        };
        let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, secret).unwrap();
        Ok(totp.generate_current().unwrap())
    }

    fn role_arn(&self) -> Result<String> {
        if let Some(role_arn) = self.role_arn.clone() {
            return Ok(role_arn);
        }
        let mut toml_str = String::new();
        let mut io = match &self.config {
            Some(path) => File::open(path).unwrap(),
            None => {
                let home_dir = dirs::home_dir().context("Unable to get home directory")?;
                File::open(home_dir.join(".aws/config.toml")).context("Unable to read $HOME/.aws/config.toml")?
            }
        };
        io.read_to_string(&mut toml_str).context("Unable to read config file")?;
        let config: Config = toml::from_str(&toml_str).context("Unable to parse config file")?;

        match &self.profile_name {
            Some(name) => match config.profile.get(name) {
                Some(profile) => Ok(profile.role_arn.clone()),
                None => Err(anyhow!("{} is not found", name)),
            },
            None => Ok(self.select_role_arn(&config)),
        }
    }

    fn select_role_arn(&self, config: &Config) -> String {
        let options = SkimOptionsBuilder::default()
            .bind(vec!["Enter::accept"])
            .build()
            .unwrap();
        let (tx_item, rx_item): (SkimItemSender, SkimItemReceiver) = unbounded();
        for (name, profile) in &config.profile {
            let item = Item {
                label: format!("{:<30}\t{}", name, profile.role_arn),
                role_arn: profile.role_arn.clone(),
            };
            let _ = tx_item.send(Arc::new(item));
        }
        drop(tx_item);

        let selected_items = Skim::run_with(&options, Some(rx_item)).map(|out| match out.final_key {
            Key::Enter => out.selected_items,
            _ => vec![],
        });
        println!("");
        selected_items.unwrap().get(0).unwrap().output().as_ref().to_string()
    }
}

impl SkimItem for Item {
    fn text(&self) -> Cow<str> {
        Cow::Borrowed(&self.label)
    }

    fn output(&self) -> Cow<str> {
        Cow::Borrowed(&self.role_arn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::eq;
    use sts::types::AssumedRoleUser;

    #[tokio::test]
    async fn test_get_caller_identity() {
        let cli = Cli::parse_from(["assume-role", "--serial-number=test_serial_number"]);
        let mut mock = MockStsImpl::default();
        mock.expect_get_caller_identity().return_once(|| {
            Ok(GetCallerIdentityOutput::builder()
                .user_id("test-user")
                .account("123456789012")
                .arn("arn:aws:iam:123456789012:user/test-user")
                .build())
        });
        let result = cli.get_caller_identity(&mock).await;
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            format!(
                "UserId:  test-user\n\
                 Account: 123456789012\n\
                 Arn:     arn:aws:iam:123456789012:user/test-user"
            )
        );
    }

    #[tokio::test]
    async fn test_assume_role() {
        let cli = Cli::parse_from([
            "assume-role",
            "--serial-number=test_serial_number",
            "--role-arn=test-role",
            "--totp-code=123456",
        ]);
        let mut mock = MockStsImpl::default();
        mock.expect_assume_role()
            .with(
                eq("test-role".to_string()),
                eq(3600),
                eq("test_serial_number".to_string()),
                eq("123456".to_string()),
            )
            .return_once(|role, _duration, _, _| {
                let timestamp = DateTime::parse_from_rfc3339("2024-05-15T20:00:00Z")
                    .unwrap()
                    .to_utc()
                    .timestamp();
                let expiration = sts::primitives::DateTime::from_secs(timestamp);

                Ok(AssumeRoleOutput::builder()
                    .assumed_role_user(
                        AssumedRoleUser::builder()
                            .assumed_role_id(role)
                            .arn("arn:iam:::user/test-assumed-user")
                            .build()
                            .context("failed to build AssumedRoleUser")?,
                    )
                    .credentials(
                        sts::types::Credentials::builder()
                            .access_key_id("test_access_key_id")
                            .secret_access_key("test_secret_access_key")
                            .session_token("test_session_token")
                            .expiration(expiration)
                            .build()
                            .context("Failed to build Credentials")?,
                    )
                    .build())
            });

        let result = cli.assume_role(&mock).await;
        assert!(result.is_ok());
        let credentials = result.unwrap();
        assert_eq!("test_access_key_id", credentials.access_key_id());
        assert_eq!("test_secret_access_key", credentials.secret_access_key());
        assert_eq!("test_session_token", credentials.session_token());
    }
}
