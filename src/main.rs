mod alpm;
mod aur;
mod build;
mod config;
mod error;
mod logging;

use clap::{Parser, Subcommand};
use log::info;

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
                println!("{} - {}", pkg.name, pkg.version);
                if let Some(desc) = pkg.description {
                    println!("  {}", desc);
                }
            }
        }
        Commands::Install { package } => {
            info!("Installing package: {}", package);
            
            // Check if already installed
            if alpm.is_package_installed(&package)? {
                println!("Package {} is already installed", package);
                return Ok(());
            }

            // Get package info
            let pkg_info = aur.get_package_info(&package).await?;
            println!("Installing {} version {}", pkg_info.name, pkg_info.version);

            // Build and install
            let build_dir = config.temp_path().join(&package);
            build::PackageBuilder::clone_repo(&package, &build_dir)?;
            build::PackageBuilder::execute_makepkg(&build_dir)?;
            alpm.install_package(&build_dir.join(format!("{}-*.pkg.tar.zst", package)))?;
            build::PackageBuilder::clean_build_artifacts(&build_dir)?;

            println!("Successfully installed {}", package);
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
        }
    }

    Ok(())
}
