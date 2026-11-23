//! Conservative preflight analysis for importing Gradle build scripts.
//!
//! The scanner does not execute Gradle or claim to parse its full Groovy/Kotlin
//! DSL. It reports known constructs that cannot be represented by the current
//! Gradus config model and uses a conservative fallback for opaque build logic.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationDiagnostic {
    pub level: DiagnosticLevel,
    pub code: &'static str,
    pub path: PathBuf,
    pub line: usize,
    pub message: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationReport {
    pub diagnostics: Vec<MigrationDiagnostic>,
}

impl MigrationReport {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.level == DiagnosticLevel::Error)
    }

    pub fn is_clean(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

/// Analyze one Gradle Groovy or Kotlin build/settings script without executing it.
pub fn analyze_gradle_script(path: impl AsRef<Path>, source: &str) -> MigrationReport {
    let path = path.as_ref().to_path_buf();
    let mut report = MigrationReport::default();
    let mut in_plugins_block = false;
    let mut in_android_block = false;
    let mut in_dependencies_block = false;

    for (index, raw_line) in source.lines().enumerate() {
        let line_no = index + 1;
        let line = raw_line.trim();
        if line.is_empty()
            || line.starts_with("//")
            || line.starts_with("/*")
            || line.starts_with('*')
        {
            continue;
        }
        if line.starts_with("plugins") && line.contains('{') {
            in_plugins_block = true;
            continue;
        }
        if line.starts_with("android") && line.contains('{') {
            in_android_block = true;
            continue;
        }
        if line.starts_with("dependencies") && line.contains('{') {
            in_dependencies_block = true;
            continue;
        }

        if in_plugins_block {
            if line.starts_with('}') {
                in_plugins_block = false;
                continue;
            }
            if let Some(plugin_id) = plugin_id(line) {
                if !supported_plugin(plugin_id) {
                    report.push(
                        DiagnosticLevel::Warning,
                        "GRADUS-PORT-PLUGIN-UNSUPPORTED",
                        &path,
                        line_no,
                        format!("plugin `{plugin_id}` is not in Gradus's recognized migration set"),
                        "Add an explicit Gradus plugin mapping or keep this project on Gradle; plugin declarations alone do not port plugin behavior.",
                    );
                } else {
                    report.push(
                        DiagnosticLevel::Warning,
                        "GRADUS-PORT-PLUGIN-NOT-EXECUTABLE",
                        &path,
                        line_no,
                        format!("plugin `{plugin_id}` can be recorded, but Gradus does not execute Gradle plugins"),
                        "Verify every feature contributed by this plugin. Use Gradle for execution until Gradus implements an equivalent native integration.",
                    );
                }
            } else if line.contains("alias(") || line.starts_with("id(") || line.starts_with("id ")
            {
                report.push(
                    DiagnosticLevel::Error,
                    "GRADUS-PORT-PLUGIN-DYNAMIC",
                    &path,
                    line_no,
                    "plugin declaration is dynamic or uses a version-catalog alias and cannot be resolved safely by the static importer".into(),
                    "Replace it with a literal plugin ID and version, or provide an explicit migration mapping.",
                );
            } else if !line.starts_with('}') {
                report.push(
                    DiagnosticLevel::Warning,
                    "GRADUS-PORT-PLUGIN-SYNTAX",
                    &path,
                    line_no,
                    "plugin block contains syntax the importer does not recognize".into(),
                    "Use literal `id 'plugin.id' version 'x.y.z'` declarations or map this plugin expression manually.",
                );
            }
            continue;
        }

        if contains_any(
            line,
            &[
                "buildscript {",
                "apply plugin:",
                "apply(plugin",
                "plugins.apply(",
            ],
        ) {
            report.push(
                DiagnosticLevel::Error,
                "GRADUS-PORT-UNSUPPORTED-SYNTAX",
                &path,
                line_no,
                "imperative plugin/buildscript application is outside the supported Gradle import subset".into(),
                "Move plugin declarations into a literal plugins block and migrate buildscript classpaths separately.",
            );
        }

        if contains_any(
            line,
            &[
                "tasks.register(",
                "tasks.create(",
                "task ",
                "doLast",
                "doFirst",
                "subprojects {",
                "allprojects {",
                "sourceSets {
",
                "productFlavors {",
                "externalNativeBuild",
                "configureEach",
                "withType(",
            ],
        ) {
            report.push(
                DiagnosticLevel::Error,
                "GRADUS-PORT-UNSUPPORTED-SYNTAX",
                &path,
                line_no,
                "custom task or advanced Gradle build logic cannot be translated by the current importer".into(),
                "Port this behavior to a supported Gradus task/integration manually; do not drop it during migration.",
            );
        }

        if in_android_block
            && contains_any(
                line,
                &[
                    "productFlavors",
                    "buildFeatures",
                    "compileOptions",
                    "packagingOptions",
                    "lint {",
                ],
            )
        {
            report.push(
                DiagnosticLevel::Warning,
                "GRADUS-PORT-ANDROID-SETTING-UNSUPPORTED",
                &path,
                line_no,
                "Android DSL setting is not represented in the current Gradus Android schema".into(),
                "Preserve this setting in Gradle or add a Gradus schema field and exporter mapping before migrating.",
            );
        }
        if in_dependencies_block
            && (line.contains("platform(") || line.contains("project(") || line.contains("files("))
        {
            report.push(
                DiagnosticLevel::Warning,
                "GRADUS-PORT-DEPENDENCY-UNSUPPORTED",
                &path,
                line_no,
                "dependency declaration is not a fixed Maven coordinate supported by the current import schema".into(),
                "Migrate project/file/platform dependencies manually and verify dependency constraints and variants.",
            );
        }
        if line.starts_with('}') {
            in_android_block = false;
            in_dependencies_block = false;
        }
    }

    report
}

impl MigrationReport {
    fn push(
        &mut self,
        level: DiagnosticLevel,
        code: &'static str,
        path: &Path,
        line: usize,
        message: String,
        suggestion: &str,
    ) {
        self.diagnostics.push(MigrationDiagnostic {
            level,
            code,
            path: path.to_path_buf(),
            line,
            message,
            suggestion: suggestion.to_owned(),
        });
    }
}

fn plugin_id(line: &str) -> Option<&str> {
    let line = line.trim().trim_end_matches(';');
    let after_id = line
        .strip_prefix("id(")
        .or_else(|| line.strip_prefix("id "))?;
    if let Some(rest) = after_id.strip_prefix('\'') {
        let end = rest.find('\'')?;
        return Some(&rest[..end]);
    }
    if let Some(rest) = after_id.strip_prefix('"') {
        let end = rest.find('"')?;
        return Some(&rest[..end]);
    }
    None
}

fn supported_plugin(id: &str) -> bool {
    matches!(
        id,
        "java"
            | "java-library"
            | "com.android.application"
            | "com.android.library"
            | "org.jetbrains.kotlin.android"
            | "org.jetbrains.kotlin.jvm"
    )
}

fn contains_any(line: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| line.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::{analyze_gradle_script, DiagnosticLevel};

    #[test]
    fn reports_unknown_plugin_with_location_and_guidance() {
        let report = analyze_gradle_script(
            "app/build.gradle.kts",
            "plugins {\n    id(\"com.acme.custom\") version \"1.2\"\n}\n",
        );
        let diagnostic = &report.diagnostics[0];
        assert_eq!(diagnostic.level, DiagnosticLevel::Warning);
        assert_eq!(diagnostic.line, 2);
        assert_eq!(diagnostic.code, "GRADUS-PORT-PLUGIN-UNSUPPORTED");
        assert!(diagnostic.suggestion.contains("mapping"));
    }

    #[test]
    fn unsupported_task_syntax_is_a_blocking_error() {
        let report = analyze_gradle_script(
            "build.gradle",
            "tasks.register(\"generate\") {\n    doLast { println(\"x\") }\n}\n",
        );
        assert!(report.has_errors());
        assert!(report
            .diagnostics
            .iter()
            .any(|item| item.code == "GRADUS-PORT-UNSUPPORTED-SYNTAX"));
    }

    #[test]
    fn recognizes_supported_plugin_but_warns_it_is_not_executed() {
        let report = analyze_gradle_script(
            "build.gradle",
            "plugins {\n    id 'com.android.application' version '8.7.3'\n}\n",
        );
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(
            report.diagnostics[0].code,
            "GRADUS-PORT-PLUGIN-NOT-EXECUTABLE"
        );
    }
}
