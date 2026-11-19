use gradus_core::graph_from_build_file;
use gradus_model::{BuildFile, ProjectSection};

fn fixture() -> BuildFile {
    BuildFile {
        gradus_version: 1,
        project: ProjectSection {
            name: "example".into(),
            version: "0.1.0".into(),
            jvm: Some("17".into()),
        },
        dependencies: Default::default(),
    }
}

#[test]
fn graph_has_four_tasks() {
    let g = graph_from_build_file(&fixture());
    assert_eq!(g.nodes().len(), 4);
}

#[test]
fn declared_dependencies_add_resolution_before_compile() {
    let mut build = fixture();
    build.dependencies.implementation = vec!["org.example:library:1.0".into()];
    let graph = graph_from_build_file(&build);

    let compile = graph
        .nodes()
        .iter()
        .find(|node| node.id.task == "compile")
        .unwrap();
    assert_eq!(compile.depends_on[0].task, "resolveDependencies");
    assert!(graph
        .nodes()
        .iter()
        .any(|node| node.id.task == "resolveDependencies"));
}

#[test]
fn build_depends_on_jar_only_until_test_execution_is_implemented() {
    let g = graph_from_build_file(&fixture());
    let node = g.nodes().iter().find(|n| n.id.task == "build").unwrap();
    let deps: Vec<_> = node.depends_on.iter().map(|d| d.task.as_str()).collect();
    assert_eq!(deps, vec!["jar"]);
}

#[test]
fn resolve_build_excludes_clean() {
    let g = graph_from_build_file(&fixture());
    let tasks = g.resolve(&gradus_model::TaskId::new("", "build")).unwrap();
    let names: Vec<_> = tasks.iter().map(|t| t.task.as_str()).collect();
    assert!(!names.contains(&"clean"));
    assert!(!names.contains(&"test"));
    assert!(names.contains(&"build"));
    assert!(names.contains(&"compile"));
    assert_eq!(names.last(), Some(&"build"));
}

#[test]
fn resolve_clean_is_just_clean() {
    let g = graph_from_build_file(&fixture());
    let tasks = g.resolve(&gradus_model::TaskId::new("", "clean")).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].task, "clean");
}
