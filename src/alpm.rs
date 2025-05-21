use super::error::{AlpmError, alpm_init_error, alpm_install_error, alpm_remove_error};
use alpm::Alpm;
use alpm::SigLevel;
use std::process::Command;
use std::path::Path;
use colored::Colorize;
use std::sync::Arc;
use std::fs::File;
use std::io::{BufRead, BufReader};
use log::{info, error, debug};

pub struct AlpmWrapper {
    alpm: Arc<Alpm>,
}

impl AlpmWrapper {
    pub fn new() -> Result<Self, AlpmError> {
        let alpm = Alpm::new("/", "/var/lib/pacman")
            .map_err(|e| alpm_init_error(format!("Failed to initialize ALPM: {}", e)))?;
        let wrapper = AlpmWrapper { alpm: Arc::new(alpm) };
        wrapper.load_syncdbs_from_pacman_conf()?;
        Ok(wrapper)
    }

    /// Loads enabled syncdbs from /etc/pacman.conf and registers them with ALPM
    pub fn load_syncdbs_from_pacman_conf(&self) -> Result<(), AlpmError> {
        let file = File::open("/etc/pacman.conf")
            .map_err(|e| AlpmError::DatabaseError(format!("Failed to open pacman.conf: {}", e)))?;
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let line = line.map_err(|e| AlpmError::DatabaseError(format!("Failed to read pacman.conf: {}", e)))?;
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let repo = &trimmed[1..trimmed.len()-1];
                if repo.eq_ignore_ascii_case("options") || repo.eq_ignore_ascii_case("multilib") {
                    continue;
                }
                if let Err(e) = self.alpm.register_syncdb(repo, SigLevel::USE_DEFAULT) {
                    error!("{} '{}': {}", "Failed to register syncdb".bold(), repo.bright_yellow(), AlpmError::DatabaseError(format!("{}", e)));
                } else {
                    debug!("{} '{}'.", "Registered syncdb".bold(), repo.bright_yellow());
                }
            }
        }
        Ok(())
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
        let mut found = false;
        for db in self.alpm.syncdbs() {
            let db_name = db.name();
            match db.pkg(package_name) {
                Ok(_) => {
                    info!("{} '{}' {} '{}'.",
                        "Found package".bold(),
                        package_name.bright_green(),
                        "in repo".bold(),
                        db_name.bright_yellow()
                    );
                    found = true;
                    break;
                },
                Err(alpm::Error::PkgNotFound) => {
                    debug!("{} '{}' {} '{}'.",
                        "Not found".bold(),
                        package_name.bright_red(),
                        "in repo".bold(),
                        db_name.bright_yellow()
                    );
                    continue;
                },
                Err(e) => return Err(AlpmError::DatabaseError(format!(
                    "Database query failed in repo '{}': {}", db_name.bright_yellow(), e
                ))),
            }
        }
        if !found {
            debug!("{} '{}' {}.", "Package".bold(), package_name.bright_red(), "not found in any enabled repo".bold());
        }
        Ok(found)
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

    pub fn install_packages(&self, package_paths: &[std::path::PathBuf]) -> Result<(), AlpmError> {
        if package_paths.is_empty() {
            return Ok(());
        }
        println!(
            "{} {:?} {}",
            "Installing:".bold(),
            package_paths.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect::<Vec<_>>(),
            "from cache/built packages".bold()
        );
        let status = std::process::Command::new("sudo")
            .arg("pacman")
            .arg("-U")
            .args(package_paths)
            .status()
            .map_err(|e| alpm_install_error(format!("Failed to execute pacman: {}", e)))?;
        if !status.success() {
            Err(alpm_install_error(format!(
                "pacman -U failed with exit code: {}",
                status
            )))
        } else {
            println!("\n{}\n", "✓ Successfully installed all packages!".green().bold());
            Ok(())
        }
    }
}
