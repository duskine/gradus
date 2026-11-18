use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "gradus", version, about = "A fast, native build tool for the JVM.")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Scaffold a new Gradus project.
    Init,
    /// Compile, test, and package.
    Build,
    /// Run the test suite.
    Test,
    /// Remove build artifacts.
    Clean,
    /// List the task graph.
    Tasks,
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Some(Command::Init) => not_yet("init"),
        Some(Command::Build) => not_yet("build"),
        Some(Command::Test) => not_yet("test"),
        Some(Command::Clean) => not_yet("clean"),
        Some(Command::Tasks) => not_yet("tasks"),
        None => {
            use clap::CommandFactory;
            Cli::command().print_help().ok();
            println!();
            0
        }
    };
    std::process::exit(code);
}

fn not_yet(cmd: &str) -> i32 {
    eprintln!("gradus {cmd}: not implemented yet (see roadmap)");
    4
}
