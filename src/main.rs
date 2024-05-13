use aws_assume_role::cli::Cli;
use clap::Parser;
use aws_config::BehaviorVersion;
use aws_sdk_sts as sts;


#[::tokio::main]
async fn main() {
    let cli = Cli::parse();

    let config = aws_config::defaults(BehaviorVersion::latest())
        .retry_config(aws_config::retry::RetryConfig::standard().with_max_attempts(3))
        .load()
        .await;
    let sts = sts::Client::new(&config);

    cli.execute(&sts).await.unwrap();
}
