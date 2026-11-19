//! Build and settings file discovery and parsing for Gradus.

use std::path::{Path, PathBuf};

use gradus_model::{BuildFile, SettingsFile, BUILD_FILE_NAMES, SETTINGS_FILE_NAMES};

#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("no build file in '{}'\n\n  expected: build.gradus.toml or build.gradus", .dir.display())]
    NoBuildFile { dir: PathBuf },

    #[error(
        "ambiguous build file in '{}'\n\n  found: {}\n  found: {}\n\nGradus requires exactly one native build file per project.\nDelete or rename one of the two files and try again.",
        .dir.display(), .toml.display(), .bare.display()
    )]
    AmbiguousBuildFile { dir: PathBuf, toml: PathBuf, bare: PathBuf },

    #[error(
        "ambiguous settings file in '{}'\n\n  found: {}\n  found: {}\n\nGradus requires exactly one native settings file per project.\nDelete or rename one of the two files and try again.",
        .dir.display(), .toml.display(), .bare.display()
    )]
    AmbiguousSettingsFile { dir: PathBuf, toml: PathBuf, bare: PathBuf },
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
    toml::from_str(&text).map_err(|source| LoadError::Parse { path, source })
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
    let parsed = toml::from_str(&text).map_err(|source| LoadError::Parse { path, source })?;
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
}