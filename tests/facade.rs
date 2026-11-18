#[test]
fn version_matches_workspace() {
    assert_eq!(gradus::version(), "0.1.1");
}

#[test]
fn facade_re_exports_are_reachable() {
    // These compile only if the re-exports work.
    let _ = gradus::TaskId::new("app", "build");
    let _ = gradus::TaskGraph::new();
    let _ = gradus::BUILD_FILE_NAMES;
}
