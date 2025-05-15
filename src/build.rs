use crate::error::{BuildError, AurError};
use git2::Repository;
use log::info;
use std::process::Command;
use std::path::{Path, PathBuf};
use std::{str, fs};
use crate::aur::AurClient;
use crate::alpm::AlpmWrapper;

/// Handles package building operations
pub struct PackageBuilder;

impl PackageBuilder {
    /// Clones a package repository from AUR
    pub fn clone_repo(package_name: &str, dest_path: &Path) -> Result<(), BuildError> {
        let url = format!("https://aur.archlinux.org/{}.git", package_name);
        info!("Cloning repository: {} to {:?}", url, dest_path);

        Repository::clone(&url, dest_path)
            .map_err(|e| BuildError::GitError(format!("Git clone failed: {}", e)))?;

        Ok(())
    }

    /// Executes makepkg in the specified directory
    /// Returns the path to the built package file.
    /// Prefers non-debug packages if available
    pub fn execute_makepkg(package_name: &str, build_dir: &Path) -> Result<PathBuf, BuildError> {
        info!("Building package {} in: {:?}", package_name, build_dir);

        // Run makepkg
        let output = Command::new("makepkg")
            .current_dir(build_dir)
            .args(["-s", "--noconfirm"])
            .output()
            .map_err(|e| BuildError::MakePkgError(format!("makepkg execution failed: {}", e)))?;

        if !output.status.success() {
            let stderr = str::from_utf8(&output.stderr).unwrap_or("<invalid UTF-8>");
            return Err(BuildError::MakePkgError(format!(
                "makepkg failed with exit code: {}\nStderr: {}",
                output.status, stderr
            )));
        }

        // Search for the package file in the build directory
        info!("Searching for package file in: {:?}", build_dir);
        let entries = fs::read_dir(build_dir)
            .map_err(|e| BuildError::MakePkgError(format!("Failed to read build directory: {}", e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| BuildError::MakePkgError(format!("Error reading directory entry: {}", e)))?;
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
                if (file_name.ends_with(".pkg.tar.zst") || file_name.ends_with(".pkg.tar.xz"))
                    && !file_name.contains("-debug-")
                    && file_name.starts_with(package_name)
                {
                    info!("Found package file: {:?}", path);
                    return Ok(path);
                }
            }
        }

        Err(BuildError::MakePkgError("No valid package file found after build".into()))
    }

    /// Runs makepkg --printsrcinfo and extracts dependencies.
    /// Returns a list of dependency strings.
    pub fn get_dependencies_from_srcinfo(build_dir: &Path) -> Result<Vec<String>, BuildError> {
        info!("Extracting dependencies from .SRCINFO in: {:?}", build_dir);

        let output = Command::new("makepkg")
            .current_dir(build_dir)
            .arg("--printsrcinfo")
            .output()
            .map_err(|e| BuildError::MakePkgError(format!("Failed to get .SRCINFO: {}", e)))?;

        if !output.status.success() {
            return Err(BuildError::MakePkgError(format!(
                "makepkg --printsrcinfo failed with code: {}",
                output.status
            )));
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

        info!("Found dependencies: {:?}", dependencies);
        Ok(dependencies)
    }

    /// Builds a package and its AUR dependencies recursively.
    /// Returns the path to the built package file.
    pub async fn build_package_with_deps(
        package_name: &str,
        build_dir: &Path,
        aur_client: &AurClient,
        alpm_wrapper: &AlpmWrapper,
    ) -> Result<PathBuf, BuildError> {
        info!("Starting build process for {} with dependencies", package_name);

        let dependencies = Self::get_dependencies_from_srcinfo(build_dir)?;
        info!("Processing {} dependencies for {}", dependencies.len(), package_name);

        for dep in dependencies {
            info!("Checking dependency: {}", dep);
            
            if !alpm_wrapper.is_package_installed(&dep)
                .map_err(|e| BuildError::MakePkgError(format!("ALPM error: {}", e)))? 
            {
                info!("Dependency {} not installed, checking official repositories...", dep);
                if !alpm_wrapper.is_package_available(&dep)
                    .map_err(|e| BuildError::MakePkgError(format!("ALPM error: {}", e)))?
                {
                    info!("Dependency {} not in official repositories, checking AUR...", dep);
                    match aur_client.get_package_info(&dep).await {
                        Ok(pkg) => {
                            info!("Building AUR dependency: {} ({})", pkg.name, pkg.version);
                            let dep_temp_dir = tempfile::tempdir()
                                .map_err(|e| BuildError::MakePkgError(format!("Failed to create temp dir: {}", e)))?;
                            let dep_build_dir = dep_temp_dir.path().join(&dep);

                            Self::clone_repo(&dep, &dep_build_dir)?;
                            let pkg_path = Box::pin(Self::build_package_with_deps(&dep, &dep_build_dir, aur_client, alpm_wrapper)).await?;
                            
                            info!("Installing dependency: {}", dep);
                            alpm_wrapper.install_package(&pkg_path)
                                .map_err(|e| BuildError::MakePkgError(format!("Installation failed: {}", e)))?;
                        }
                        Err(AurError::NotFound(_)) => {
                            info!("Dependency {} not in AUR, skipping", dep);
                        }
                        Err(e) => return Err(BuildError::MakePkgError(format!("AUR error: {}", e))),
                    }
                } else {
                    info!("Dependency {} is available in official repositories, skipping AUR check", dep);
                }
            }
        }

        info!("All dependencies resolved, building main package: {}", package_name);
        let pkg_path = Self::execute_makepkg(package_name, build_dir)?;
        Ok(pkg_path)
    }
}
