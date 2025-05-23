//! Comprehensive tests for lilac core modules

#[cfg(test)]
mod tests {
    use lilac_aur::error::*;
    use lilac_aur::config::AppConfig;
    use tempfile;
    use assert_cmd::Command;
    use predicates::prelude::*;

    #[test]
    fn test_aur_error_display() {
        let e = AurError::RequestFailed("fail".into());
        assert!(format!("{}", e).contains("AUR request failed"));
        let e = AurError::ParseError("fail".into());
        assert!(format!("{}", e).contains("Failed to parse AUR response"));
        let e = AurError::NotFound("foo".into());
        assert!(format!("{}", e).contains("Package not found in AUR"));
        let e = AurError::ApiError("fail".into());
        assert!(format!("{}", e).contains("AUR API error"));
    }

    #[test]
    fn test_build_error_display() {
        let e = BuildError::GitError { source: "fail".into(), package: "foo".into() };
        assert!(format!("{}", e).contains("Git operation failed"));
        let e = BuildError::MakePkgError { source: "fail".into(), stage: "bar".into() };
        assert!(format!("{}", e).contains("makepkg failed during"));
    }

    #[test]
    fn test_alpm_error_display() {
        let e = AlpmError::InitError("fail".into());
        assert!(format!("{}", e).contains("ALPM initialization failed"));
        let e = AlpmError::InstallError("fail".into());
        assert!(format!("{}", e).contains("Package installation failed"));
        let e = AlpmError::DatabaseError("fail".into());
        assert!(format!("{}", e).contains("Database operation failed"));
        let e = AlpmError::RemoveError("fail".into());
        assert!(format!("{}", e).contains("Package removal failed"));
        let e = AlpmError::NotFound("foo".into());
        assert!(format!("{}", e).contains("Package not found in ALPM"));
    }

    #[test]
    fn test_config_load_and_cache_path() {
        let config = AppConfig::load().expect("Should load config");
        let cache_path = config.cache_path().expect("Should get cache path");
        assert!(cache_path.ends_with(".cache/lilac"));
        assert!(cache_path.exists());
        let temp_path = config.temp_path();
        assert!(temp_path.exists());
    }

    #[test]
    fn test_config_temp_dir_is_unique() {
        let config1 = AppConfig::load().unwrap();
        let config2 = AppConfig::load().unwrap();
        assert_ne!(config1.temp_path(), config2.temp_path());
    }

    #[test]
    fn test_error_helpers() {
        let _ = aur_request_failed("fail");
        let _ = aur_parse_error("fail");
        let _ = aur_api_error("fail");
        let _ = alpm_init_error("fail");
        let _ = alpm_install_error("fail");
        let _ = alpm_remove_error("fail");
        let _ = build_git_error("fail", "foo");
        let _ = build_makepkg_error("fail", "bar");
    }

    #[test]
    fn test_packagebuilder_find_cached_package_none() {
        use lilac::build::PackageBuilder;
        let temp = tempfile::tempdir().unwrap();
        let found = PackageBuilder::find_cached_package(temp.path(), "notapkg");
        assert!(found.is_none());
    }

    #[test]
    fn test_packagebuilder_read_dependency_list_empty() {
        use lilac::build::PackageBuilder;
        let temp = tempfile::tempdir().unwrap();
        let deps = PackageBuilder::read_dependency_list("notapkg", temp.path()).unwrap();
        assert!(deps.is_empty());
    }

    #[test]
    fn test_aurpackage_struct() {
        use lilac::aur::AurPackage;
        let pkg = AurPackage {
            name: "foo".into(),
            version: "1.0".into(),
            description: Some("desc".into()),
            url: Some("http://foo".into()),
            maintainer: Some("me".into()),
            num_votes: 1,
            popularity: 0.1,
            first_submitted: 0,
            last_modified: 0,
        };
        assert_eq!(pkg.name, "foo");
        assert_eq!(pkg.version, "1.0");
    }

    #[test]
    fn test_help_command() {
        Command::cargo_bin("lilac")
            .unwrap()
            .arg("--help")
            .assert()
            .success()
            .stdout(predicate::str::contains("Usage: lilac"));
    }
} 
