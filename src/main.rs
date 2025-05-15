use lilac::{
    AlpmWrapper,
    AurClient,
    AppConfig,
    init_logger,
    commands::{Commands, handle_command}
};

use clap::Parser;
use log::debug;
use colored::Colorize;


#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logger();
    let config = AppConfig::load()?;
    debug!("{}\n", "Configuration loaded".bright_green());

    let aur = AurClient::new(config.aur_base_url.clone());
    let alpm = AlpmWrapper::new()?;

    let cli = Cli::parse();

    handle_command(cli.command, &config, &aur, &alpm).await?;

    Ok(())
}
