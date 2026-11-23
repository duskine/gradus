use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::maven::resolve_dependencies;
use crate::BuildFile;

#[derive(Debug, Clone)]
pub(crate) struct JavaTools {
    javac: PathBuf,
    jar: PathBuf,
}

impl Default for JavaTools {
    fn default() -> Self {
        Self {
            javac: PathBuf::from("javac"),
            jar: PathBuf::from("jar"),
        }
    }
}

impl JavaTools {
    #[cfg(test)]
    #[cfg(unix)]
    fn new(javac: PathBuf, jar: PathBuf) -> Self {
        Self { javac, jar }
    }
}

/// Compile `src/main/java` and package the resulting classes into a JAR.
pub(crate) fn build_project(
    project_root: &Path,
    build: &BuildFile,
    tools: &JavaTools,
    offline: bool,
) -> Result<PathBuf, String> {
    let source_root = project_root.join("src/main/java");
    let mut sources = Vec::new();
    collect_java_sources(&source_root, &mut sources)
        .map_err(|e| format!("failed to scan '{}': {e}", source_root.display()))?;
    sources.sort();
    if sources.is_empty() {
        return Err(format!(
            "no Java source files found under '{}'",
            source_root.display()
        ));
    }

    let dependencies =
        resolve_dependencies(project_root, &build.dependencies.implementation, offline)
            .map_err(|error| format!("dependency resolution failed: {error}"))?;

    let build_dir = project_root.join("build");
    ensure_directory_is_safe(&build_dir, "build output directory")?;
    fs::create_dir_all(&build_dir)
        .map_err(|e| format!("failed to create '{}': {e}", build_dir.display()))?;

    let classes_root = build_dir.join("classes");
    ensure_directory_is_safe(&classes_root, "Java classes directory")?;
    if classes_root.exists() {
        fs::remove_dir_all(&classes_root)
            .map_err(|e| format!("failed to clear '{}': {e}", classes_root.display()))?;
    }
    let classes_dir = classes_root.join("java/main");
    fs::create_dir_all(&classes_dir)
        .map_err(|e| format!("failed to create '{}': {e}", classes_dir.display()))?;

    let mut javac = Command::new(&tools.javac);
    if let Some(release) = &build.project.jvm {
        javac.arg("--release").arg(release);
    }
    if !dependencies.is_empty() {
        let classpath = std::env::join_paths(
            dependencies
                .iter()
                .map(|dependency| dependency.jar_path.as_os_str()),
        )
        .map_err(|error| format!("failed to construct Java classpath: {error}"))?;
        javac.arg("--class-path").arg(classpath);
    }
    javac.arg("-d").arg(&classes_dir).args(&sources);
    let status = javac
        .status()
        .map_err(|e| format!("failed to start javac ('{}'): {e}", tools.javac.display()))?;
    if !status.success() {
        return Err(format!("javac failed with status {status}"));
    }

    let libs_dir = build_dir.join("libs");
    ensure_directory_is_safe(&libs_dir, "JAR output directory")?;
    fs::create_dir_all(&libs_dir)
        .map_err(|e| format!("failed to create '{}': {e}", libs_dir.display()))?;

    let jar_name = format!(
        "{}-{}.jar",
        safe_component(&build.project.name, "project"),
        safe_component(&build.project.version, "0.0.0")
    );
    let jar_path = libs_dir.join(jar_name);
    ensure_file_is_safe(&jar_path, "JAR output file")?;

    let status = Command::new(&tools.jar)
        .arg("--create")
        .arg("--file")
        .arg(&jar_path)
        .arg("-C")
        .arg(&classes_dir)
        .arg(".")
        .status()
        .map_err(|e| format!("failed to start jar ('{}'): {e}", tools.jar.display()))?;
    if !status.success() {
        return Err(format!("jar failed with status {status}"));
    }

    println!("compiled {} Java source file(s)", sources.len());
    Ok(jar_path)
}

/// Remove only the project's generated `build/` directory.
pub(crate) fn clean_project(project_root: &Path) -> Result<bool, String> {
    let build_dir = project_root.join("build");
    let metadata = match fs::symlink_metadata(&build_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!(
                "failed to inspect '{}': {error}",
                build_dir.display()
            ))
        }
    };

    if metadata.file_type().is_symlink() {
        return Err(format!(
            "refusing to clean '{}' because it is a symbolic link",
            build_dir.display()
        ));
    }
    if !metadata.is_dir() {
        return Err(format!(
            "refusing to clean '{}': expected a directory",
            build_dir.display()
        ));
    }

    fs::remove_dir_all(&build_dir)
        .map_err(|e| format!("failed to remove '{}': {e}", build_dir.display()))?;
    Ok(true)
}

fn collect_java_sources(dir: &Path, sources: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_java_sources(&path, sources)?;
        } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "java") {
            sources.push(path);
        }
    }
    Ok(())
}

fn ensure_directory_is_safe(path: &Path, label: &str) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "refusing to use '{}' because it is a symbolic link",
            path.display()
        )),
        Ok(metadata) if !metadata.is_dir() => Err(format!(
            "refusing to use '{}': expected a directory",
            path.display()
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to inspect {label} '{}': {error}",
            path.display()
        )),
    }
}

fn ensure_file_is_safe(path: &Path, label: &str) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "refusing to overwrite '{}' because it is a symbolic link",
            path.display()
        )),
        Ok(metadata) if !metadata.is_file() => Err(format!(
            "refusing to overwrite '{}': expected a regular file",
            path.display()
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to inspect {label} '{}': {error}",
            path.display()
        )),
    }
}

fn safe_component(value: &str, fallback: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let sanitized = sanitized.trim_matches('.');
    if sanitized.is_empty() {
        fallback.to_owned()
    } else {
        sanitized.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DependenciesSection, ProjectSection};
    use std::fs;

    fn build_file() -> BuildFile {
        BuildFile {
            gradus_version: 1,
            project: ProjectSection {
                name: "sample-app".to_owned(),
                version: "1.2.3".to_owned(),
                group: None,
                jvm: Some("17".to_owned()),
            },
            dependencies: DependenciesSection::default(),
            plugins: Vec::new(),
            android: None,
        }
    }

    #[cfg(unix)]
    #[test]
    fn builds_a_jar_using_the_configured_jvm_release() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("src/main/java/org/example/Hello.java");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, "package org.example; class Hello {}\n").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let javac = temp.path().join("fake-javac");
            let jar = temp.path().join("fake-jar");
            fs::write(
                &javac,
                "#!/bin/sh\nout=\nrelease_seen=0\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = --release ]; then shift; [ \"$1\" = 17 ] || exit 3; release_seen=1; fi\n  if [ \"$1\" = -d ]; then shift; out=\"$1\"; fi\n  shift\ndone\n[ \"$release_seen\" = 1 ] || exit 4\nmkdir -p \"$out/org/example\"\n: > \"$out/org/example/Hello.class\"\n",
            )
            .unwrap();
            fs::write(
                &jar,
                "#!/bin/sh\nout=\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = --file ]; then shift; out=\"$1\"; break; fi\n  shift\ndone\n: > \"$out\"\n",
            )
            .unwrap();
            for tool in [&javac, &jar] {
                let mut permissions = fs::metadata(tool).unwrap().permissions();
                permissions.set_mode(0o755);
                fs::set_permissions(tool, permissions).unwrap();
            }

            let output = build_project(
                temp.path(),
                &build_file(),
                &JavaTools::new(javac, jar),
                false,
            )
            .unwrap();
            assert!(output.is_file());
            assert!(temp
                .path()
                .join("build/classes/java/main/org/example/Hello.class")
                .is_file());
            assert_eq!(output.file_name().unwrap(), "sample-app-1.2.3.jar");
        }
    }

    #[test]
    fn rejects_missing_sources_instead_of_creating_an_empty_jar() {
        let temp = tempfile::tempdir().unwrap();
        let error =
            build_project(temp.path(), &build_file(), &JavaTools::default(), false).unwrap_err();
        assert!(error.contains("no Java source files"));
    }

    #[test]
    fn rejects_malformed_compile_dependency_coordinates() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("src/main/java/Example.java");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(source, "class Example {}\n").unwrap();
        let mut config = build_file();
        config.dependencies.implementation = vec!["not-a-coordinate".to_owned()];
        let error = build_project(temp.path(), &config, &JavaTools::default(), false).unwrap_err();
        assert!(error.contains("expected group:artifact:version"));
    }

    #[test]
    fn clean_removes_only_the_build_directory_and_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("build/classes")).unwrap();
        fs::write(temp.path().join("build/classes/Example.class"), "compiled").unwrap();
        fs::write(temp.path().join("keep.txt"), "keep").unwrap();

        assert!(clean_project(temp.path()).unwrap());
        assert!(!temp.path().join("build").exists());
        assert!(temp.path().join("keep.txt").is_file());
        assert!(!clean_project(temp.path()).unwrap());
    }

    #[test]
    fn unsafe_project_names_are_sanitized_for_the_jar_filename() {
        assert_eq!(safe_component("../bad/name", "project"), "_bad_name");
        assert_eq!(safe_component("...", "project"), "project");
    }
}
