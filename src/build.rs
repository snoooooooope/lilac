use crate::error::{BuildError, build_git_error, build_makepkg_error};
use git2::Repository;
use std::process::{Command, Stdio};
use std::path::{Path, PathBuf};
use std::{str, fs};
use colored::Colorize;
use crate::config::AppConfig;
use crate::alpm::AlpmWrapper;
use crate::AlpmError;

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

    pub fn execute_makepkg(
        package_name: &str,
        build_dir: &Path,
    ) -> Result<(), BuildError> {
        println!(
            "{} {} {} {}",
            "Running makepkg for".bold(),
            package_name.bright_green(),
            "in:".bold(),
            format!("{:?}", build_dir).bright_cyan()
        );

        let status = Command::new("makepkg")
            .current_dir(build_dir)
            .args(["--syncdeps", "--cleanbuild"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| build_makepkg_error(
                format!("Failed to spawn makepkg: {}", e),
                "build"
            ))?;

        if !status.success() {
            return Err(build_makepkg_error(
                format!("makepkg failed with exit code: {}", status),
                "build"
            ));
        }

        println!("\n{}\n", "✓ makepkg build succeeded.".green().bold());
        Ok(())
    }

    pub fn get_dependencies_from_srcinfo(build_dir: &Path) -> Result<Vec<String>, BuildError> {
        println!(
            "{} {}",
            "Extracting dependencies from .SRCINFO in:".bold(),
            format!("{:?}", build_dir).bright_cyan()
        );

        let srcinfo_path = build_dir.join(".SRCINFO");

        if !srcinfo_path.exists() {
            return Err(build_makepkg_error(
                format!(".SRCINFO file not found at {:?}", srcinfo_path),
                "dependency extraction"
            ));
        }

        let content = fs::read_to_string(&srcinfo_path)
            .map_err(|e| build_makepkg_error(
                format!("Failed to read .SRCINFO file: {}", e),
                "dependency extraction"
            ))?;

        let mut dependencies = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("depends =") || 
               trimmed.starts_with("makedepends =") || 
               trimmed.starts_with("checkdepends =") {
                if let Some(dep) = trimmed.splitn(2, '=').nth(1) {
                    let dep = dep.trim();
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
        }

        Ok(dependencies)
    }

    pub async fn install_dependencies(
        dependencies: &[String],
        alpm: &AlpmWrapper,
        aur: &crate::aur::AurClient,
        config: &AppConfig,
    ) -> Result<(Vec<String>, Vec<std::path::PathBuf>), BuildError> {
        let cache_dir = config.cache_path()?;
        let mut official_repo_deps: Vec<String> = Vec::new();
        let mut aur_deps_to_build: Vec<String> = Vec::new();
        let mut cached_deps_to_install: Vec<String> = Vec::new();
        let mut cached_pkg_paths: Vec<std::path::PathBuf> = Vec::new();

        println!("{}", "Categorizing dependencies...".bold());

        for dep in dependencies.iter() {
            match alpm.is_package_installed(dep) {
                Ok(true) => {
                    continue;
                }
                Err(AlpmError::NotFound(_)) | Ok(false) => {
                }
                Err(e) => {
                    return Err(build_makepkg_error(
                        format!("Failed to check if dependency {} is installed: {}", dep, e),
                        "dependency check",
                    ));
                }
            }

            // Check if the dependency is in the official repositories
            match alpm.is_package_available(dep) {
                 Ok(true) => {
                    official_repo_deps.push(dep.clone());
                 }
                 Ok(false) => {
                    if let Some(cached_pkg_path) = Self::find_cached_package(&cache_dir, dep) {
                        cached_deps_to_install.push(dep.clone());
                        cached_pkg_paths.push(cached_pkg_path);
                    } else {
                         match aur.get_package_info(dep).await {
                             Ok(_) => {
                                 aur_deps_to_build.push(dep.clone());
                             },
                             Err(crate::error::AurError::NotFound(_)) => {
                                 return Err(build_makepkg_error(
                                     format!("Dependency {} not found in official repos, cache, or AUR", dep),
                                     "dependency resolution",
                                 ));
                             },
                             Err(e) => {
                                 return Err(build_makepkg_error(
                                     format!("Failed to check AUR for dependency {}: {}", dep, e),
                                     "dependency resolution",
                                 ));
                             }
                         }
                    }
                 }
                 Err(e) => {
                     return Err(build_makepkg_error(
                         format!("Failed to check if dependency {} is in official repos: {}", dep, e),
                         "dependency check",
                     ));
                 }
            }
        }

        // Build and cache AUR dependencies
        if !aur_deps_to_build.is_empty() {
            for dep in &aur_deps_to_build {
                let current_alpm = AlpmWrapper::new()?;
                match current_alpm.is_package_installed(&dep) {
                    Ok(true) => {
                        continue;
                    },
                    Err(AlpmError::NotFound(_)) | Ok(false) => {},
                    Err(e) => {
                        return Err(build_makepkg_error(
                            format!("Failed to re-check if dependency {} is installed: {}", dep, e),
                            "dependency check",
                        ));
                    }
                }

                // Build from AUR
                let temp_dir = tempfile::tempdir().map_err(|e| build_makepkg_error(
                    format!("Failed to create temp dir for {}: {}", dep, e),
                    "dependency resolution"
                ))?;

                let dep_build_dir = temp_dir.path().join(&dep);
                Self::clone_repo(&dep, &dep_build_dir)?;

                let output = std::process::Command::new("makepkg")
                    .current_dir(&dep_build_dir)
                    .args(["--syncdeps"])
                    .output()
                    .map_err(|e| build_makepkg_error(
                        format!("makepkg failed for dependency {}: {}", dep, e),
                        "dependency build"
                    ))?;

                if !output.status.success() {
                    return Err(build_makepkg_error(
                        format!("Failed to build dependency {}: {}", dep,
                            std::str::from_utf8(&output.stderr).unwrap_or("<invalid UTF-8>")),
                        "dependency build"
                    ));
                }

                let pkg_path_in_temp = Self::find_built_package(&dep_build_dir, &dep)?;
                Self::cache_package(&pkg_path_in_temp, &cache_dir, &dep)?;
                let cached_path = cache_dir.join(pkg_path_in_temp.file_name().unwrap());
                if cached_path.exists() {
                    cached_pkg_paths.push(cached_path);
                } else {
                    return Err(build_makepkg_error(
                        format!("Failed to find cached package {} in cache after building and caching", dep),
                        "caching",
                    ));
                }
            }
        }

        // Add any cached dependencies
        for cached_pkg_path in &cached_pkg_paths {
            if !official_repo_deps.contains(&cached_pkg_path.to_string_lossy().to_string()) {
                // Only add unique paths, I dont like this
                // TODO: fix my own stupidity
            }
        }

        Ok((official_repo_deps, cached_pkg_paths))
    }

    pub async fn build_package_with_deps(
        package_name: &str,
        build_dir: &Path,
        aur: &crate::aur::AurClient,
        config: &AppConfig,
    ) -> Result<Vec<PathBuf>, BuildError> {
        let cache_dir = config.cache_path().map_err(|e| build_makepkg_error(
            format!("Failed to access cache directory: {}", e),
            "caching",
        ))?;

        if let Some(cached_pkg) = Self::find_cached_package(&cache_dir, package_name) {
            println!(
                "{} {} {}",
                "Using cached package:".bold(),
                package_name.bright_green(),
                format!("({:?})", cached_pkg).bright_cyan()
            );
            let mut pkgs = vec![];
            let deps = Self::read_dependency_list(package_name, &cache_dir).unwrap_or_default();
            for dep in deps {
                if let Some(dep_pkg) = Self::find_cached_package(&cache_dir, &dep) {
                    pkgs.push(dep_pkg);
                }
            }
            pkgs.push(cached_pkg);
            return Ok(pkgs);
        }

        println!(
            "{} {} {} {}",
            "Building package".bold(),
            package_name.bright_green(),
            "in:".bold(),
            format!("{:?}", build_dir).bright_cyan()
        );

        if !build_dir.exists() || !build_dir.is_dir() || std::fs::read_dir(build_dir).map_err(|e| build_makepkg_error(
            format!("Failed to read directory {:?}: {}", build_dir, e),
            "dependency check"
        ))?.count() == 0 {
            Self::clone_repo(package_name, build_dir)?;
        } else {
            println!("{} {} already exists, skipping clone.", "Repository:".bold(), package_name.bright_green());
        }

        let dependencies = Self::get_dependencies_from_srcinfo(build_dir)?;
        
        let alpm = AlpmWrapper::new()?;
        let (official_repo_deps, aur_pkg_paths) = Self::install_dependencies(&dependencies, &alpm, aur, config).await?;

        // Install official repo dependencies with pacman -S --needed
        if !official_repo_deps.is_empty() {
            println!("\n{}\n", "✓ Official repository dependencies found.".green().bold());
            let status = std::process::Command::new("sudo")
                .arg("pacman")
                .arg("-S")
                .arg("--needed")
                .args(&official_repo_deps)
                .status();
            match status {
                Ok(exit_status) => {
                    if !exit_status.success() {
                        return Err(build_makepkg_error(
                            format!("pacman -S failed with exit code: {}", exit_status),
                            "dependency pre-install",
                        ));
                    }
                }
                Err(e) => {
                    return Err(build_makepkg_error(
                        format!("Failed to execute pacman -S for dependencies: {}", e),
                        "dependency pre-install",
                    ));
                }
            }
        }

        // Install AUR dependencies with pacman -U
        if !aur_pkg_paths.is_empty() {
            println!("\n{}\n", "✓ AUR dependencies found.".green().bold());
            let status = std::process::Command::new("sudo")
                .arg("pacman")
                .arg("-U")
                .args(&aur_pkg_paths)
                .status();
            match status {
                Ok(exit_status) => {
                    if !exit_status.success() {
                        return Err(build_makepkg_error(
                            format!("pacman -U failed with exit code: {}", exit_status),
                            "dependency pre-install",
                        ));
                    }
                }
                Err(e) => {
                    return Err(build_makepkg_error(
                        format!("Failed to execute pacman -U for AUR dependencies: {}", e),
                        "dependency pre-install",
                    ));
                }
            }
        }

        Self::execute_makepkg(package_name, build_dir)?;

        println!("{} {} {}.", "Main package:".bold(), package_name.bright_green(), "built successfully".bold());

        let pkg_path_in_temp = Self::find_built_package(build_dir, package_name)?;
        Self::cache_package(&pkg_path_in_temp, &cache_dir, package_name)?;
        Self::save_dependency_list(package_name, &cache_dir, &dependencies)?;

        // Collect all dependency package paths from install_dependencies, then add main package last
        let mut all_pkgs = aur_pkg_paths;
        if let Some(main_pkg) = Self::find_cached_package(&cache_dir, package_name) {
            all_pkgs.push(main_pkg);
        }
        if all_pkgs.is_empty() {
            return Err(build_makepkg_error(
                format!("Failed to find any packages to install for {}", package_name),
                "caching"
            ));
        }
        Ok(all_pkgs)
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

        let mut packages_info: Vec<String> = Vec::new();

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
                        format!("Failed to delete cached package: {}\n", e),
                        "cache cleanup",
                    ))?;
                    println!(
                        "{} {} {}",
                        "Deleted cached package:".bold(),
                        package_name.bright_green(),
                        format!("({:?})", path).bright_cyan()
                    );
                    packages_info.push(file_name.to_string());
                }
            }
        }

        packages_info.sort();

        Ok(())
    }

    // function to save the list of newly installed dependencies
    fn save_dependency_list(
        package_name: &str,
        cache_dir: &Path,
        dependencies: &[String],
    ) -> Result<(), BuildError> {
        let deps_file_path = cache_dir.join(format!("{}.lilac_deps", package_name));
        let content = dependencies.join("\n");
        fs::write(&deps_file_path, content).map_err(|e| build_makepkg_error(
            format!("Failed to write dependency list to {}: {}", deps_file_path.display(), e),
            "dependency tracking",
        ))?;
        println!(
            "{} {} {}",
            "Saved dependency list for:".bold(),
            package_name.bright_green(),
            format!("({:?})", deps_file_path).bright_cyan()
        );
        Ok(())
    }

    // function to read the list of dependencies for a package from the cache
    pub fn read_dependency_list(
        package_name: &str,
        cache_dir: &Path,
    ) -> Result<Vec<String>, BuildError> {
        let deps_file_path = cache_dir.join(format!("{}.lilac_deps", package_name));

        if !deps_file_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&deps_file_path).map_err(|e| build_makepkg_error(
            format!("Failed to read dependency list from {}: {}", deps_file_path.display(), e),
            "dependency tracking",
        ))?;

        let dependencies: Vec<String> = content.lines().map(|s| s.to_string()).collect();

        println!(
            "{} {} {}",
            "Read dependency list for:".bold(),
            package_name.bright_green(),
            format!("({:?})", deps_file_path).bright_cyan()
        );

        Ok(dependencies)
    }

    fn find_built_package(build_dir: &Path, package_name: &str) -> Result<PathBuf, BuildError> {
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

        Err(build_makepkg_error(
            format!("No valid package file found for {}", package_name),
            "package discovery"
        ))
    }
}
