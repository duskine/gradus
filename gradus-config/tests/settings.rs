use std::fs;
use std::path::Path;

use gradus_config::{find_settings_file, load_settings_file, DiscoveryError};
use tempfile::TempDir;

fn touch(dir: &Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).unwrap();
}

#[test]
fn missing_settings_file_is_none() {
    let tmp = TempDir::new().unwrap();
    assert!(find_settings_file(tmp.path()).unwrap().is_none());
    assert!(load_settings_file(tmp.path()).unwrap().is_none());
}

#[test]
fn parses_minimal_settings() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "settings.gradus.toml", "root_project = \"demo\"\n");
    let s = load_settings_file(tmp.path()).unwrap().unwrap();
    assert_eq!(s.gradus_version, 1);
    assert!(s.include.is_empty());
    assert!(s.root_project.is_none());
}

#[test]
fn parses_settings_with_includes() {
    let tmp = TempDir::new().unwrap();
    touch(
        tmp.path(),
        "settings.gradus.toml",
        r#"gradus_version = 1
root_project = "root"
include = ["app", "lib"]
"#,
    );
    let s = load_settings_file(tmp.path()).unwrap().unwrap();
    assert_eq!(s.root_project.as_deref(), Some("root"));
    assert_eq!(s.include, vec!["app", "lib"]);
}

#[test]
fn both_settings_spellings_is_ambiguous() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "settings.gradus.toml", "gradus_version = 1\n");
    touch(tmp.path(), "settings.gradus", "gradus_version = 1\n");
    let err = find_settings_file(tmp.path()).unwrap_err();
    assert!(matches!(err, DiscoveryError::AmbiguousSettingsFile { .. }));
}

#[test]
fn unknown_settings_key_is_rejected() {
    let tmp = TempDir::new().unwrap();
    touch(
        tmp.path(),
        "settings.gradus.toml",
        "gradus_version = 1\nbogus = true\n",
    );
    let err = load_settings_file(tmp.path()).unwrap_err();
    assert!(err.to_string().contains("bogus"));
}
