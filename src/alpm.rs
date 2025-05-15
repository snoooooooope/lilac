use crate::error::{AlpmError, alpm_init_error, alpm_install_error};
use alpm::Alpm;
use std::process::Command;
use std::path::Path;
use colored::Colorize;

pub struct AlpmWrapper {
    alpm: Alpm,
}

impl AlpmWrapper {
    pub fn new() -> Result<Self, AlpmError> {
        let alpm = Alpm::new("/", "/var/lib/pacman")
            .map_err(|e| alpm_init_error(format!("Failed to initialize ALPM: {}", e)))?;

        Ok(AlpmWrapper { alpm })
    }

    // Checks if a package is installed
    pub fn is_package_installed(&self, package_name: &str) -> Result<bool, AlpmError> {
        self.alpm.localdb().pkg(package_name)
            .map(|_| true)
            .or_else(|e| match e {
                alpm::Error::PkgNotFound => Err(AlpmError::NotFound(package_name.to_string())),
                e => Err(AlpmError::DatabaseError(format!("Database query failed: {}", e))),
            })
    }

    // Installs a package from a file.
    pub fn install_package(&self, package_path: &Path) -> Result<(), AlpmError> {
        println!(
            "{} {} {} {}
",
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

    // Removes a package from the system.
    pub fn remove_package(&self, package_name: &str) -> Result<(), AlpmError> {
        println!(
            "{} {} {}",
            "Removing:".bold(),
            package_name.bright_green(),
            "from the system".bold()
        );

        let status = Command::new("sudo")
            .arg("pacman")
            .arg("-Rc")
            .arg(package_name)
            .status()
            .map_err(|e| crate::error::alpm_remove_error(format!("Failed to execute pacman for removal: {}", e)))?;

        if !status.success() {
            Err(crate::error::alpm_remove_error(format!(
                "pacman -Rc failed with exit code: {}",
                status
            )))
        } else {
            println!("
{}
", "✓ Successfully removed!".green().bold());
            Ok(())
        }
    }
}
