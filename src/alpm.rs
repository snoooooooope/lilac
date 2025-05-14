use crate::error::{AlpmError, AlpmError::*};
use alpm::Alpm;
use log::info;
use std::path::Path;

/// Handles ALPM (pacman) operations
pub struct AlpmWrapper {
    alpm: Alpm,
}

impl AlpmWrapper {
    /// Creates a new ALPM wrapper instance
    pub fn new() -> Result<Self, AlpmError> {
        let alpm = Alpm::new("/", "/var/lib/pacman")
            .map_err(|e| InitError(format!("Failed to initialize ALPM: {}", e)))?;

        Ok(AlpmWrapper { alpm })
    }

    /// Checks if a package is installed
    pub fn is_package_installed(&self, package_name: &str) -> Result<bool, AlpmError> {
        match self.alpm.localdb().pkg(package_name) {
            Ok(_) => Ok(true),
            Err(e) => Err(QueryError(format!("Failed to query package: {}", e))),
        }
    }

    /// Installs a built package file
    pub fn install_package(&self, package_path: &Path) -> Result<(), AlpmError> {
        info!("Installing package from: {:?}", package_path);
        
        let package_path_str = package_path
            .to_str()
            .ok_or_else(|| InstallError("Invalid package path".into()))?;

        let status = std::process::Command::new("sudo")
            .arg("pacman")
            .arg("-U")
            .arg("--noconfirm")
            .arg(package_path_str)
            .status()
            .map_err(|e| InstallError(format!("Failed to execute pacman: {}", e)))?;

        if !status.success() {
            return Err(InstallError(format!(
                "Package installation failed with exit code: {}",
                status
            )));
        }

        Ok(())
    }
}
