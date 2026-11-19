use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, ExitCode};

use crate::java_build::{build_project, clean_project, JavaTools};
use crate::maven::resolve_dependencies;
use crate::{graph_from_build_file, load_build_file, load_settings_file, BuildFile, TaskId};
use clap::{CommandFactory, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "gradus",
    version,
    about = "A fast, native build tool for the JVM."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Scaffold a new Gradus project in the current directory.
    Init,
    /// Parse the build and settings files and report any errors.
    Validate,
    /// Print the task graph derived from the build file.
    Tasks,
    /// Compile Java sources and package them as a JAR.
    Build {
        /// Print the task plan and exit without executing.
        #[arg(long)]
        dry_run: bool,
        /// Resolve dependencies only from the lockfile and local cache.
        #[arg(long)]
        offline: bool,
    },
    /// Resolve and print declared Maven dependencies.
    Dependencies {
        /// Resolve only from the lockfile and local cache.
        #[arg(long)]
        offline: bool,
    },
    /// Check project setup and create missing standard Java source directories.
    Fix {
        /// Report the repairs that would be made without changing files.
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove the generated build/ directory.
    Clean,
}

pub(crate) fn run() -> ExitCode {
    let cli = Cli::parse();
    let code = match cli.command {
        Some(Command::Init) => cmd_init(),
        Some(Command::Validate) => cmd_validate(),
        Some(Command::Tasks) => cmd_tasks(),
        Some(Command::Build { dry_run, offline }) => cmd_build(dry_run, offline),
        Some(Command::Dependencies { offline }) => cmd_dependencies(offline),
        Some(Command::Fix { dry_run }) => cmd_fix(dry_run),
        Some(Command::Clean) => cmd_clean(),
        None => {
            Cli::command().print_help().ok();
            println!();
            0
        }
    };
    ExitCode::from(code)
}

fn cmd_init() -> u8 {
    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: cannot determine current directory: {e}");
            return 4;
        }
    };
    for name in crate::BUILD_FILE_NAMES {
        if cwd.join(name).exists() {
            eprintln!("error: '{name}' already exists in this directory");
            return 2;
        }
    }
    let project_name = cwd
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();
    let content = format!(
        "gradus_version = 1\n\n[project]\nname = \"{name}\"\nversion = \"0.1.0\"\njvm = \"17\"\n",
        name = project_name,
    );
    let path = cwd.join(crate::BUILD_FILE_NAMES[0]);
    if let Err(e) = std::fs::write(&path, content) {
        eprintln!("error: failed to write '{}': {e}", path.display());
        return 4;
    }
    println!("created {}", path.display());
    0
}

fn cmd_validate() -> u8 {
    let cwd = current_dir();
    let build = match load_or_exit(&cwd) {
        Some(b) => b,
        None => return 2,
    };
    println!(
        "build file OK: project '{}', version '{}'",
        build.project.name, build.project.version
    );
    match load_settings_file(&cwd) {
        Ok(Some(s)) => {
            println!("settings file OK: {} module(s) included", s.include.len());
        }
        Ok(None) => {
            println!("settings file: not present (single-project build)");
        }
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    }
    0
}

fn cmd_tasks() -> u8 {
    let cwd = current_dir();
    let build = match load_or_exit(&cwd) {
        Some(b) => b,
        None => return 2,
    };
    let graph = graph_from_build_file(&build);
    match graph.topological_order() {
        Ok(order) => {
            let nodes: HashMap<_, _> = graph.nodes().iter().map(|n| (n.id.clone(), n)).collect();
            for id in &order {
                if let Some(node) = nodes.get(id) {
                    if node.depends_on.is_empty() {
                        println!("{id}");
                    } else {
                        let deps: Vec<_> = node.depends_on.iter().map(|d| d.to_string()).collect();
                        println!("{id} <- {}", deps.join(", "));
                    }
                }
            }
            0
        }
        Err(cycle) => {
            eprintln!(
                "error: task graph contains a cycle: {}",
                cycle
                    .iter()
                    .map(|t| t.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            2
        }
    }
}

fn cmd_build(dry_run: bool, offline: bool) -> u8 {
    let cwd = current_dir();
    let build = match load_or_exit(&cwd) {
        Some(b) => b,
        None => return 2,
    };
    let graph = graph_from_build_file(&build);
    let root = TaskId::new("", "build");
    let order = match graph.resolve(&root) {
        Ok(o) => o,
        Err(cycle) => {
            eprintln!(
                "error: task graph contains a cycle: {}",
                cycle
                    .iter()
                    .map(|t| t.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            return 2;
        }
    };
    if dry_run {
        println!("would run:");
        for id in &order {
            println!("  {id}");
        }
        return 0;
    }

    match build_project(&cwd, &build, &JavaTools::default(), offline) {
        Ok(jar) => {
            println!("created {}", jar.display());
            0
        }
        Err(error) => {
            eprintln!("error: {error}");
            if error.starts_with("dependency resolution failed:") {
                3
            } else {
                1
            }
        }
    }
}

fn cmd_dependencies(offline: bool) -> u8 {
    let cwd = current_dir();
    let build = match load_or_exit(&cwd) {
        Some(build) => build,
        None => return 2,
    };
    if build.dependencies.implementation.is_empty() {
        println!("no implementation dependencies declared");
        return 0;
    }
    match resolve_dependencies(&cwd, &build.dependencies.implementation, offline) {
        Ok(artifacts) => {
            println!("resolved {} Maven artifact(s):", artifacts.len());
            for artifact in artifacts {
                let indent = "  ".repeat(artifact.depth);
                println!(
                    "{indent}{}  sha256:{}",
                    artifact.coordinate, artifact.sha256
                );
            }
            0
        }
        Err(error) => {
            eprintln!("error: dependency resolution failed: {error}");
            3
        }
    }
}

fn cmd_fix(dry_run: bool) -> u8 {
    let cwd = current_dir();
    if load_or_exit(&cwd).is_none() {
        return 2;
    }
    if let Err(error) = load_settings_file(&cwd) {
        eprintln!("error: {error}");
        return 2;
    }

    match ensure_java_source_directory(&cwd, dry_run) {
        Ok(true) if dry_run => println!("would create src/main/java/"),
        Ok(true) => println!("created src/main/java/"),
        Ok(false) => println!("ok: src/main/java/ exists"),
        Err(error) => {
            eprintln!("error: {error}");
            return 1;
        }
    }

    let source_root = cwd.join("src/main/java");
    if source_root.is_dir() {
        match contains_java_sources(&source_root) {
            Ok(true) => println!("ok: Java sources found under src/main/java/"),
            Ok(false) => println!(
                "note: no Java source files yet; add a .java file under src/main/java/ before building"
            ),
            Err(error) => {
                eprintln!("error: failed to inspect '{}': {error}", source_root.display());
                return 1;
            }
        }
    }

    let mut missing_tools = false;
    for tool in ["javac", "jar"] {
        if java_tool_available(tool) {
            println!("ok: {tool} is available");
        } else {
            eprintln!("needs attention: '{tool}' is not available on PATH; install a JDK");
            missing_tools = true;
        }
    }

    if missing_tools {
        1
    } else {
        println!("project setup checks completed");
        0
    }
}

fn ensure_java_source_directory(project_root: &Path, dry_run: bool) -> Result<bool, String> {
    let mut path = project_root.to_path_buf();
    let mut created = false;
    for component in ["src", "main", "java"] {
        path.push(component);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "refusing to inspect '{}' because it is a symbolic link",
                    path.display()
                ));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(format!(
                    "refusing to use '{}': expected a directory",
                    path.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if dry_run {
                    return Ok(true);
                }
                fs::create_dir(&path)
                    .map_err(|error| format!("failed to create '{}': {error}", path.display()))?;
                created = true;
            }
            Err(error) => {
                return Err(format!("failed to inspect '{}': {error}", path.display()));
            }
        }
    }
    Ok(created)
}

fn java_tool_available(tool: &str) -> bool {
    ProcessCommand::new(tool)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn contains_java_sources(directory: &Path) -> std::io::Result<bool> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_dir() && contains_java_sources(&path)? {
            return Ok(true);
        }
        if kind.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension == "java")
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn cmd_clean() -> u8 {
    match clean_project(&current_dir()) {
        Ok(true) => {
            println!("removed build artifacts");
            0
        }
        Ok(false) => {
            println!("nothing to clean");
            0
        }
        Err(error) => {
            eprintln!("error: {error}");
            1
        }
    }
}

fn current_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn load_or_exit(dir: &Path) -> Option<BuildFile> {
    match load_build_file(dir) {
        Ok(b) => Some(b),
        Err(e) => {
            eprintln!("error: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ensure_java_source_directory;

    #[test]
    fn fix_creates_only_the_missing_java_source_directories() {
        let temp = tempfile::tempdir().unwrap();
        assert!(ensure_java_source_directory(temp.path(), false).unwrap());
        assert!(temp.path().join("src/main/java").is_dir());
        assert!(!temp.path().join("src/test").exists());
        assert!(!ensure_java_source_directory(temp.path(), false).unwrap());
    }

    #[test]
    fn fix_dry_run_does_not_change_the_project() {
        let temp = tempfile::tempdir().unwrap();
        assert!(ensure_java_source_directory(temp.path(), true).unwrap());
        assert!(!temp.path().join("src").exists());
    }
}
