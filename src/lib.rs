//! Gradus — a fast, native build tool for the JVM.
//!
//! This crate will re-export the public API of the Gradus workspace.
//! Version 0.1.0 reserves the name while the engine is under construction.

/// Returns the current Gradus version string.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}