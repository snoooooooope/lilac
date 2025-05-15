use crate::error::{BuildError, build_git_error, build_makepkg_error};
use git2::Repository;
use log::info;
use std::process::Command;
use std::path::{Path, PathBuf};
use std::{str, fs};
use crate::aur::AurClient;
use crate::alpm::AlpmWrapper;
use colored::Colorize;
use crate::error::AurError;

pub struct PackageBuilder;

impl PackageBuilder {
    pub fn clone_repo(package_name: &str, dest_path: &Path) -> Result<(), BuildError> {
        let url = format!("https://aur.archlinux.org/{}.git", package_name);
        info!(
            "{} {} {} {}\n",
            "Cloning repository:".white(),
            package_name.bright_green(),
            "to".white(),
            format!("{:?}", dest_path).bright_cyan()
        );

        Repository::clone(&url, dest_path)
            .map_err(|e| build_git_error(
                format!("Git clone failed: {}", e),
                package_name
            ))?;

        Ok(())
    }

    pub fn execute_makepkg(package_name: &str, build_dir: &Path) -> Result<PathBuf, BuildError> {
        info!(
            "{} {} {} {}",
            "Building package".white(),
            package_name.bright_green(),
            "in:".white(),
            format!("{:?}", build_dir).bright_cyan()
        );

        let output = Command::new("makepkg")
            .current_dir(build_dir)
            .args(["-s", "--noconfirm"])
            .output()
            .map_err(|e| build_makepkg_error(
                format!("makepkg execution failed: {}", e),
                "build"
            ))?;

        if !output.status.success() {
            let stderr = str::from_utf8(&output.stderr).unwrap_or("<invalid UTF-8>");
            return Err(build_makepkg_error(
                format!("Exit code: {}\nStderr: {}", output.status, stderr),
                "build"
            ));
        }

        let entries = fs::read_dir(build_dir)
            .map_err(|e| build_makepkg_error(
                format!("Failed to read build directory: {}", e),
                "package discovery"
            ))?;

        for entry in entries {
            let entry = entry.map_err(|e| build_makepkg_error(
                format!("Error reading directory entry: {}", e),
                "package discovery"
            ))?;
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
                if (file_name.ends_with(".pkg.tar.zst") || file_name.ends_with(".pkg.tar.xz"))
                    && !file_name.contains("-debug-")
                    && file_name.starts_with(package_name)
                {
                    return Ok(path);
                }
            }
        }

        Err(build_makepkg_error("No valid package file found after build", "package discovery"))
    }

    // Runs makepkg --printsrcinfo and extracts dependencies.
    // Returns a list of dependency strings.
    pub fn get_dependencies_from_srcinfo(build_dir: &Path) -> Result<Vec<String>, BuildError> {
        info!(
            "{} {}",
            "Extracting dependencies from .SRCINFO in:".white(),
            format!("{:?}", build_dir).bright_cyan()
        );

        let output = Command::new("makepkg")
            .current_dir(build_dir)
            .arg("--printsrcinfo")
            .output()
            .map_err(|e| build_makepkg_error(
                format!("Failed to get .SRCINFO: {}", e),
                "dependency extraction"
            ))?;

        if !output.status.success() {
            return Err(build_makepkg_error(
                format!("makepkg --printsrcinfo failed with code: {}", output.status),
                "dependency extraction"
            ));
        }

        let stdout = str::from_utf8(&output.stdout).unwrap_or("");
        let mut dependencies = Vec::new();

        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("depends =") 
                || trimmed.starts_with("makedepends =")
                || trimmed.starts_with("checkdepends =") 
            {
                let dep = trimmed.splitn(2, '=').nth(1).unwrap_or("").trim();
                if !dep.is_empty() {
                    // Extract base package name (strip version constraints)
                    let pkg_name = dep.split(&['<', '>', '=', ' '][..])
                        .next()
                        .unwrap_or(dep)
                        .trim()
                        .to_string();
                    if !pkg_name.is_empty() && !dependencies.contains(&pkg_name) {
                        dependencies.push(pkg_name);
                    }
                }
            }
        }

        Ok(dependencies)
    }

    pub async fn build_package_with_deps(
        package_name: &str,
        build_dir: &Path,
        aur_client: &AurClient,
        alpm_wrapper: &AlpmWrapper,
    ) -> Result<PathBuf, BuildError> {
        // Verify .SRCINFO exists before proceeding
        let srcinfo_path = build_dir.join(".SRCINFO");
        if !srcinfo_path.exists() {
            return Err(build_makepkg_error(
                format!("No .SRCINFO found for package {} - is this a valid AUR package?", package_name),
                "dependency resolution"
            ));
        }

        info!(
            "{} {} {}\n",
            "Starting build process for".white(),
            package_name.bright_green(),
            "with dependencies".white()
        );

        let dependencies = Self::get_dependencies_from_srcinfo(build_dir)?;
        info!(
            "{} {} {} {}",
            "Processing".white(),
            dependencies.len().to_string().bright_green(),
            "dependencies for".white(),
            package_name.bright_green()
        );

        for dep in dependencies {
            info!(
                "{} {}",
                "Checking dependency:".white(),
                dep.bright_green()
            );
            
            if !alpm_wrapper.is_package_installed(&dep)
                .map_err(|e| build_makepkg_error(
                    format!("ALPM error: {}", e),
                    "dependency resolution"
                ))? 
            {
                info!(
                    "{} {} {}",
                    "Dependency".white(),
                    dep.bright_green(),
                    "not installed, checking official repositories...".white()
                );
                if !alpm_wrapper.is_package_available(&dep)
                    .map_err(|e| build_makepkg_error(
                        format!("ALPM error: {}", e),
                        "dependency resolution"
                    ))?
                {
                    info!(
                        "{} {} {}",
                        "Dependency".white(),
                        dep.bright_green(),
                        "not in official repositories, checking AUR...".white()
                    );
                    match aur_client.get_package_info(&dep).await {
                        Ok(pkg) => {
                            info!(
                                "{} {} {}",
                                "Building AUR dependency:".white(),
                                pkg.name.bright_green(),
                                format!("({})", pkg.version).bright_cyan()
                            );
                            let dep_temp_dir = tempfile::tempdir()
                                .map_err(|e| build_makepkg_error(
                                    format!("Failed to create temp dir: {}", e),
                                    "dependency resolution"
                                ))?;
                            let dep_build_dir = dep_temp_dir.path().join(&dep);

                            Self::clone_repo(&dep, &dep_build_dir)?;
                            let pkg_path = Box::pin(Self::build_package_with_deps(&dep, &dep_build_dir, aur_client, alpm_wrapper)).await?;
                            
                            info!("Installing dependency: {}", dep);
                            alpm_wrapper.install_package(&pkg_path)
                                .map_err(|e| build_makepkg_error(
                                    format!("Installation failed: {}", e),
                                    "dependency resolution"
                                ))?;
                        }
                        Err(AurError::NotFound(_)) => {
                            info!(
                                "{} {} {}",
                                "Dependency".white(),
                                dep.bright_green(),
                                "not in AUR, skipping".white()
                            );
                        }
                        Err(e) => return Err(build_makepkg_error(
                            format!("AUR error: {}", e),
                            "dependency resolution"
                        )),
                    }
                } else {
                    info!(
                        "{} {} {}",
                        "Dependency".white(),
                        dep.bright_green(),
                        "is available in official repositories, skipping AUR check".white()
                    );
                }
            }
        }

        info!(
            "{} {}\n",
            "All dependencies resolved, building main package:".white(),
            package_name.bright_green()
        );
        let pkg_path = Self::execute_makepkg(package_name, build_dir)?;
        Ok(pkg_path)
    }
}
