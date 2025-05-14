use crate::error::{BuildError, BuildError::*};
use git2::Repository;
use log::info;
use std::process::{Command, Output};
use std::path::{Path, PathBuf};
use std::str;

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
    /// Returns the path to the built package file.
    pub fn execute_makepkg(build_dir: &Path) -> Result<PathBuf, BuildError> {
        info!("Building package in: {:?}", build_dir);

        let output = Command::new("makepkg")
            .current_dir(build_dir)
            .args(["--noconfirm"])
            .output()
            .map_err(|e| MakePkgError(format!("makepkg execution failed: {}", e)))?;

        if !output.status.success() {
            let stderr = str::from_utf8(&output.stderr).unwrap_or("<invalid UTF-8>");
            return Err(MakePkgError(format!("makepkg failed with exit code: {}\nStderr: {}", output.status, stderr)));
        }

        // Attempt to parse the package filename from stdout
        let stdout = str::from_utf8(&output.stdout).unwrap_or("");
        let package_filename = stdout.lines()
            .rev()
            .find_map(|line|
                line.trim().strip_prefix("==> Created package: ")
            );

        match package_filename {
            Some(filename) => {
                let package_path = build_dir.join(filename);
                info!("Built package file: {:?}", package_path);
                Ok(package_path)
            }
            None => Err(MakePkgError("Failed to find package filename in makepkg output".to_string())),
        }
    }
}
