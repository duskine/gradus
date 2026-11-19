#[test]
fn version_matches_workspace() {
    assert_eq!(gradus::version(), "0.1.2");
}

#[test]
fn facade_re_exports_are_reachable() {
    let _ = gradus::TaskId::new("app", "build");
    let _ = gradus::TaskGraph::new();
    let _ = gradus::BUILD_FILE_NAMES;
    let _ = gradus::SETTINGS_FILE_NAMES;
    let _ = gradus::COMPAT_BUILD_FILE_NAMES;
    let _: Option<gradus::SettingsFile> = None;
}
