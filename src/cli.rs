use anyhow::{anyhow, bail, ensure, Context, Result};
use aws_sdk_sts as sts;
use backon::{ExponentialBuilder, Retryable};
use chrono::{DateTime, Local, SecondsFormat};
use clap::error::ErrorKind;
use clap::{Args, CommandFactory, Parser, ValueEnum};
use ini::Ini;
use regex::Regex;
use serde::Deserialize;
use skim::prelude::*;
#[allow(unused_imports)]
use skim::{Skim, SkimItemReceiver, SkimItemSender};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
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
        role_arn: Option<String>,
        duration_seconds: Option<i32>,
        serial_number: Option<String>,
        token_code: Option<String>,
    ) -> Result<AssumeRoleOutput> {
        let now = Local::now().timestamp_millis();
        self.inner
            .assume_role()
            .set_role_session_name(Some(format!("{}-session", now)))
            .set_role_arn(role_arn)
            .set_duration_seconds(duration_seconds)
            .set_serial_number(serial_number)
            .set_token_code(token_code)
            .send()
            .await
            .context("Failed to call assume_role")
    }
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// AWS profile name in AWS_CONFIG_FILE.
    /// This option is used to detect jump account information.
    #[arg(long, env)]
    pub aws_profile: Option<String>,
    /// The profile name
    #[arg(short, long)]
    profile_name: Option<String>,

    /// The IAM Role ARN to assume
    #[arg(short, long, env, conflicts_with_all = ["profile_name", "config"])]
    role_arn: Option<String>,

    /// The config file. default: $HOME/.aws/config.toml
    /// Load the first of the following files found:
    ///   1. the file specified by this option
    ///   2. $HOME/.aws/config.toml
    ///   3. $HOME/.aws/config
    #[arg(short, long, verbatim_doc_comment)]
    pub config: Option<PathBuf>,

    /// The duration in seconds of the role session. (900-43200)
    /// The following suffixes are available:
    ///   "s": seconds
    ///   "m": minutes
    ///   "h": hours
    /// No suffix means seconds.
    #[arg(short, long, default_value = "1h", value_parser = parse_duration, verbatim_doc_comment)]
    duration: i32,

    /// MFA device ARN such as arn:aws:iam::123456789012/mfa/user
    #[arg(short = 'n', long, env)]
    serial_number: Option<String>,

    #[command(flatten)]
    totp_args: TotpArgs,

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

#[derive(Args)]
#[group(required = false, multiple = false)]
struct TotpArgs {
    /// The base32 format TOTP secret
    #[arg(short = 's', long, env)]
    totp_secret: Option<String>,

    /// The TOTP code generated by other tool
    #[arg(short, long, env)]
    totp_code: Option<String>,
}

#[derive(Clone, Debug, ValueEnum)]
enum Format {
    Json,
    Bash,
    Zsh,
    Fish,
    PowerShell,
}

fn parse_duration(s: &str) -> Result<i32> {
    let re = Regex::new(r"(\d+)(s|m|h)?").unwrap();
    let duration = match re.captures(s) {
        Some(caps) => match (caps[1].parse::<i32>(), caps.get(2)) {
            (Ok(amount), Some(m)) if m.as_str() == "s" => amount,
            (Ok(amount), Some(m)) if m.as_str() == "m" => amount * 60,
            (Ok(amount), Some(m)) if m.as_str() == "h" => amount * 60 * 60,
            (Ok(amount), None) => amount,
            (Ok(_), Some(_)) => bail!("Unexpected {}", s),
            (Err(e), _) => bail!("Failed to parse duration: {} {:?}", s, e),
        },
        None => bail!("Failed to parse duration: {}", s),
    };
    ensure!(
        duration >= 900 && duration <= 43200,
        "duration ({}) must be between 900 seconds (15 minutes) and 43200 seconds (12 hours)",
        s
    );
    Ok(duration)
}

#[derive(Debug, Deserialize)]
struct Config {
    profile: HashMap<String, Profile>,
}

#[derive(Debug, Deserialize)]
struct Profile {
    role_arn: String,
}

#[allow(dead_code)]
struct Item {
    label: String,
    role_arn: String,
}

impl<'a> Cli {
    pub fn validate_arguments(&self) -> Result<(), clap::Error> {
        if self.aws_profile.is_none()
            && self.config.is_none()
            && self.profile_name.is_none()
            && self.role_arn.is_none()
            && self.serial_number.is_none()
            && self.totp_args.totp_code.is_none()
            && self.totp_args.totp_secret.is_none()
        {
            let mut cmd = Self::command();
            let err = cmd
                .error(ErrorKind::MissingRequiredArgument, "Required arguments are missing")
                .apply();
            Err(err)
        } else if self.serial_number.is_some()
            && self.totp_args.totp_code.is_none()
            && self.totp_args.totp_secret.is_none()
        {
            let mut cmd = Self::command();
            let err = cmd
                .error(
                    ErrorKind::MissingRequiredArgument,
                    "Require one of --totp-code or --totp-secret if set --serial-number-",
                )
                .apply();
            Err(err)
        } else {
            Ok(())
        }
    }

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
        let output = (|| async {
            sts.assume_role(
                Some(self.role_arn()?),
                Some(self.duration),
                self.serial_number().ok(),
                self.totp_code().ok(),
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

    fn serial_number(&self) -> Result<String> {
        if let Some(serial_number) = self.serial_number.clone() {
            return Ok(serial_number);
        }

        if let (Some(aws_profile_name), Some(config_path)) = (self.aws_profile.clone(), self.config.clone()) {
            if config_path.extension() == None {
                return self.serial_number_from_ini(&config_path, &aws_profile_name);
            }
        }

        if let Some(aws_profile_name) = self.aws_profile.clone() {
            let home_dir = dirs::home_dir().context("Unable to get home directory")?;
            let path = home_dir.join(".aws/config").canonicalize();
            if let Ok(path) = path {
                return self.serial_number_from_ini(&path, &aws_profile_name);
            }
        }

        bail!("Unable to get serial number");
    }

    fn serial_number_from_ini(&self, path: &PathBuf, aws_profile_name: &str) -> Result<String> {
        let ini = Ini::load_from_file(path).context("Unable to parse ini")?;
        let serial_number = ini
            .get_from(Some(format!("profile {}", aws_profile_name)), "serial_number")
            .with_context(|| format!("serial_number is missing for profile {}", aws_profile_name))?;
        Ok(serial_number.to_string())
    }

    fn totp_code(&self) -> Result<String> {
        if let Some(totp_code) = self.totp_args.totp_code.clone() {
            return Ok(totp_code);
        }
        let secret = match self.totp_args.totp_secret.clone() {
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

        let config = self.config_from_path(&self.config).context("Unable to load config")?;
        match &self.profile_name {
            Some(name) => match config.profile.get(name) {
                Some(profile) => Ok(profile.role_arn.clone()),
                None => Err(anyhow!("--role-arn={} is not found", name)),
            },
            None => Ok(self.select_role_arn(&config)),
        }
    }

    fn config_from_path(&self, path: &Option<PathBuf>) -> Result<Config> {
        match path {
            Some(path) => match path.extension() {
                Some(ext) if ext == "toml" => self.config_from_toml(path),
                Some(ext) => bail!("Unsupported extension: {:?}", ext),
                None => self.config_from_ini(path),
            },
            None => {
                let home_dir = dirs::home_dir().context("Unable to get home directory")?;
                let path = home_dir
                    .join(".aws/config.toml")
                    .canonicalize()
                    .unwrap_or_else(|_| home_dir.join(".aws/config").canonicalize().unwrap());
                self.config_from_path(&Some(path))
            }
        }
    }

    fn config_from_toml(&self, path: &PathBuf) -> Result<Config> {
        let mut toml_str = String::new();
        let mut io = File::open(path).with_context(|| format!("Unable to open file {:?}", path))?;
        io.read_to_string(&mut toml_str).context("Unable to read config file")?;
        let config: Config = toml::from_str(&toml_str).context("Unable to parse config file")?;
        Ok(config)
    }

    fn config_from_ini(&self, path: &PathBuf) -> Result<Config> {
        let ini = Ini::load_from_file(path).context("Unable to parse ini")?;
        let profile = ini
            .sections()
            .filter(|section| section.is_some() && ini.get_from(Some(section.unwrap()), "role_arn").is_some())
            .flat_map(|item| {
                item.map(|key| {
                    let key_part = key.split(' ').collect::<Vec<_>>().last().unwrap().to_string();
                    let role_arn = ini.get_from(Some(key), "role_arn").unwrap().to_string();
                    (key_part, Profile { role_arn })
                })
            })
            .collect::<HashMap<String, Profile>>();
        Ok(Config { profile })
    }

    #[cfg(test)]
    fn select_role_arn(&self, _config: &Config) -> String {
        panic!("select_role_arn is interactive method, so cannot invoke if test. check arguments before debug.");
    }

    #[cfg(not(test))]
    fn select_role_arn(&self, config: &Config) -> String {
        let options = SkimOptionsBuilder::default()
            .bind(vec!["Enter::accept".to_string()])
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
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use sts::types::AssumedRoleUser;

    fn duration_range_error(d: &str) -> String {
        format!(
            "duration ({}) must be between 900 seconds (15 minutes) and 43200 seconds (12 hours)",
            d
        )
    }

    #[rstest]
    #[case::error_empty_string("", 0, "Failed to parse duration: ")]
    #[case::error_less_than_min_n("899", 899, duration_range_error("899"))]
    #[case::error_less_than_min_s("899s", 899, duration_range_error("899s"))]
    #[case::error_more_than_max_n("43201", 43201, duration_range_error("43201"))]
    #[case::error_more_than_max_s("43201s", 899, duration_range_error("43201s"))]
    #[case::error_less_than_min_m("14m", 840, duration_range_error("14m"))]
    #[case::error_more_than_max_m("721m", 840, duration_range_error("721m"))]
    #[case::error_more_than_max_h("13h", 840, duration_range_error("13h"))]
    #[case::success_1_hour("1h", 3600, "")]
    #[case::success_12_hours("12h", 43200, "")]
    #[case::success_15_minutes("15m", 900, "")]
    #[case::success_720_minutes("720m", 43200, "")]
    #[case::success_900_seconds("900s", 900, "")]
    #[case::success_43200_seconds("43200s", 43200, "")]
    #[case::success_900("900", 900, "")]
    #[case::success_43200("43200", 43200, "")]
    fn test_parse_duration(#[case] s: &str, #[case] expected: i32, #[case] message: String) -> Result<()> {
        match parse_duration(s) {
            Ok(actual) => assert_eq!(actual, expected),
            Err(e) => assert_eq!(e.to_string(), message),
        };
        Ok(())
    }

    #[tokio::test]
    async fn test_get_caller_identity() {
        let cli = Cli::parse_from([
            "assume-role",
            "--serial-number=test_serial_number",
            "--totp-code=123456",
        ]);
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
                eq(Some("test-role".to_string())),
                eq(Some(3600)),
                eq(Some("test_serial_number".to_string())),
                eq(Some("123456".to_string())),
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
                            .assumed_role_id(role.unwrap())
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

    #[tokio::test]
    async fn test_assume_role_with_role_only() {
        let cli = Cli::parse_from(["assume-role", "--role-arn=test-role"]);
        let mut mock = MockStsImpl::default();
        mock.expect_assume_role()
            .with(eq(Some("test-role".to_string())), eq(Some(3600)), eq(None), eq(None))
            .return_once(|role, _duration, _, _| {
                let timestamp = DateTime::parse_from_rfc3339("2024-05-15T20:00:00Z")
                    .unwrap()
                    .to_utc()
                    .timestamp();
                let expiration = sts::primitives::DateTime::from_secs(timestamp);

                Ok(AssumeRoleOutput::builder()
                    .assumed_role_user(
                        AssumedRoleUser::builder()
                            .assumed_role_id(role.unwrap())
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

    #[rstest]
    #[tokio::test]
    async fn test_assume_role_with_config_file(#[files("tests/fixtures/*")] path: PathBuf) {
        let cli = Cli::parse_from([
            "assume-role",
            "--serial-number=test_serial_number",
            "--totp-code=123456",
            "--duration=12h",
            "--format=json",
            "--config",
            path.to_str().unwrap(),
            "--profile-name=test",
        ]);
        let mut mock = MockStsImpl::default();
        mock.expect_assume_role()
            .with(
                eq(Some("arn:aws:iam::987654321234:role/TestUser".to_string())),
                eq(Some(3600 * 12)),
                eq(Some("test_serial_number".to_string())),
                eq(Some("123456".to_string())),
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
                            .assumed_role_id(role.unwrap())
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
        dbg!(&result);
        debug_assert!(result.is_ok());
        let credentials = result.unwrap();
        assert_eq!("test_access_key_id", credentials.access_key_id());
        assert_eq!("test_secret_access_key", credentials.secret_access_key());
        assert_eq!("test_session_token", credentials.session_token());
    }

    #[rstest]
    #[tokio::test]
    async fn test_assume_role_with_aws_profile(#[files("tests/fixtures/config")] path: PathBuf) {
        let cli = Cli::parse_from([
            "assume-role",
            "--aws-profile=jump",
            "--totp-code=123456",
            "--duration=12h",
            "--format=json",
            "--config",
            path.to_str().unwrap(),
            "--profile-name=test",
        ]);
        let mut mock = MockStsImpl::default();
        mock.expect_assume_role()
            .with(
                eq(Some("arn:aws:iam::987654321234:role/TestUser".to_string())),
                eq(Some(3600 * 12)),
                eq(Some("arn:aws:iam::123456789012:mfa/serialnumber".to_string())),
                eq(Some("123456".to_string())),
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
                            .assumed_role_id(role.unwrap())
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
        dbg!(&result);
        debug_assert!(result.is_ok());
        let credentials = result.unwrap();
        assert_eq!("test_access_key_id", credentials.access_key_id());
        assert_eq!("test_secret_access_key", credentials.secret_access_key());
        assert_eq!("test_session_token", credentials.session_token());
    }
}
