use aws_assume_role::cli::Cli;
use clap::Parser;


#[::tokio::main]
async fn main() {
    let cli = Cli::parse();
    cli.execute().await.unwrap();
}
