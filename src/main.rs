use anyhow::{anyhow, bail, ensure, Context, Result};
use aws_config::BehaviorVersion;
use aws_sdk_sts as sts;
use chrono::{DateTime, Local, SecondsFormat};
use clap::{Parser, ValueEnum};
use regex::Regex;
use serde::Deserialize;
use skim::prelude::*;
use skim::{Skim, SkimItemReceiver, SkimItemSender};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::Command;
use totp_rs::{Algorithm, Secret, TOTP};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// The profile name
    #[arg(short, long)]
    profile_name: Option<String>,
    /// The config file. default: $HOME/.aws/config.toml
    #[arg(short, long)]
    config: Option<String>,
    /// The duration, in seconds, of the role session. (900-43200)
    #[arg(short, long, default_value = "1h")]
    duration: String,
    /// MFA device ARN such as arn:aws:iam::123456789012/mfa/user
    #[arg(short, long, env)]
    serial_number: String,
    /// The base32 format TOTP secret
    #[arg(short, long, env)]
    totp_secret: Option<String>,
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

#[::tokio::main]
async fn main() {
    let cli = Cli::parse();
    cli.execute().await.unwrap();
}

impl<'a> Cli {
    async fn execute(&self) -> Result<()> {
        let config = aws_config::defaults(BehaviorVersion::latest())
            .retry_config(aws_config::retry::RetryConfig::standard().with_max_attempts(3))
            .load().await;
        let now = Local::now().timestamp_millis();

        let sts = sts::Client::new(&config);
        if self.verbose {
            let response = sts.get_caller_identity().send().await?;
            println!("UserId:  {}", response.user_id().unwrap_or_default());
            println!("Account: {}", response.account().unwrap_or_default());
            println!("Arn:     {}", response.arn().unwrap_or_default());
        }

        // TODO: retry
        let response = sts
            .assume_role()
            .role_session_name(format!("{}-session", now))
            .role_arn(self.role_arn().unwrap())
            .duration_seconds(self.duration_seconds().context("Invalid duration")?)
            .serial_number(&self.serial_number)
            .token_code(self.totp_code().context("Unable to generate TOTP code")?)
            .send()
            .await;
        
        match response {
            Ok(output) => match output.credentials() {
                Some(credentials) => {
                    let dt = DateTime::from_timestamp_millis(credentials.expiration().to_millis()?).context("")?;
                    let envs = HashMap::from([
                        ("AWS_ACCESS_KEY_ID", credentials.access_key_id.clone()),
                        ("AWS_SECRET_ACCESS_KEY", credentials.secret_access_key.clone()),
                        ("AWS_SESSION_TOKEN", credentials.session_token.clone()),
                        ("AWS_EXPIRATION", dt.to_rfc3339_opts(SecondsFormat::Millis, false)),
                    ]);
                    match &self.format {
                        Some(format) => self.output(format, &envs)?,
                        None => self.exec_command(&envs)?,
                    };
                },
                None => bail!("Unable to fetch temporary credentials")
            },
            Err(e) => bail!("Unable to assume role: {}", e)
        };
        Ok(())
    }

    fn output(&self, format: &Format, envs: &HashMap<&str, String>) -> Result<()> {
        match format {
            Format::Json => println!("{}", serde_json::to_string(envs)?),
            Format::Bash | Format::Zsh => envs.iter().for_each(|(k, v)| println!(r#"export {}="{}""#, k, v)),
            Format::Fish => envs.iter().for_each(|(k, v)| println!(r#"set -gx {} "{}""#, k, v)),
            Format::PowerShell => envs.iter().for_each(|(k,v)| println!(r#"$env:{}="{}""#, k, v)),
        }
        Ok(())
    }

    fn exec_command(&self, envs: &HashMap<&str, String>) -> Result<()> {
        let (exe, args) = self.args.split_at(1);
        Command::new(exe[0].clone()).args(args).envs(envs).exec();
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
        let secret = match self.totp_secret.clone() {
            Some(s) => Secret::Encoded(s).to_bytes().unwrap(),
            None => bail!("TOTP_SECRET is required"),
        };
        let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, secret).unwrap();
        Ok(totp.generate_current().unwrap())
    }

    fn role_arn(&self) -> Result<String> {
        let mut toml_str = String::new();
        let mut io = match &self.config {
            Some(path) => File::open(path).unwrap(),
            None => {
                let home_dir = dirs::home_dir().context("Unable to get home directory")?;
                File::open(home_dir.join(".aws/config.toml"))
                    .context("Unable to read $HOME/.aws/config.toml")?
            }
        };
        io.read_to_string(&mut toml_str)
            .context("Unable to read config file")?;
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

        let selected_items =
            Skim::run_with(&options, Some(rx_item)).map(|out| match out.final_key {
                Key::Enter => out.selected_items,
                _ => vec![],
            });
        println!("");
        selected_items
            .unwrap()
            .get(0)
            .unwrap()
            .output()
            .as_ref()
            .to_string()
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
