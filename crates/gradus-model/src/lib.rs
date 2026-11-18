//! Shared domain types for Gradus.
//!
//! Pure data and identifiers. No I/O, no async, no algorithms.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Globally unique identity of a task: project path plus task name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TaskId {
    pub project: String,
    pub task: String,
}

impl TaskId {
    pub fn new(project: impl Into<String>, task: impl Into<String>) -> Self {
        Self { project: project.into(), task: task.into() }
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
