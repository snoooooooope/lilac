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
use crate::error::{AurError, AlpmError};

#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Search { query: String },
    Install { package: String },
    Info { package: String },
    Remove { package: String },
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
            info!("\n{}: {}", "Searching for".bold(), query.bright_green());
            let results = aur.search_packages(&query).await?;
            for pkg in results {
                println!("\n{}: {}", "Name".bold(), pkg.name.bright_green());
                println!("{}: {}", "Version".bold(), pkg.version.bright_cyan());
                if let Some(desc) = pkg.description {
                    println!("{}: {}", "Description".bold(), desc);
                }
                if let Some(url) = pkg.url {
                    println!("{}: {}", "URL".bold(), url);
                }
                if let Some(maintainer) = pkg.maintainer {
                    println!("{}: {}", "Maintainer".bold(), maintainer);
                }
            }
        }
        Commands::Install { package } => {
            println!(
                "\n{} {}",
                "Attempting to install package:".bold(),
                package.bright_green()
            );

            println!("{} {}", "Checking AUR for package:".bold(), package.bright_green());
            match aur.get_package_info(&package).await {
                Ok(pkg) => println!("{} {} {}", "Found package:".bold(), pkg.name.bright_green(), format!("({})", pkg.version).bright_cyan()),
                Err(AurError::NotFound(_)) => {
                    eprintln!("\n{} {}", "✗ Package not found in AUR:".red().bold(), package.bright_red());
                    return Ok(());
                }
                Err(e) => return Err(e.into()),
            }

            match alpm.is_package_installed(&package) {
                Ok(true) => {
                    println!(
                        "\n{} {} {}\n",
                        "Package".bold(),
                        package.bright_green(),
                        "is already installed"
                    );
                    return Ok(());
                }
                Err(AlpmError::NotFound(_)) => {
                    println!(
                        "\n{} {} {}",
                        "Package".bold(),
                        package.bright_green(),
                        "is not installed, proceeding with AUR installation".bold()
                    );
                }
                Ok(false) => {
                    return Err(anyhow::anyhow!("Unexpected result from is_package_installed: Ok(false)"));
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(e).context("Failed to check if package is installed"));
                }
            }

            println!(
                "{} {}",
                "Fetching package info for:".bold(),
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
            println!("{}: {}", "\nPackage".bold(), pkg_info.name.green());
            println!("{}: {}", "Version".bold(), pkg_info.version.bright_cyan());
            if let Some(desc) = pkg_info.description {
                println!("{}: {}", "Description".bold(), desc);
            }
            if let Some(url) = pkg_info.url {
                println!("{}: {}", "URL".bold(), url);
            }
            if let Some(maintainer) = pkg_info.maintainer {
                println!("{}: {}", "Maintainer".bold(), maintainer);
            }
            println!("{}: {}", "Votes".bold(), pkg_info.num_votes);
            println!("{}: {}", "Popularity".bold(), pkg_info.popularity);
            let first_submitted_dt = Utc.timestamp_opt(pkg_info.first_submitted as i64, 0).unwrap();
            let last_modified_dt = Utc.timestamp_opt(pkg_info.last_modified as i64, 0).unwrap();
            println!("{}: {}", "First Submitted".bold(), first_submitted_dt.format("%m/%d/%Y"));
            println!("{}: {}", "Last Modified".bold(), last_modified_dt.format("%m/%d/%Y"));
        }
        Commands::Remove { package } => {
            match alpm.is_package_installed(&package) {
                Ok(true) => {
                    println!(
                        "\n{} {} {}",
                        "Package".bold(),
                        package.bright_green(),
                        "is installed, proceeding with removal".bold()
                    );
                    alpm.remove_package(&package)
                        .context(format!("Failed to remove package {}", package))?;
                }
                Ok(false) => {
                    eprintln!("\n{} {}", "✗ Package not found in system:".red().bold(), package.bright_red());
                    return Ok(());
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(e).context("Failed to check if package is installed"));
                }
            }
        }
    }

    Ok(())
}
