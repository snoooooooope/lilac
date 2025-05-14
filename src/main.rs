mod alpm;
mod aur;
mod build;
mod config;
mod error;
mod logging;

use clap::{Parser, Subcommand};
use log::info;
use chrono::{TimeZone, Utc};
use anyhow::Context;

/// CLI command structure
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Available subcommands
#[derive(Subcommand)]
enum Commands {
    /// Search for packages in AUR
    Search {
        /// Search query
        query: String,
    },
    /// Install a package from AUR
    Install {
        /// Package name to install
        package: String,
    },
    /// Show package information
    Info {
        /// Package name to show info for
        package: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging and config
    logging::init_logger();
    let config = config::AppConfig::load()?;
    info!("Configuration loaded");

    // Initialize clients
    let aur = aur::AurClient::new(config.aur_base_url.clone());
    let alpm = alpm::AlpmWrapper::new()?;

    // Parse CLI arguments
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
                println!("--------------------"); // Separator for clarity
            }
        }
        Commands::Install { package } => {
            info!("Attempting to install package: {}", package);

            // Check if already installed
            match alpm.is_package_installed(&package) {
                Ok(true) => {
                    println!("Package {} is already installed", package);
                    return Ok(());
                }
                Ok(false) => {
                    info!("Package {} is not installed, proceeding with AUR installation", package);
                }
                Err(e) => {
                    // A genuine error occurred while querying the local database
                    return Err(anyhow::anyhow!(e).context("Failed to check if package is installed"));
                }
            }

            // Get package info from AUR
            let pkg_info = aur.get_package_info(&package).await
                .context(format!("Failed to get package info for {}", package))?;

            println!("Building and installing {} version {}", pkg_info.name, pkg_info.version);

            // Clone repository
            let build_dir = config.temp_path().join(&package);
            build::PackageBuilder::clone_repo(&package, &build_dir)
                .context(format!("Failed to clone repository for {}", package))?;
            info!("Cloned repository to {:?}", build_dir);

            // Execute makepkg to build the package
            let package_path = build::PackageBuilder::execute_makepkg(&build_dir)
                .context(format!("Failed to build package {}", package))?;
            info!("Built package file at {:?}", package_path);

            // Install the package using alpm
            alpm.install_package(&package_path)
                .context(format!("Failed to install package {}", package))?;

            println!("Successfully installed {}", package);

            // Temporary directory cleanup is handled by the config object dropping
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
