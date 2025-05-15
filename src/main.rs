mod alpm;
mod aur;
mod build;
mod config;
mod error;
mod logging;

use clap::{Parser, Subcommand};
use log::{info, debug};
use chrono::{TimeZone, Utc};
use anyhow::Context;
use colored::Colorize;
use crate::error::AurError;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Search { query: String },
    Install { package: String },
    Info { package: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    logging::init_logger();
    let config = config::AppConfig::load()?;
    debug!("{}\n", "Configuration loaded".bright_green());

    let aur = aur::AurClient::new(config.aur_base_url.clone());
    let alpm = alpm::AlpmWrapper::new()?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Search { query } => {
            info!("Searching for: {}", query);
            let results = aur.search_packages(&query).await?;
            for pkg in results {
                println!("Name: {}", pkg.name);
                println!("Version: {}", pkg.version);
                if let Some(desc) = pkg.description {
                    println!("Description: {}", desc);
                }
                if let Some(url) = pkg.url {
                    println!("URL: {}", url);
                }
                if let Some(maintainer) = pkg.maintainer {
                    println!("Maintainer: {}", maintainer);
                }
                println!("--------------------");
            }
        }
        Commands::Install { package } => {
            info!(
                "\n{} {}",
                "Attempting to install package:".white(),
                package.bright_green()
            );

            info!("{} {}", "Checking AUR for package:".white(), package.bright_green());
            match aur.get_package_info(&package).await {
                Ok(pkg) => info!("{} {} {}", "Found package:".white(), pkg.name.bright_green(), format!("({})", pkg.version).bright_cyan()),
                Err(AurError::NotFound(_)) => {
                    eprintln!("\n{} {}\n", "✗ Package not found in AUR:".red().bold(), package.bright_red());
                    return Ok(());
                }
                Err(e) => return Err(e.into()),
            }

            match alpm.is_package_installed(&package) {
                Ok(true) => {
                    println!(
                        "\n{} {} {}\n",
                        "Package".white(),
                        package.bright_green(),
                        "is already installed".white()
                    );
                    return Ok(());
                }
                Ok(false) => {
                    info!(
                        "\n{} {} {}\n",
                        "Package".white(),
                        package.bright_green(),
                        "is not installed, proceeding with AUR installation".white()
                    );
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(e).context("Failed to check if package is installed"));
                }
            }

            info!(
                "{} {}",
                "Fetching package info for:".white(),
                package.bright_green()
            );

            let build_dir = config.temp_path().join(&package);
            build::PackageBuilder::clone_repo(&package, &build_dir)
                .context(format!("Failed to clone repository for {}", package))?;

            let package_path = build::PackageBuilder::build_package_with_deps(
                &package,
                &build_dir,
                &aur,
                &alpm,
            ).await
            .context(format!("Failed to build package {} with dependencies", package))?;

            alpm.install_package(&package_path)
                .context(format!("Failed to install package {}", package))?;
        }
        Commands::Info { package } => {
            let pkg_info = aur.get_package_info(&package).await?;
            println!("Package: {}", pkg_info.name);
            println!("Version: {}", pkg_info.version);
            if let Some(desc) = pkg_info.description {
                println!("Description: {}", desc);
            }
            if let Some(url) = pkg_info.url {
                println!("URL: {}", url);
            }
            if let Some(maintainer) = pkg_info.maintainer {
                println!("Maintainer: {}", maintainer);
            }
            println!("Votes: {}", pkg_info.num_votes);
            println!("Popularity: {}", pkg_info.popularity);
            let first_submitted_dt = Utc.timestamp_opt(pkg_info.first_submitted as i64, 0).unwrap();
            let last_modified_dt = Utc.timestamp_opt(pkg_info.last_modified as i64, 0).unwrap();
            println!("First Submitted: {}", first_submitted_dt.format("%Y-%m-%d %H:%M:%S UTC"));
            println!("Last Modified: {}", last_modified_dt.format("%Y-%m-%d %H:%M:%S UTC"));
        }
    }

    Ok(())
}
