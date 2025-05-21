use super::error::{AlpmError, alpm_init_error, alpm_install_error, alpm_remove_error};
use alpm::Alpm;
use std::process::Command;
use std::path::Path;
use colored::Colorize;
use std::sync::Arc;

pub struct AlpmWrapper {
    alpm: Arc<Alpm>,
}

impl AlpmWrapper {
    pub fn new() -> Result<Self, AlpmError> {
        let alpm = Alpm::new("/", "/var/lib/pacman")
            .map_err(|e| alpm_init_error(format!("Failed to initialize ALPM: {}", e)))?;
        Ok(AlpmWrapper { alpm: Arc::new(alpm) })
    }

    // Checks if a package is installed
    pub fn is_package_installed(&self, package_name: &str) -> Result<bool, AlpmError> {
        self.alpm.localdb().pkg(package_name)
            .map(|_| true)
            .or_else(|e| match e {
                alpm::Error::PkgNotFound => Ok(false),
                e => Err(AlpmError::DatabaseError(format!("Database query failed: {}", e))),
            })
    }

    pub fn install_package(&self, package_path: &Path) -> Result<(), AlpmError> {
        println!(
            "{} {} {} {}",
            "Installing:".bold(),
            package_path.file_name().unwrap().to_str().unwrap().bright_green(),
            "from:".bold(),
            package_path.parent().unwrap().display().to_string().bright_cyan()
        );

        let status = Command::new("sudo")
            .arg("pacman")
            .arg("-U")
            .arg(package_path)
            .status()
            .map_err(|e| alpm_install_error(format!("Failed to execute pacman: {}", e)))?;

        if !status.success() {
            Err(alpm_install_error(format!(
                "pacman failed with exit code: {}",
                status
            )))
        } else {
            println!("\n{}", "✓ Successfully installed!\n".green().bold());
            Ok(())
        }
    }

    // Checks if a package is available in the official repositories.
    pub fn is_package_available(&self, package_name: &str) -> Result<bool, AlpmError> {
        for db in self.alpm.syncdbs() {
            match db.pkg(package_name) {
                Ok(_) => return Ok(true),
                Err(alpm::Error::PkgNotFound) => continue,
                Err(e) => return Err(AlpmError::DatabaseError(format!(
                    "Database query failed: {}", e
                ))),
            }
        }
        Ok(false)
    }

    // Removes a package from the system recursively, removing dependencies no longer needed
    pub fn remove_package(&self, package_names: &[String]) -> Result<(), AlpmError> {
        println!(
            "{} {:?} {}",
            "Removing:".bold(),
            package_names,
            "from the system".bold()
        );

        let status = Command::new("sudo")
            .arg("pacman")
            .arg("-Rs")
            .args(package_names)
            .status()
            .map_err(|e| alpm_remove_error(format!("Failed to execute pacman for removal: {}", e)))?;

        if !status.success() {
            Err(alpm_remove_error(format!(
                "pacman -Rs failed with exit code: {}",
                status
            )))
        } else {
            println!("\n{}\n", "✓ Successfully removed!".green().bold());
            Ok(())
        }
    }

    pub fn force_remove_package(&self, package_name: &str) -> Result<(), AlpmError> {
        println!(
            "{} {} {}",
            "Forcibly removing:".bold(),
            package_name.bright_green(),
            "from the system (bypassing dependency checks)".bold()
        );

        let status = Command::new("sudo")
            .arg("pacman")
            .arg("-R")
            .arg(package_name)
            .status()
            .map_err(|e| alpm_remove_error(format!("Failed to execute pacman for forced removal: {}", e)))?;

        if !status.success() {
            Err(alpm_remove_error(format!(
                "pacman -R failed with exit code: {}",
                status
            )))
        } else {
            println!("\n{}", "✓ Successfully force removed!".green().bold());
            Ok(())
        }
    }
}
