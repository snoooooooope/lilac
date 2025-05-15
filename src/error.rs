use std::fmt;
use config::ConfigError;

/// AUR module errors
#[derive(Debug)]
pub enum AurError {
    RequestFailed(String),
    ParseError(String),
    NotFound(String),
    ApiError(String),
}

/// Build module errors
#[derive(Debug)]
pub enum BuildError {
    GitError { source: String, package: String },
    MakePkgError { source: String, stage: String },
}

/// ALPM module errors
#[derive(Debug)]
pub enum AlpmError {
    InitError(String),
    InstallError(String),
    DatabaseError(String),
    RemoveError(String),
    NotFound(String),
}

// Implement Display for error types
impl fmt::Display for AurError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AurError::RequestFailed(e) => write!(f, "AUR request failed: {}", e),
            AurError::ParseError(e) => write!(f, "Failed to parse AUR response: {}", e),
            AurError::NotFound(e) => write!(f, "Package not found in AUR: {}", e),
            AurError::ApiError(e) => write!(f, "AUR API error: {}", e),
        }
    }
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::GitError { source, package } => 
                write!(f, "Git operation failed (package: {}): {}", package, source),
            BuildError::MakePkgError { source, stage } => 
                write!(f, "makepkg failed during {}: {}", stage, source),
        }
    }
}

impl fmt::Display for AlpmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AlpmError::InitError(e) => write!(f, "ALPM initialization failed: {}", e),
            AlpmError::InstallError(e) => write!(f, "Package installation failed: {}", e),
            AlpmError::DatabaseError(e) => write!(f, "Database operation failed: {}", e),
            AlpmError::RemoveError(e) => write!(f, "Package removal failed: {}", e),
            AlpmError::NotFound(e) => write!(f, "Package not found in ALPM: {}", e),
        }
    }
}

// Error traits for error types
impl std::error::Error for AurError {}
impl std::error::Error for BuildError {}
impl std::error::Error for AlpmError {}

// Helper functions for creating errors
pub fn aur_request_failed(e: impl Into<String>) -> AurError {
    AurError::RequestFailed(e.into())
}

pub fn aur_parse_error(e: impl Into<String>) -> AurError {
    AurError::ParseError(e.into())
}

pub fn aur_api_error(e: impl Into<String>) -> AurError {
    AurError::ApiError(e.into())
}

pub fn alpm_init_error(e: impl Into<String>) -> AlpmError {
    AlpmError::InitError(e.into())
}

pub fn alpm_install_error(e: impl Into<String>) -> AlpmError {
    AlpmError::InstallError(e.into())
}

pub fn alpm_remove_error(e: impl Into<String>) -> AlpmError {
    AlpmError::RemoveError(e.into())
}

pub fn build_git_error(source: impl Into<String>, package: impl Into<String>) -> BuildError {
    BuildError::GitError {
        source: source.into(),
        package: package.into()
    }
}

pub fn build_makepkg_error(source: impl Into<String>, stage: impl Into<String>) -> BuildError {
    BuildError::MakePkgError {
        source: source.into(),
        stage: stage.into()
    }
}

// Implementations for error types
impl From<ConfigError> for BuildError {
    fn from(err: ConfigError) -> Self {
        BuildError::MakePkgError {
            source: format!("Config error: {}", err),
            stage: "configuration".to_string(),
        }
    }
}

impl From<AlpmError> for BuildError {
    fn from(err: AlpmError) -> Self {
        BuildError::MakePkgError {
            source: format!("ALPM error: {}", err),
            stage: "dependency installation".to_string(),
        }
    }
}
