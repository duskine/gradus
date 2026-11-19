//! Shared domain types for Gradus.

use std::fmt;

use serde::{Deserialize, Serialize};

/// The two valid native build file names.
pub const BUILD_FILE_NAMES: [&str; 2] = ["build.gradus.toml", "build.gradus"];

/// The two valid native settings file names.
pub const SETTINGS_FILE_NAMES: [&str; 2] = ["settings.gradus.toml", "settings.gradus"];

/// Gradle compatibility file names. Recognized as out of scope while a
/// native build file is present.
pub const COMPAT_BUILD_FILE_NAMES: [&str; 2] = ["build.gradle.kts", "build.gradle"];

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

/// Deserialized form of `build.gradus.toml` or `build.gradus`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildFile {
    pub gradus_version: u32,
    pub project: ProjectSection,
    #[serde(default)]
    pub dependencies: DependenciesSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSection {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub jvm: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependenciesSection {
    #[serde(default)]
    pub implementation: Vec<String>,
    #[serde(default)]
    pub test: Option<DependencyScope>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyScope {
    #[serde(default)]
    pub implementation: Vec<String>,
}

/// Deserialized form of `settings.gradus.toml` or `settings.gradus`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsFile {
    pub gradus_version: u32,
    #[serde(default)]
    pub root_project: Option<String>,
    #[serde(default)]
    pub include: Vec<String>,
}
