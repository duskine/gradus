use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand};
use gradus::{
    find_settings_file, graph_from_build_file, load_build_file, load_settings_file, BuildFile,
    TaskGraph, TaskId,
};

#[derive(Parser)]
#[command(name = "gradus", version, about = "A fast, native build tool for the JVM.")]
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
    /// Show what would run, without executing anything.
    Build {
        /// Print the task plan and exit without executing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove build artifacts. Reserved for a future release.
    Clean,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let code = match cli.command {
        Some(Command::Init) => cmd_init(),
        Some(Command::Validate) => cmd_validate(),
        Some(Command::Tasks) => cmd_tasks(),
        Some(Command::Build { dry_run }) => cmd_build(dry_run),
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
    for name in gradus::BUILD_FILE_NAMES {
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
    let path = cwd.join(gradus::BUILD_FILE_NAMES[0]);
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
            let nodes: HashMap<_, _> =
                graph.nodes().iter().map(|n| (n.id.clone(), n)).collect();
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

fn cmd_build(dry_run: bool) -> u8 {
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
    if !dry_run {
        eprintln!("note: build execution is not implemented in 0.1.2.");
        eprintln!("re-run with --dry-run to see the task plan.");
        return 4;
    }
    println!("would run:");
    for id in &order {
        println!("  {id}");
    }
    0
}

fn cmd_clean() -> u8 {
    eprintln!("note: clean execution is not implemented in 0.1.2.");
    4
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

// silence unused-import warnings for paths used in error branches
#[allow(dead_code)]
fn _unused(_: Option<TaskGraph>) {
    let _ = find_settings_file;
}