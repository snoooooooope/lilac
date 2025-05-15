use clap::{Parser, Subcommand};
use log::{info, debug};
use chrono::{TimeZone, Utc};
use anyhow::Context;
use colored::Colorize;
use lilac::{
    AlpmWrapper,
    AurClient,
    PackageBuilder,
    AppConfig,
    error::{AlpmError, AurError},
    init_logger,
};
use tempfile::tempdir;
use std::fs;
use versions::Version;

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
    Info {
        package: String,
        #[arg(long)]
        deps: bool,
    },
    Remove { package: String },
    List,
    Update { package: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logger();
    let config = AppConfig::load()?;
    debug!("{}\n", "Configuration loaded".bright_green());

    let aur = AurClient::new(config.aur_base_url.clone());
    let alpm = AlpmWrapper::new()?;

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
            PackageBuilder::clone_repo(&package, &build_dir)
                .context(format!("Failed to clone repository for {}", package))?;

            let package_path = PackageBuilder::build_package_with_deps(
                &package,
                &build_dir,
                &config,
            ).await
            .context(format!("Failed to build package {} with dependencies", package))?;

            alpm.install_package(&package_path)
                .context(format!("Failed to install package {}", package))?;
        }
        Commands::Info { package, deps } => {
            let pkg_info = aur.get_package_info(&package).await
                .map_err(|e| {
                    eprintln!("\n{} {}", "✗ Failed to fetch AUR info:".red().bold(), e);
                    anyhow::anyhow!(e).context(format!("Failed to get AUR package info for {}", package))
                })?;

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

            if deps {
                let temp_dir = tempdir()
                     .map_err(|e| {
                        eprintln!("\n{} {}", "✗ Failed to create temporary directory:".red().bold(), e);
                        anyhow::anyhow!(e).context("Failed to create temporary directory")
                     })?;
                let build_dir = temp_dir.path().join(&package);

                match PackageBuilder::clone_repo(&package, &build_dir) {
                    Ok(_) => {
                         match PackageBuilder::get_dependencies_from_srcinfo(&build_dir) {
                             Ok(dependencies) => {
                                 if !dependencies.is_empty() {
                                     println!("{}:", "Dependencies".bold());
                                     for dep in dependencies {
                                         println!("  - {}", dep.bright_green());
                                     }
                                 } else {
                                      println!("{}: {}\n", "Dependencies".bold(), "None found".bright_green());
                                 }
                             }
                             Err(e) => {
                                eprintln!("{} {}\n", "✗ Failed to extract dependencies:".red().bold(), e);
                             }
                         }
                    }
                    Err(e) => {
                        eprintln!("{} {}\n", "✗ Failed to clone repository for dependency info:".red().bold(), e);
                    }
                }
            }
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

                    // Delete the package from the cache
                    let cache_dir = config.cache_path()?;
                    PackageBuilder::delete_cached_package(&cache_dir, &package)
                        .context("Failed to delete cached package")?;
                }
                Err(AlpmError::NotFound(_)) => {
                    eprintln!("\n{} {}\n", "✗ Package not found in system:".red().bold(), package.bright_red());
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(e).context("Failed to check if package is installed"));
                }
                Ok(false) => {
                    eprintln!("{} {}", "✗ Package not found in system:".red().bold(), package.bright_red());
                }
            }
        }
        Commands::List => {
            let cache_dir = config.cache_path()?;
            let entries = fs::read_dir(&cache_dir)
                .context("Failed to read cache directory")?;

            let mut packages_info = Vec::new();
            for entry in entries {
                let entry = entry.context("Failed to read cache directory entry")?;
                let path = entry.path();
                if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
                    if file_name.ends_with(".pkg.tar.zst") || file_name.ends_with(".pkg.tar.xz") {    
                        // Remove the file extension first
                        let file_name_without_ext = file_name
                            .strip_suffix(".pkg.tar.zst")
                            .or_else(|| file_name.strip_suffix(".pkg.tar.xz"))
                            .unwrap_or(file_name);

                        // Find the index of the first hyphen followed by a digit (marks start of version)
                        let version_start_index = file_name_without_ext.char_indices()
                            .find(|(i, c)| {
                                *c == '-' && file_name_without_ext.chars().nth(i + 1).map_or(false, |next_c| next_c.is_digit(10))
                            })
                            .map(|(i, _)| i);

                        let (package_name, version) = if let Some(index) = version_start_index {
                            let name_part = &file_name_without_ext[..index];
                            let version_with_arch = &file_name_without_ext[index + 1..];
                            let arch_start_index = version_with_arch.rfind('-');
                            let version_only = if let Some(arch_idx) = arch_start_index {
                                version_with_arch[..arch_idx].to_string()
                            } else {
                                version_with_arch.to_string()
                            };
                            (name_part.to_string(), version_only)
                        } else {
                             // Fallback if version structure is unexpected
                             (file_name_without_ext.to_string(), "unknown".to_string())
                        };

                        // Format the output string
                        packages_info.push(format!("{} ({})", package_name, version));
                    }
                }
            }

            // Sort the output alphabetically by package name
            packages_info.sort();


            if packages_info.is_empty() {
                println!("\n{}", "No packages installed via lilac found in cache.".bold());
            } else {
                println!("\n{}", "Packages installed via lilac:".bold());
                for pkg_info in packages_info {
                    println!("  - {}\n", pkg_info.bright_green());
                }
            }
        }
        Commands::Update { package } => {
            println!(
                "\n{} {}",
                "Checking for updates for package:".bold(),
                package.bright_green()
            );

            // Fetch latest package info from AUR
            let latest_pkg = aur.get_package_info(&package).await
                .context("Failed to fetch latest package info from AUR")?;

            // Check if the package is installed
            match alpm.is_package_installed(&package) {
                Ok(true) => {
                    println!(
                        "{} {} {}",
                        "Package".bold(),
                        package.bright_green(),
                        "is installed, checking for updates...".bold()
                    );
                }
                Err(AlpmError::NotFound(_)) => {
                    eprintln!("\n{} {}\n", "✗ Package not found in system:".red().bold(), package.bright_red());
                    return Ok(());
                }
                Err(e) => return Err(e.into()),
                Ok(false) => {
                    eprintln!("\n{} {}\n", "✗ Package not installed:".red().bold(), package.bright_red());
                    return Ok(());
                }
            }

            // Helper function to extract version from filename
            fn extract_version_from_filename(file_name: &str, package_name: &str) -> Option<String> {
                let stripped = file_name.strip_prefix(package_name)?;  // Remove package name
                let parts: Vec<&str> = stripped.split('-').collect();
                if parts.len() >= 3 {
                    Some(format!("{}-{}", parts[1], parts[2]))
                } else {
                    None
                }
            }

            // Get the cached package version
            let cache_dir = config.cache_path()?;
            let cached_pkg = PackageBuilder::find_cached_package(&cache_dir, &package);
            let cached_version = match cached_pkg {
                Some(path) => {
                    let file_name = path.file_name().unwrap().to_str().unwrap();
                    extract_version_from_filename(file_name, &package).unwrap_or_else(|| {
                        println!("{}", "✗ Failed to extract version from cached filename.".red().bold());
                        "unknown".to_string()
                    })
                }
                None => "unknown".to_string(),
            };

            println!(
                "{}: {} (cached) vs {} (latest)",
                "Version comparison".bold(),
                cached_version.bright_cyan(),
                latest_pkg.version.bright_green()
            );

            // Compare normalized versions
            let cached_ver = Version::new(&cached_version);
            let latest_ver = Version::new(&latest_pkg.version);

            if cached_ver < latest_ver {
                println!(
                    "{} {} {}",
                    "Updating package:".bold(),
                    package.bright_green(),
                    format!("(from {} to {})", cached_version, latest_pkg.version).bright_cyan()
                );

                // Rebuild and reinstall
                let build_dir = config.temp_path().join(&package);
                PackageBuilder::clone_repo(&package, &build_dir)
                    .context("Failed to clone repository for update")?;

                let package_path = PackageBuilder::build_package_with_deps(
                    &package,
                    &build_dir,
                    &config,
                ).await
                .context("Failed to rebuild package")?;

                alpm.remove_package(&package)
                    .context("Failed to remove old package")?;

                alpm.install_package(&package_path)
                    .context("Failed to install updated package")?;

                println!("\n{}", "✓ Update completed successfully!".green().bold());
            } else {
                println!(
                    "\n{} {} {}",
                    "Package".bold(),
                    package.bright_green(),
                    "is already up to date.\n".bold()
                );
            }
        }
    }

    Ok(())
}
