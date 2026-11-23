//! Build and settings file discovery and parsing for Gradus.

use std::path::{Path, PathBuf};

use gradus_model::BuildFile;
use serde::{Deserialize, Serialize};

mod custom;
mod migration;
mod translate;

pub use migration::{analyze_gradle_script, DiagnosticLevel, MigrationDiagnostic, MigrationReport};

pub use translate::{
    build_gradus_to_kotlin_dsl, build_model_to_kotlin_dsl, settings_gradus_to_kotlin_dsl,
    settings_model_to_kotlin_dsl, TranslationError,
};

/// The two valid native build file names.
pub const BUILD_FILE_NAMES: [&str; 2] = ["build.gradus.toml", "build.gradus"];

/// The two valid native settings file names.
pub const SETTINGS_FILE_NAMES: [&str; 2] = ["settings.gradus.toml", "settings.gradus"];

/// Gradle compatibility file names, ignored while a native build file is present.
pub const COMPAT_BUILD_FILE_NAMES: [&str; 2] = ["build.gradle.kts", "build.gradle"];

/// Deserialized form of `settings.gradus.toml` or `settings.gradus`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsFile {
    #[serde(default = "default_gradus_version")]
    pub gradus_version: u32,
    #[serde(default)]
    pub root_project: Option<String>,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub repositories: RepositorySettings,
    #[serde(default)]
    pub dependency_resolution: Option<DependencyResolutionSettings>,
}

fn default_gradus_version() -> u32 {
    1
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositorySettings {
    #[serde(default)]
    pub google: bool,
    #[serde(default)]
    pub maven_central: bool,
    #[serde(default)]
    pub gradle_plugin_portal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyResolutionSettings {
    pub mode: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("no build file in '{}'\n\n  expected: build.gradus.toml or build.gradus", .dir.display())]
    NoBuildFile { dir: PathBuf },

    #[error(
        "ambiguous build file in '{}'\n\n  found: {}\n  found: {}\n\nGradus requires exactly one native build file per project.\nDelete or rename one of the two files and try again.",
        .dir.display(), .toml.display(), .bare.display()
    )]
    AmbiguousBuildFile {
        dir: PathBuf,
        toml: PathBuf,
        bare: PathBuf,
    },

    #[error(
        "ambiguous settings file in '{}'\n\n  found: {}\n  found: {}\n\nGradus requires exactly one native settings file per project.\nDelete or rename one of the two files and try again.",
        .dir.display(), .toml.display(), .bare.display()
    )]
    AmbiguousSettingsFile {
        dir: PathBuf,
        toml: PathBuf,
        bare: PathBuf,
    },
}

pub fn find_build_file(dir: &Path) -> Result<PathBuf, DiscoveryError> {
    let toml = dir.join(BUILD_FILE_NAMES[0]);
    let bare = dir.join(BUILD_FILE_NAMES[1]);
    match (toml.is_file(), bare.is_file()) {
        (true, true) => Err(DiscoveryError::AmbiguousBuildFile {
            dir: dir.to_path_buf(),
            toml,
            bare,
        }),
        (true, false) => Ok(toml),
        (false, true) => Ok(bare),
        (false, false) => Err(DiscoveryError::NoBuildFile {
            dir: dir.to_path_buf(),
        }),
    }
}

pub fn find_settings_file(dir: &Path) -> Result<Option<PathBuf>, DiscoveryError> {
    let toml = dir.join(SETTINGS_FILE_NAMES[0]);
    let bare = dir.join(SETTINGS_FILE_NAMES[1]);
    match (toml.is_file(), bare.is_file()) {
        (true, true) => Err(DiscoveryError::AmbiguousSettingsFile {
            dir: dir.to_path_buf(),
            toml,
            bare,
        }),
        (true, false) => Ok(Some(toml)),
        (false, true) => Ok(Some(bare)),
        (false, false) => Ok(None),
    }
}

pub fn load_build_file(dir: &Path) -> Result<BuildFile, LoadError> {
    let path = find_build_file(dir)?;
    let text = std::fs::read_to_string(&path).map_err(|source| LoadError::Io {
        path: path.clone(),
        source,
    })?;
    let parsed = if path.file_name().and_then(|name| name.to_str()) == Some("build.gradus") {
        let normalized = custom::to_toml(&text).map_err(|source| LoadError::CustomParse {
            path: path.clone(),
            message: source.to_string(),
        })?;
        toml::from_str(&normalized).map_err(|source| LoadError::Parse { path, source })?
    } else {
        toml::from_str(&text).map_err(|source| LoadError::Parse { path, source })?
    };
    Ok(parsed)
}

pub fn load_settings_file(dir: &Path) -> Result<Option<SettingsFile>, LoadError> {
    let path = match find_settings_file(dir)? {
        Some(p) => p,
        None => return Ok(None),
    };
    let text = std::fs::read_to_string(&path).map_err(|source| LoadError::Io {
        path: path.clone(),
        source,
    })?;
    let parsed = if path.file_name().and_then(|name| name.to_str()) == Some("settings.gradus") {
        let normalized = custom::to_toml(&text).map_err(|source| LoadError::CustomParse {
            path: path.clone(),
            message: source.to_string(),
        })?;
        toml::from_str(&normalized).map_err(|source| LoadError::Parse { path, source })?
    } else {
        toml::from_str(&text).map_err(|source| LoadError::Parse { path, source })?
    };
    Ok(Some(parsed))
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error(transparent)]
    Discovery(#[from] DiscoveryError),

    #[error("failed to read '{}': {source}", .path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse '{}': {source}", .path.display())]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("failed to parse '{}': {message}", .path.display())]
    CustomParse { path: PathBuf, message: String },
}
