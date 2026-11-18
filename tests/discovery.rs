use std::fs;
use std::path::Path;

use gradus_config::{find_build_file, find_settings_file, DiscoveryError};
use tempfile::TempDir;

fn touch(dir: &Path, name: &str) {
    fs::write(dir.join(name), "").unwrap();
}

#[test]
fn finds_toml_build_file() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "build.gradus.toml");
    let path = find_build_file(tmp.path()).unwrap();
    assert_eq!(path.file_name().unwrap(), "build.gradus.toml");
}

#[test]
fn finds_bare_build_file() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "build.gradus");
    let path = find_build_file(tmp.path()).unwrap();
    assert_eq!(path.file_name().unwrap(), "build.gradus");
}

#[test]
fn both_native_build_files_is_ambiguous() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "build.gradus.toml");
    touch(tmp.path(), "build.gradus");
    let err = find_build_file(tmp.path()).unwrap_err();
    assert!(matches!(err, DiscoveryError::AmbiguousBuildFile { .. }));
    let msg = err.to_string();
    assert!(msg.contains("build.gradus.toml"));
    assert!(msg.contains("build.gradus"));
    assert!(msg.contains("Delete or rename"));
}

#[test]
fn compat_build_file_is_ignored_while_native_present() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "build.gradus.toml");
    touch(tmp.path(), "build.gradle");
    let path = find_build_file(tmp.path()).unwrap();
    assert_eq!(path.file_name().unwrap(), "build.gradus.toml");
}

#[test]
fn compat_build_file_alone_is_not_found() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "build.gradle");
    let err = find_build_file(tmp.path()).unwrap_err();
    assert!(matches!(err, DiscoveryError::NoBuildFile { .. }));
}

#[test]
fn missing_settings_file_is_ok() {
    let tmp = TempDir::new().unwrap();
    assert!(find_settings_file(tmp.path()).unwrap().is_none());
}

#[test]
fn both_native_settings_files_is_ambiguous() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "settings.gradus.toml");
    touch(tmp.path(), "settings.gradus");
    let err = find_settings_file(tmp.path()).unwrap_err();
    assert!(matches!(err, DiscoveryError::AmbiguousSettingsFile { .. }));
}
