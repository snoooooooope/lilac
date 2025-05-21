use clap::Subcommand;
use anyhow::Context;
use colored::Colorize;
use log::info;
use std::fs;
use tempfile::tempdir;
use versions::Version;
use chrono::{Utc, TimeZone};

use crate::alpm::AlpmWrapper;
use crate::aur::AurClient;
use crate::build::PackageBuilder;
use crate::config::AppConfig;
use crate::error::{AlpmError, AurError, BuildError};

#[derive(Subcommand)]
pub enum Commands {
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

pub async fn handle_command(
    command: Commands,
    config: &AppConfig,
    aur: &AurClient,
    alpm: &AlpmWrapper,
) -> anyhow::Result<()> {
    match command {
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

            // First, check if the package is already installed
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
                Ok(false) => {
                    println!(
                        "\n{} {} {}",
                        "Package".bold(),
                        package.bright_green(),
                        "is not installed, proceeding with installation".bold()
                    );
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(e as AlpmError).context("Failed to check if package is installed"));
                }
            }

            let cache_dir = config.cache_path()?;

            let package_path_to_install = if let Some(cached_pkg) = PackageBuilder::find_cached_package(&cache_dir, &package) {
                println!(
                    "{} {} {}",
                    "Using cached package:".bold(),
                    package.bright_green(),
                    format!("({:?})", cached_pkg).bright_cyan()
                );
                // If package is cached, check and install its dependencies first
                println!(
                    "{} {}.{}",
                    "Checking dependencies for cached package:".bold(),
                    package.bright_green(),
                    "\n".bold()
                );
                match PackageBuilder::read_dependency_list(&package, &cache_dir) {
                    Ok(dependencies) => {
                        if !dependencies.is_empty() {
                            println!("{}", "Installing missing dependencies for cached package...".bold());
                            match PackageBuilder::install_dependencies(&dependencies, alpm, config) {
                                Ok(_) => {
                                    println!("{}", "✓ Dependencies for cached package installed successfully.".green().bold());
                                },
                                Err(e) => {
                                    return Err(anyhow::anyhow!(e).context(format!("Failed to install dependencies for cached package {}", package)));
                                }
                            }
                        } else {
                            println!("{}", "No tracked dependencies found for cached package.".bright_yellow());
                        }
                    },
                    Err(e) => {
                        eprintln!("{} {}", "Warning: Failed to read dependency list for cached package:".yellow().bold(), e);
                        println!("{}", "Proceeding with main package installation, but dependencies might be missing.".yellow());
                        // Continue even if reading dependency list fails, log a warning
                    }
                }
                cached_pkg // Return the path to the cached package for main installation
            } else {
                println!(
                    "{} {}",
                    "Fetching package info for:".bold(),
                    package.bright_green()
                );

                // Proceed with building if no cached package exists
                let build_dir = config.temp_path().join(&package);
                PackageBuilder::clone_repo(&package, &build_dir)
                    .context(format!("Failed to clone repository for {}", package))?;

                PackageBuilder::build_package_with_deps(
                    &package,
                    &build_dir,
                    &config,
                ).await
                .context(format!("Failed to build package {} with dependencies", package))?
            };

            // Install the specified package after build, either using cache or building from AUR
            alpm.install_package(&package_path_to_install)
                .context(format!("\nFailed to install package {}", package))?;
        }
        Commands::Info { package, deps } => {
            let pkg_info = aur.get_package_info(&package).await
                .map_err(|e: AurError| {
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
            println!("{}: {}\n", "Last Modified".bold(), last_modified_dt.format("%m/%d/%Y"));

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
                                      println!("{}: {}", "Dependencies".bold(), "None found".bright_green());
                                 }
                             }
                             Err(e) => {
                                 eprintln!("{} {}", "✗ Failed to extract dependencies:".red().bold(), 
        anyhow::anyhow!(e as BuildError).context("Error details"));
                             }
                         }
                    }
                    Err(e) => {
                         eprintln!("{} {}", "✗ Failed to clone repository for dependency info:".red().bold(), 
        anyhow::anyhow!(e as BuildError).context("Error details"));
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

                    let cache_dir = config.cache_path()?;
                    let aur_deps_to_remove = PackageBuilder::read_dependency_list(&package, &cache_dir)
                         .context("Failed to read AUR dependency list")?;

                    let mut packages_to_remove = vec![package.clone()];
                    packages_to_remove.extend(aur_deps_to_remove.clone());

                    alpm.remove_package(&packages_to_remove)
                        .context(format!("Failed to remove packages {:?}\n", packages_to_remove))?;

                    for dep in &aur_deps_to_remove {
                        PackageBuilder::delete_cached_package(&cache_dir, dep)?;
                    }

                    PackageBuilder::delete_cached_package(&cache_dir, &package)
                        .context("Failed to delete cached package")?;
                }
                Err(AlpmError::NotFound(_)) => {
                    eprintln!("{} {}\n", "✗ Package not found in system:".red().bold(), package.bright_red());
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(e as AlpmError).context("Failed to check if package is installed"));
                }
                Ok(false) => { // WHEN this ever happens I'm in some deep, deep shit
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
                        // Remove the file extension
                        let file_name_without_ext = file_name
                            .strip_suffix(".pkg.tar.zst")
                            .or_else(|| file_name.strip_suffix(".pkg.tar.xz"))
                            .unwrap_or(file_name);

                        let package_name;
                        let version;

                        // Find the index of the first hyphen followed by a digit (marks start of version)
                        let version_start_index = file_name_without_ext.char_indices()
                            .find(|(i, c)| {
                                *c == '-' && file_name_without_ext.chars().nth(i + 1).map_or(false, |next_c| next_c.is_digit(10))
                            })
                            .map(|(i, _)| i);

                        if let Some(index) = version_start_index {
                            let name_part = &file_name_without_ext[..index];
                            let version_with_arch = &file_name_without_ext[index + 1..];

                            // Remove the architecture from the version string
                            let arch_start_index = version_with_arch.rfind('-');
                            let version_only = if let Some(arch_idx) = arch_start_index {
                                version_with_arch[..arch_idx].to_string()
                            } else {
                                version_with_arch.to_string()
                            };
                            package_name = name_part.to_string();
                            version = version_only;
                        } else {
                             package_name = file_name_without_ext.to_string();
                             version = "unknown".to_string();
                        };
                        packages_info.push(format!("{} ({})", package_name, version));
                    }
                }
            }

            packages_info.sort();

            if packages_info.is_empty() {
                println!("\n{}\n", "No packages installed via lilac found in cache.".bold());
            } else {
                println!("\n{}\n", "Packages installed via lilac:".bold());
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

            let latest_pkg = aur.get_package_info(&package).await
                .context("Failed to fetch latest package info from AUR")?;

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

            fn extract_version_from_filename(file_name: &str, package_name: &str) -> Option<String> {
                let stripped = file_name.strip_prefix(package_name)?;
                let parts: Vec<&str> = stripped.split('-').collect();
                if parts.len() >= 3 {
                    // Combine version and release (e.g., "0.7.7-1")
                    Some(format!("{}-{}", parts[1], parts[2]))
                } else {
                    None
                }
            }

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

            let cached_ver = Version::new(&cached_version);
            let latest_ver = Version::new(&latest_pkg.version);

            if cached_ver < latest_ver {
                println!(
                    "{} {} {}",
                    "Updating package:".bold(),
                    package.bright_green(),
                    format!("(from {} to {})", cached_version, latest_pkg.version).bright_cyan()
                );

                let build_dir = config.temp_path().join(&package);
                PackageBuilder::clone_repo(&package, &build_dir)
                    .context("Failed to clone repository for update")?;

                let package_path = PackageBuilder::build_package_with_deps(
                    &package,
                    &build_dir,
                    &config,
                ).await
                .context("Failed to rebuild package")?;

                alpm.remove_package(&[package])
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
