use crate::error::{BuildError, build_git_error, build_makepkg_error};
use git2::Repository;
use std::process::Command;
use std::path::{Path, PathBuf};
use std::{str, fs};
use colored::Colorize;
use crate::config::AppConfig;

pub struct PackageBuilder;

impl PackageBuilder {
    pub fn clone_repo(package_name: &str, dest_path: &Path) -> Result<(), BuildError> {
        let url = format!("https://aur.archlinux.org/{}.git", package_name);
        println!(
            "{} {} {} {}",
            "Cloning repository:".bold(),
            package_name.bright_green(),
            "to".bold(),
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
        println!(
            "{} {} {} {}",
            "Building package".bold(),
            package_name.bright_green(),
            "in:".bold(),
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

    pub fn get_dependencies_from_srcinfo(build_dir: &Path) -> Result<Vec<String>, BuildError> {
        println!(
            "{} {}",
            "Extracting dependencies from .SRCINFO in:".bold(),
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
        config: &AppConfig,
    ) -> Result<PathBuf, BuildError> {
        let cache_dir = config.cache_path().map_err(|e| build_makepkg_error(
            format!("Failed to access cache directory: {}", e),
            "caching",
        ))?;
        let cached_pkg = Self::find_cached_package(&cache_dir, package_name);
        if let Some(cached_pkg) = cached_pkg {
            println!(
                "{} {} {}",
                "Using cached package:".bold(),
                package_name.bright_green(),
                format!("({:?})", cached_pkg).bright_cyan()
            );
            return Ok(cached_pkg);
        }

        let pkg_path = Self::execute_makepkg(package_name, build_dir)?;
        Self::cache_package(&pkg_path, &cache_dir, package_name)?;
        Ok(pkg_path)
    }

    pub fn find_cached_package(cache_dir: &Path, package_name: &str) -> Option<PathBuf> {
        let entries = fs::read_dir(cache_dir).ok()?;
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
                    if file_name.starts_with(package_name) 
                        && (file_name.ends_with(".pkg.tar.zst") || file_name.ends_with(".pkg.tar.xz"))
                    {
                        return Some(path);
                    }
                }
            }
        }
        None
    }

    fn cache_package(pkg_path: &Path, cache_dir: &Path, package_name: &str) -> Result<(), BuildError> {
        let cached_path = cache_dir.join(pkg_path.file_name().unwrap());
        fs::copy(pkg_path, &cached_path).map_err(|e| build_makepkg_error(
            format!("Failed to cache package: {}", e),
            "caching",
        ))?;

        println!(
            "{} {} {}",
            "Cached package:".bold(),
            package_name.bright_green(),
            format!("({:?})", cached_path).bright_cyan()
        );
        Ok(())
    }

    /// Deletes a package from the cache directory.
    pub fn delete_cached_package(cache_dir: &Path, package_name: &str) -> Result<(), BuildError> {
        let entries = fs::read_dir(cache_dir)
            .map_err(|e| build_makepkg_error(
                format!("Failed to read cache directory: {}", e),
                "cache cleanup",
            ))?;

        for entry in entries {
            let entry = entry.map_err(|e| build_makepkg_error(
                format!("Error reading cache directory entry: {}", e),
                "cache cleanup",
            ))?;
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
                if file_name.starts_with(package_name) 
                    && (file_name.ends_with(".pkg.tar.zst") || file_name.ends_with(".pkg.tar.xz"))
                {
                    fs::remove_file(&path).map_err(|e| build_makepkg_error(
                        format!("Failed to delete cached package: {}", e),
                        "cache cleanup",
                    ))?;
                    println!(
                        "{} {} {}",
                        "Deleted cached package:".bold(),
                        package_name.bright_green(),
                        format!("({:?})", path).bright_cyan()
                    );
                }
            }
        }
        Ok(())
    }
}
