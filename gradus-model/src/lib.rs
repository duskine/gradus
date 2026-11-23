//! Shared domain types for Gradus.

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
    #[serde(default = "default_gradus_version")]
    pub gradus_version: u32,
    pub project: ProjectSection,
    #[serde(default)]
    pub dependencies: DependenciesSection,
    #[serde(default)]
    pub plugins: Vec<PluginDeclaration>,
    #[serde(default)]
    pub android: Option<AndroidSection>,
}

fn default_gradus_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSection {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub jvm: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginDeclaration {
    pub id: String,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AndroidSection {
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub compile_sdk: Option<u32>,
    #[serde(default)]
    pub default_config: Option<AndroidDefaultConfig>,
    #[serde(default)]
    pub build_types: std::collections::BTreeMap<String, AndroidBuildType>,
    #[serde(default)]
    pub signing: Option<AndroidSigningConfig>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AndroidBuildType {
    #[serde(default)]
    pub debuggable: Option<bool>,
    #[serde(default)]
    pub minify_enabled: Option<bool>,
    #[serde(default)]
    pub shrink_resources: Option<bool>,
    #[serde(default)]
    pub application_id_suffix: Option<String>,
    #[serde(default)]
    pub proguard_files: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AndroidSigningConfig {
    #[serde(default)]
    pub release_keystore: Option<String>,
    #[serde(default)]
    pub release_key_alias: Option<String>,
    #[serde(default)]
    pub release_store_password: Option<String>,
    #[serde(default)]
    pub release_key_password: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AndroidDefaultConfig {
    #[serde(default)]
    pub application_id: Option<String>,
    #[serde(default)]
    pub min_sdk: Option<u32>,
    #[serde(default)]
    pub target_sdk: Option<u32>,
    #[serde(default)]
    pub version_code: Option<u32>,
    #[serde(default)]
    pub version_name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependenciesSection {
    #[serde(default)]
    pub implementation: Vec<String>,
    #[serde(default)]
    pub test_implementation: Vec<String>,
    #[serde(default)]
    pub test: Option<DependencyScope>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyScope {
    #[serde(default)]
    pub implementation: Vec<String>,
}
