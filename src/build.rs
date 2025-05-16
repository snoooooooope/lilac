use crate::error::{BuildError, build_git_error, build_makepkg_error};
use git2::Repository;
use std::process::{Command, Stdio};
use std::path::{Path, PathBuf};
use std::{str, fs};
use colored::Colorize;
use crate::config::AppConfig;
use crate::alpm::AlpmWrapper;
use tempfile::tempdir;
use std::io::{BufReader, Read};
use std::thread;
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

        let mut child = Command::new("makepkg")
            .current_dir(build_dir)
            .args(["--syncdeps", "--cleanbuild"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| build_makepkg_error(
                format!("Failed to spawn makepkg: {}", e),
                "build"
            ))?;

        let stdout = child.stdout.take()
            .ok_or_else(|| build_makepkg_error("Failed to capture makepkg stdout", "build"))?;
        let stderr = child.stderr.take()
            .ok_or_else(|| build_makepkg_error("Failed to capture makepkg stderr", "build"))?;

        let stdout_handle = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut buffer = String::new();
            let _ = reader.read_to_string(&mut buffer);
            buffer
        });

        let stderr_handle = thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut buffer = String::new();
            let _ = reader.read_to_string(&mut buffer);
            buffer
        });

        let status_code = child.wait()
            .map_err(|e| build_makepkg_error(
                format!("Error waiting for makepkg process to exit: {}", e),
                "build"
            ))?;

        let makepkg_output = stdout_handle.join().unwrap_or_default();
        let makepkg_stderr = stderr_handle.join().unwrap_or_default();

        println!("makepkg output:\n{}", makepkg_output);
        println!("makepkg stderr:\n{}", makepkg_stderr);

        if !status_code.success() {
            return Err(build_makepkg_error(
                format!("Exit code: {}", status_code),
                "build"
            ));
        }

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

    pub async fn build_package_with_deps(
        package_name: &str,
        build_dir: &Path,
        config: &AppConfig,
    ) -> Result<PathBuf, BuildError> {
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
            return Ok(cached_pkg);
        }

        println!(
            "{} {} {} {}",
            "Building package".bold(),
            package_name.bright_green(),
            "in:".bold(),
            format!("{:?}", build_dir).bright_cyan()
        );

        if !build_dir.exists() || !build_dir.is_dir() || fs::read_dir(build_dir).map_err(|e| build_makepkg_error(
            format!("Failed to read directory {:?}: {}", build_dir, e),
            "dependency check"
        ))?.count() == 0 {
            Self::clone_repo(package_name, build_dir)?;
        } else {
            println!("{} {} already exists, skipping clone.", "Repository:".bold(), package_name.bright_green());
        }

        let dependencies = Self::get_dependencies_from_srcinfo(build_dir)?;
        
        let alpm = AlpmWrapper::new()?;
        let newly_installed_deps = Self::install_dependencies(&dependencies, &alpm, config)?;

        Self::execute_makepkg(package_name, build_dir)?;

        println!("{} {} built successfully.", "Main package:".bold(), package_name.bright_green());

        let pkg_path_in_temp = Self::find_built_package(build_dir, package_name)?;

        Self::cache_package(&pkg_path_in_temp, &cache_dir, package_name)?;

        Self::save_dependency_list(package_name, &cache_dir, &newly_installed_deps)?;

        Self::find_cached_package(&cache_dir, package_name)
             .ok_or_else(|| build_makepkg_error(
                 format!("Failed to find cached package {} after building", package_name),
                 "caching"
             ))
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
                        "Deleted cached package:\n".bold(),
                        package_name.bright_green(),
                        format!("({:?})", path).bright_cyan()
                    );
                    packages_info.push(file_name.to_string());
                }
            }
        }

        packages_info.sort();

        if packages_info.is_empty() {
            println!("\n{}", "No packages installed via lilac found in cache.".bold());
        } else {
            println!("\n{}", "Packages in cache:".bold());
            for pkg in packages_info {
                println!("  - {}", pkg.bright_green());
            }
        }

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

    pub fn install_dependencies(
        dependencies: &[String],
        alpm: &AlpmWrapper,
        config: &AppConfig,
    ) -> Result<Vec<String>, BuildError> {
        let cache_dir = config.cache_path()?;
        let mut newly_installed_deps = Vec::new();
        let mut official_repo_deps: Vec<String> = Vec::new();
        let mut aur_deps_to_build: Vec<String> = Vec::new();
        let mut cached_deps_to_install: Vec<String> = Vec::new();
        let mut cached_pkg_paths: Vec<PathBuf> = Vec::new();

        println!("{}", "Categorizing dependencies...".bold());

        for dep in dependencies.iter() {
            print!("  - {}: ", dep.bright_green());

            // 1. Check if dependency is already installed globally by pacman
            match alpm.is_package_installed(dep) {
                Ok(true) => {
                    println!("{}", "Already installed".bright_yellow());
                    newly_installed_deps.push(dep.clone());
                    continue;
                }
                Err(AlpmError::NotFound(_)) | Ok(false) => {
                    print!("{}", "Not installed, ".bright_yellow());
                }
                Err(e) => {
                    return Err(build_makepkg_error(
                        format!("Failed to check if dependency {} is installed: {}", dep, e),
                        "dependency check",
                    ));
                }
            }

            // 2. Check if the dependency is in the official repositories
            match alpm.is_package_available(dep) {
                 Ok(true) => {
                    println!("{}", "Found in official repos".bright_blue());
                    official_repo_deps.push(dep.clone());
                 }
                 Ok(false) => {
                    print!("{}", "Not in official repos, ".bright_blue());

                    // 3. If not in official repos, check if it's in the lilac cache
                    if let Some(cached_pkg_path) = Self::find_cached_package(&cache_dir, dep) {
                        println!("{}", "Found in cache".bright_cyan());
                        cached_deps_to_install.push(dep.clone());
                        cached_pkg_paths.push(cached_pkg_path);
                    } else {
                         print!("{}", "Not in cache, ".bright_cyan());
                         println!("{}", "Likely AUR (needs building)".bright_yellow());
                         aur_deps_to_build.push(dep.clone()); // Add to a separate list for AUR processing (needs building)
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

        if !official_repo_deps.is_empty() {
            println!("{}", "Installing official repository dependencies...".bold());
            let status = Command::new("sudo")
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
                            "dependency installation",
                        ));
                    }
                     println!("{}", "✓ Official repository dependencies installed successfully.".green().bold());
                      for dep in official_repo_deps {
                          match alpm.is_package_installed(&dep) {
                              Ok(true) => { newly_installed_deps.push(dep); },
                              Err(_) | Ok(false) => { /* Should not happen if pacman -S succeeded with --needed */ }
                          }
                      }
                }
                 Err(e) => {
                     return Err(build_makepkg_error(
                        format!("Failed to execute pacman for official repository dependencies: {}", e),
                        "dependency installation",
                    ));
                 }
            }
            // Re-initialize ALPM wrapper to refresh database view after batch install
            let _alpm = AlpmWrapper::new()?;
        }
        let mut newly_built_pkg_paths: Vec<PathBuf> = Vec::new(); // Paths for newly built packages

        if !aur_deps_to_build.is_empty() {
             println!("{}", "Processing AUR dependencies (building)...".bold());
             for dep in &aur_deps_to_build {
                  println!("  - {}: ", dep.bright_green());
                  let current_alpm = AlpmWrapper::new()?;
                  match current_alpm.is_package_installed(&dep) {
                        Ok(true) => { 
                            println!("{}", "Already installed".bright_yellow());
                            // Add to newly_installed_deps if it wasn't already there
                            if !newly_installed_deps.contains(dep) { newly_installed_deps.push(dep.clone()); }
                            continue; // Skip building if already installed
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
                   println!("{}", "Building from AUR".bright_yellow());

                   let temp_dir = tempdir().map_err(|e| build_makepkg_error(
                       format!("Failed to create temp dir for {}: {}", dep, e),
                       "dependency resolution"
                   ))?;

                   let dep_build_dir = temp_dir.path().join(&dep);
                   Self::clone_repo(&dep, &dep_build_dir)?; // Clone the repo

                   let output = Command::new("makepkg")
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
                               str::from_utf8(&output.stderr).unwrap_or("<invalid UTF-8>")),
                           "dependency build"
                       ));
                   }

                   let pkg_path_in_temp = Self::find_built_package(&dep_build_dir, &dep)?; // Find the built package file in temp
                   Self::cache_package(&pkg_path_in_temp, &cache_dir, &dep)?; // Cache the built package
                   // Add the path to the *cached* package for batch installation
                   let cached_path = cache_dir.join(pkg_path_in_temp.file_name().unwrap());
                   if cached_path.exists() {
                       newly_built_pkg_paths.push(cached_path);
                   } else {
                       // This case should ideally not happen if cache_package was successful
                       return Err(build_makepkg_error(
                           format!("Failed to find cached package {} in cache after building and caching", dep),
                           "caching",
                       ));
                   }
               }
           }

           // 3. Install AUR dependencies (both cached and newly built) in a batch
           let all_aur_pkg_paths_to_install = cached_pkg_paths.into_iter()
                                                   .chain(newly_built_pkg_paths.into_iter())
                                                   .collect::<Vec<PathBuf>>();

           if !all_aur_pkg_paths_to_install.is_empty() {
               println!("{}", "Installing AUR dependencies (from cache and newly built)...".bold());
               let status = Command::new("sudo")
                   .arg("pacman")
                   .arg("-U")
                   .args(&all_aur_pkg_paths_to_install)
                   .status();

               match status {
                   Ok(exit_status) => {
                       if !exit_status.success() {
                           return Err(build_makepkg_error(
                               format!("pacman -U failed with exit code: {}", exit_status),
                               "dependency installation",
                           ));
                       }
                       println!("{}", "✓ AUR dependencies installed successfully.".green().bold());
                       // Add the names of dependencies that were installed via the batch command
                       for dep_name in aur_deps_to_build.into_iter().chain(cached_deps_to_install.into_iter()) {
                           if !newly_installed_deps.contains(&dep_name) {
                               newly_installed_deps.push(dep_name);
                           }
                       }
                   }
                    Err(e) => {
                        return Err(build_makepkg_error(
                           format!("Failed to execute pacman for AUR dependencies: {}", e),
                           "dependency installation",
                       ));
                    }
               }
               // Re-initialize ALPM wrapper to refresh database view after batch install
               let _alpm = AlpmWrapper::new()?;
           }

           Ok(newly_installed_deps) // Return the list of newly installed dependencies
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
