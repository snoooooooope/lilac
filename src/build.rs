use crate::error::{BuildError, BuildError::*};
use git2::Repository;
use log::info;
use std::process::Command;
use std::path::Path;

/// Handles package building operations
pub struct PackageBuilder;

impl PackageBuilder {
    /// Clones a package repository from AUR
    pub fn clone_repo(package_name: &str, dest_path: &Path) -> Result<(), BuildError> {
        let url = format!("https://aur.archlinux.org/{}.git", package_name);
        info!("Cloning repository: {} to {:?}", url, dest_path);

        Repository::clone(&url, dest_path)
            .map_err(|e| GitError(format!("Git clone failed: {}", e)))?;

        Ok(())
    }

    /// Executes makepkg in the specified directory
    pub fn execute_makepkg(build_dir: &Path) -> Result<(), BuildError> {
        info!("Building package in: {:?}", build_dir);

        let status = Command::new("makepkg")
            .current_dir(build_dir)
            .args(["-si", "--noconfirm"])
            .status()
            .map_err(|e| MakePkgError(format!("makepkg execution failed: {}", e)))?;

        if !status.success() {
            return Err(MakePkgError(format!("makepkg failed with exit code: {}", status)));
        }

        Ok(())
    }

    /// Cleans up build artifacts
    pub fn clean_build_artifacts(build_dir: &Path) -> Result<(), BuildError> {
        info!("Cleaning build artifacts in: {:?}", build_dir);
        
        let status = Command::new("makepkg")
            .current_dir(build_dir)
            .arg("--clean")
            .status()
            .map_err(|e| CleanupError(format!("Clean failed: {}", e)))?;

        if !status.success() {
            return Err(CleanupError("Failed to clean build artifacts".into()));
        }

        Ok(())
    }
}
