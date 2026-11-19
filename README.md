# Gradus

[![Crates.io](https://img.shields.io/crates/v/gradus.svg)](https://crates.io/crates/gradus)
[![Downloads](https://img.shields.io/crates/d/gradus.svg)](https://crates.io/crates/gradus)
[![Documentation](https://docs.rs/gradus/badge.svg)](https://docs.rs/gradus)
[![CI](https://github.com/duskine/gradus/actions/workflows/ci.yml/badge.svg)](https://github.com/duskine/gradus/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](#license)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue.svg)](#)

A fast, native build tool for the JVM, written in Rust.

Gradus is a native build tool for JVM projects. Its first build path
compiles Java sources and packages them as a JAR using a local JDK. It can
resolve fixed-version Maven dependencies into the compile classpath. It reads
project configuration from TOML and is not a wrapper around Gradle; Gradus
itself does not require a JVM to start.

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

Status: early development. Java compilation, JAR packaging, and a bounded
Maven Central resolver are implemented. Test execution, Kotlin support,
incremental builds, and parallel execution are not available yet.

## Features

Planned and in progress:

- Native binary, no JVM bootstrap required to start a build
- Parallel task execution over a validated DAG, with cycle detection
- Incremental builds via input and output hashing
- Local and remote build cache
- Maven Central resolution for fixed coordinates, including transitive JARs,
  basic parent POM inheritance, and nearest-first version selection
- `gradus.lock` records resolved artifacts and SHA-256 checksums for offline builds
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

Install the Gradus program from crates.io after version 0.1.3 is published:

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
gradus fix
gradus dependencies
gradus dependencies --offline
gradus build
gradus build --dry-run
gradus build --offline
gradus clean
```

`gradus fix` validates the project configuration, creates a missing
`src/main/java` directory, and checks whether `javac` and `jar` are
available. It does not install system packages or alter shell settings.
If no `.java` files exist yet, it reports that they must be added before
building. Use `gradus fix --dry-run` to preview directory creation.

The build supports Java files under `src/main/java`, requires `javac` and
`jar` on `PATH`, and uses `project.jvm` as the `javac --release` target when
set. It writes classes to `build/classes/java/main` and the JAR to
`build/libs/<name>-<version>.jar`. Direct dependencies use
`group:artifact:version` strings in `dependencies.implementation` and are
resolved from Maven Central. Set `GRADUS_MAVEN_REPOSITORY` to use an HTTP(S)
mirror, or `GRADUS_CACHE_DIR` to change the artifact cache location.

An online build writes `gradus.lock`; commit it with the project. Offline
builds read the lockfile, require the corresponding JARs in Gradus's local
cache, and verify each SHA-256 checksum. The initial resolver supports
fixed versions, parent POMs, properties, dependency management (including
BOM imports), exclusions, and compile-scope transitive JAR dependencies.
Runtime, provided, test, and optional transitive dependencies are omitted
from the compile classpath. Version ranges, snapshots, non-JAR dependency
types, and property-based parent coordinates are not supported yet. Tests,
Kotlin sources, and running applications are not supported yet.

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

Example dependency declaration:

```toml
[dependencies]
implementation = ["org.apache.commons:commons-lang3:3.14.0"]
```

## Current CLI scope

The `gradus` package provides the command-line executable as well as the
library API. `gradus-cli` remains available as a compatibility package.
Offline dependency resolution is available with `--offline`. Parallel
execution, incremental compilation, tests, and runtime launching remain
planned.

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
