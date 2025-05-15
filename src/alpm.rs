use crate::error::{AlpmError, AlpmError::*};
use alpm::Alpm;
use std::process::Command;
use std::path::Path;
use colored::Colorize;

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
            Err(_) => Ok(false),
        }
    }

    /// Installs a package from a file.
    /// This will require sudo privileges.
    pub fn install_package(&self, package_path: &Path) -> Result<(), AlpmError> {
        println!(
            "{} {} {} {}\n",
            "Installing:".white(),
            package_path.file_name().unwrap().to_str().unwrap().bright_green(),
            "from:".white(),
            package_path.parent().unwrap().display().to_string().bright_cyan()
        );

        let status = Command::new("sudo")
            .arg("pacman")
            .arg("-U")
            .arg(package_path)
            .status()
            .map_err(|e| AlpmError::InitError(format!("Failed to execute pacman: {}", e)))?;

        if !status.success() {
            eprintln!("{}", "✗ Installation failed!".red().bold());
            return Err(AlpmError::InitError(format!(
                "pacman failed with exit code: {}",
                status
            )));
        }

        println!("{}", "✓ Successfully installed!".green().bold());
        Ok(())
    }

    /// Checks if a package is available in the official repositories (sync databases).
    pub fn is_package_available(&self, package_name: &str) -> Result<bool, AlpmError> {
        for db in self.alpm.syncdbs() {
            if db.pkg(package_name).is_ok() {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
