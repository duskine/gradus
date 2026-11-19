# Gradus

[![Crates.io](https://img.shields.io/crates/v/gradus.svg)](https://crates.io/crates/gradus)
[![Downloads](https://img.shields.io/crates/d/gradus.svg)](https://crates.io/crates/gradus)
[![Documentation](https://docs.rs/gradus/badge.svg)](https://docs.rs/gradus)
[![CI](https://github.com/duskine/gradus/actions/workflows/ci.yml/badge.svg)](https://github.com/duskine/gradus/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](#license)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue.svg)](#)

A fast, native build tool for the JVM, written in Rust.

Gradus is a native build tool for JVM projects. Its first build path
compiles dependency-free Java sources and packages them as a JAR using a
local JDK. It reads project configuration from TOML and is not a wrapper
around Gradle; the Gradus process itself does not require a JVM to start.

## Why

JVM builds are slow to start and slow to configure. A typical Gradle
invocation spends seconds warming up a JVM and evaluating build scripts
before the first file is compiled. Most of that work is configuration,
not compilation.

Gradus takes the parts of the Gradle model that are worth keeping —
incremental builds, a task graph with real dependency tracking, Maven
dependency resolution, reproducible lockfiles — and implements them in a
native binary with a parseable config format. The goal is a build tool
that starts in milliseconds, produces the same output on every machine,
and can be read and audited without executing arbitrary code.

Status: early development. Java compilation and JAR packaging are
implemented for dependency-free projects. Dependency resolution, test
execution, Kotlin support, incremental builds, and parallel execution are
planned but not yet available.

## Features

Planned and in progress:

- Native binary, no JVM bootstrap required to start a build
- Parallel task execution over a validated DAG, with cycle detection
- Incremental builds via input and output hashing
- Local and remote build cache
- Maven dependency resolution with parent POMs, BOM imports, and
  transitive conflict resolution
- Committed lockfile for reproducible, offline-capable builds
- Broader language and task support: Kotlin, test, run, and dependency-aware Java builds
- Multi-project builds with project-relative task paths
- TOML configuration, no scripting required

Configuration files:

- `build.gradus.toml` or `build.gradus` define a project
- `settings.gradus.toml` or `settings.gradus` define multi-project includes
- Both spellings are valid and parse as TOML
- Exactly one build file per project is required; if both spellings are
  present, Gradus aborts with an error rather than guessing
- `build.gradle` and `build.gradle.kts` may coexist with a native build
  file and are ignored while a native file is present

Compatibility with Groovy and Kotlin Gradle scripts is a later milestone,
not part of the current release.

## Usage

Install from crates.io once a release is published:

```sh
cargo install gradus
```

### Create a project:

```sh
gradus init
```

### Build a Java project:

```sh
gradus validate
gradus tasks
gradus build
gradus build --dry-run
gradus clean
```

The initial build supports Java files under `src/main/java`, requires
`javac` and `jar` on `PATH`, and uses `project.jvm` as the `javac --release`
target when set. It writes classes to
`build/classes/java/main` and the JAR to `build/libs/<name>-<version>.jar`.
Declared `implementation` dependencies fail with a clear message until
Maven resolution is implemented. Tests, Kotlin sources, and running
applications are not supported yet.

### Inspect the task graph:

```sh
gradus tasks            # list the task graph
```

## A minimal build.gradus.toml:

```toml
gradus_version = 1

[project]
name = "example"
version = "0.1.0"
jvm = "17"
```

The configuration schema accepts dependency declarations, but builds with
`implementation` dependencies fail explicitly until Maven resolution is ready.

## Current CLI scope

Supported command options are `gradus build --dry-run`, `--help`, and
`--version`. Other flags such as parallel execution, caching, and offline
dependency resolution are planned but not implemented yet.

Exit codes currently used: 0 for success, 1 for build or clean failure,
2 for invalid configuration, and 4 for initialization or environment
errors. Code 3 is reserved for dependency-resolution failures.

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for the full text.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in this project by you shall be licensed under the
MIT License, without any additional terms or conditions.

## Contribution

Contributions are welcome. Before opening a pull request:

- Open an issue for anything larger than a bug fix, so the design can be
  agreed on first
- Keep changes focused; one concern per pull request
- Run `cargo fmt, cargo clippy -- -D warnings`, and `cargo test` before
  submitting
- Add tests for new behavior, including fixture projects where relevant

Discussion happens in GitHub issues. By participating you agree to the
[Code of Conduct](CODE_OF_CONDUCT.md).
