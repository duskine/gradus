use std::fs;

use gradus_config::{load_build_file, load_settings_file};
use tempfile::TempDir;

#[test]
fn loads_custom_build_syntax_without_equals() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("build.gradus"),
        "[project]\nname 'sample-app'\nversion '1.0.0'\n[dependencies]\nimplementation ['org.example:lib:1.0']\n",
    )
    .unwrap();

    let build = load_build_file(dir.path()).unwrap();
    assert_eq!(build.project.name, "sample-app");
    assert_eq!(build.project.version, "1.0.0");
    assert_eq!(build.dependencies.implementation, ["org.example:lib:1.0"]);
}

#[test]
fn loads_toml_build_without_version_field() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("build.gradus.toml"),
        "[project]\nname = \"toml-app\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();

    let build = load_build_file(dir.path()).unwrap();
    assert_eq!(build.gradus_version, 1);
    assert_eq!(build.project.name, "toml-app");
}

#[test]
fn loads_custom_settings_syntax_without_equals() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("settings.gradus"),
        "root_project 'sample-app'\ninclude ['app', 'core']\n",
    )
    .unwrap();

    let settings = load_settings_file(dir.path()).unwrap().unwrap();
    assert_eq!(settings.root_project.as_deref(), Some("sample-app"));
    assert_eq!(settings.include, ["app", "core"]);
}

#[test]
fn loads_bracketed_root_project_settings_section() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("settings.gradus"),
        "[root_project]\nname 'sample-app'\ninclude ['app', 'core']\n",
    )
    .unwrap();

    let settings = load_settings_file(dir.path()).unwrap().unwrap();
    assert_eq!(settings.root_project.as_deref(), Some("sample-app"));
    assert_eq!(settings.include, ["app", "core"]);
}

#[test]
fn custom_syntax_errors_include_file_and_line() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("build.gradus"),
        "[project]\nname [invalid\nversion '1.0.0'\n",
    )
    .unwrap();

    let error = load_build_file(dir.path()).unwrap_err().to_string();
    assert!(error.contains("build.gradus"));
    assert!(error.contains("line 2"));
}
