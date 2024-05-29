use aws_assume_role::cli::Cli;
use aws_config::BehaviorVersion;
use aws_runtime::env_config::file::{EnvConfigFileKind, EnvConfigFiles};
use aws_sdk_sts as sts;
use clap::error::ErrorKind;
use clap::{CommandFactory, Parser};

#[::tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(e) = cli.validate_arguments() {
        e.exit();
    }

    let loader = aws_config::defaults(BehaviorVersion::latest());
    let loader = match cli.aws_profile.clone() {
        Some(profile_name) => loader.profile_name(profile_name),
        None => loader,
    };
    let loader = match cli.config.clone() {
        Some(config_path) if config_path.extension() == None => {
            let profile_files = EnvConfigFiles::builder()
                .with_file(EnvConfigFileKind::Config, config_path)
                .build();
            loader.profile_files(profile_files)
        }
        Some(_) => loader,
        None => loader,
    };
    let config = loader
        .retry_config(aws_config::retry::RetryConfig::standard().with_max_attempts(3))
        .load()
        .await;
    let sts = sts::Client::new(&config);

    if let Err(e) = cli.execute(sts).await {
        let mut cmd = Cli::command();
        cmd.error(ErrorKind::Io, e.to_string()).exit();
    }
}
