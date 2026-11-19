//! Gradus — a fast, native build tool for the JVM.
//!
//! This is the public façade crate. It re-exports the domain model,
//! configuration discovery, and task graph from the workspace so
//! downstream users have a single entry point.

mod cli;
mod java_build;
mod maven;

pub use gradus_config::*;
pub use gradus_core::*;
pub use gradus_model::*;

/// Returns the current Gradus version string.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Entry point used by the `gradus` executable.
#[doc(hidden)]
pub fn cli_main() -> std::process::ExitCode {
    cli::run()
}
