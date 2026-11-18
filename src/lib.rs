//! Gradus — a fast, native build tool for the JVM.
//!
//! This crate is the public façade for the Gradus workspace. It currently
//! exposes the configuration schema, file discovery, and the core task
//! graph types. The engine, resolver, and JVM task runner are under
//! active development.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Returns the current Gradus version string.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

// === File discovery ===

/// The two valid native build file names.
pub const BUILD_FILE_NAMES: [&str; 2] = ["build.gradus.toml", "build.gradus"];

/// The two valid native settings file names.
pub const SETTINGS_FILE_NAMES: [&str; 2] = ["settings.gradus.toml", "settings.gradus"];

/// Gradle compatibility file names. Recognized as out of scope while a
/// native build file is present.
pub const COMPAT_BUILD_FILE_NAMES: [&str; 2] = ["build.gradle.kts", "build.gradle"];

#[derive(Debug)]
pub enum DiscoveryError {
    NoBuildFile {
        dir: PathBuf,
    },
    AmbiguousBuildFile {
        dir: PathBuf,
        toml: PathBuf,
        bare: PathBuf,
    },
    AmbiguousSettingsFile {
        dir: PathBuf,
        toml: PathBuf,
        bare: PathBuf,
    },
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoBuildFile { dir } => write!(
                f,
                "no build file in '{}'\n\n  expected: build.gradus.toml or build.gradus",
                dir.display()
            ),
            Self::AmbiguousBuildFile { dir, toml, bare } => write!(
                f,
                "ambiguous build file in '{}'\n\n  found: {}\n  found: {}\n\nGradus requires exactly one native build file per project.\nDelete or rename one of the two files and try again.",
                dir.display(),
                toml.display(),
                bare.display()
            ),
            Self::AmbiguousSettingsFile { dir, toml, bare } => write!(
                f,
                "ambiguous settings file in '{}'\n\n  found: {}\n  found: {}\n\nGradus requires exactly one native settings file per project.\nDelete or rename one of the two files and try again.",
                dir.display(),
                toml.display(),
                bare.display()
            ),
        }
    }
}

impl std::error::Error for DiscoveryError {}

/// Locate the native build file in `dir`.
///
/// Both native spellings are valid. If both are present, discovery is
/// ambiguous and returns an error rather than picking one silently.
/// Compat build files are ignored while a native file is present.
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

/// Locate the native settings file in `dir`, if any.
///
/// A missing settings file is not an error; it means a single-project
/// build. If both native spellings are present, discovery is ambiguous
/// and returns an error.
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

// === Configuration schema ===

/// Deserialized form of `build.gradus.toml` or `build.gradus`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildFile {
    pub schema_version: u32,
    pub project: ProjectSection,
    #[serde(default)]
    pub dependencies: DependenciesSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSection {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub jvm: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DependenciesSection {
    #[serde(default)]
    pub implementation: Vec<String>,
    #[serde(default)]
    pub test: Option<DependencyScope>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DependencyScope {
    #[serde(default)]
    pub implementation: Vec<String>,
}

// === Task graph ===

/// Globally unique identity of a task: project path plus task name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TaskId {
    pub project: String,
    pub task: String,
}

impl TaskId {
    pub fn new(project: impl Into<String>, task: impl Into<String>) -> Self {
        Self {
            project: project.into(),
            task: task.into(),
        }
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.project.is_empty() {
            write!(f, "{}", self.task)
        } else {
            write!(f, ":{}:{}", self.project, self.task)
        }
    }
}

/// A node in the task graph.
#[derive(Debug, Clone)]
pub struct TaskNode {
    pub id: TaskId,
    pub depends_on: Vec<TaskId>,
}

/// A task graph: nodes plus validated edges.
#[derive(Debug, Default, Clone)]
pub struct TaskGraph {
    nodes: Vec<TaskNode>,
}

impl TaskGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, node: TaskNode) {
        self.nodes.push(node);
    }

    pub fn nodes(&self) -> &[TaskNode] {
        &self.nodes
    }

    /// Return a topological ordering of the graph, or the set of nodes
    /// involved in a cycle if one exists.
    pub fn topological_order(&self) -> Result<Vec<TaskId>, Vec<TaskId>> {
        use std::collections::{HashMap, VecDeque};

        let mut indegree: HashMap<&TaskId, usize> = self
            .nodes
            .iter()
            .map(|n| (&n.id, n.depends_on.len()))
            .collect();

        let mut dependents: HashMap<&TaskId, Vec<&TaskId>> = HashMap::new();
        for node in &self.nodes {
            for dep in &node.depends_on {
                dependents.entry(dep).or_default().push(&node.id);
            }
        }

        let mut ready: VecDeque<&TaskId> = self
            .nodes
            .iter()
            .filter(|n| indegree[&n.id] == 0)
            .map(|n| &n.id)
            .collect();

        let mut order = Vec::with_capacity(self.nodes.len());
        while let Some(id) = ready.pop_front() {
            order.push(id.clone());
            if let Some(deps) = dependents.get(id) {
                for d in deps {
                    if let Some(e) = indegree.get_mut(*d) {
                        *e -= 1;
                        if *e == 0 {
                            ready.push_back(*d);
                        }
                    }
                }
            }
        }

        if order.len() == self.nodes.len() {
            Ok(order)
        } else {
            let cycle = self
                .nodes
                .iter()
                .filter(|n| !order.contains(&n.id))
                .map(|n| n.id.clone())
                .collect();
            Err(cycle)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_manifest() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn task_id_display_uses_project_path() {
        assert_eq!(TaskId::new("app", "build").to_string(), ":app:build");
        assert_eq!(TaskId::new("", "build").to_string(), "build");
    }

    #[test]
    fn topological_order_respects_dependencies() {
        let mut g = TaskGraph::new();
        g.add(TaskNode {
            id: TaskId::new("", "clean"),
            depends_on: vec![],
        });
        g.add(TaskNode {
            id: TaskId::new("", "compile"),
            depends_on: vec![TaskId::new("", "clean")],
        });
        g.add(TaskNode {
            id: TaskId::new("", "test"),
            depends_on: vec![TaskId::new("", "compile")],
        });
        let order = g.topological_order().unwrap();
        assert_eq!(order.first(), Some(&TaskId::new("", "clean")));
        assert_eq!(order.last(), Some(&TaskId::new("", "test")));
    }

    #[test]
    fn cycle_is_reported_not_hidden() {
        let mut g = TaskGraph::new();
        g.add(TaskNode {
            id: TaskId::new("", "a"),
            depends_on: vec![TaskId::new("", "b")],
        });
        g.add(TaskNode {
            id: TaskId::new("", "b"),
            depends_on: vec![TaskId::new("", "a")],
        });
        let cycle = g.topological_order().unwrap_err();
        assert_eq!(cycle.len(), 2);
    }
}