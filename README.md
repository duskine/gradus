# Gradus

[![Crates.io](https://img.shields.io/crates/v/gradus.svg)](https://crates.io/crates/gradus)
[![Downloads](https://img.shields.io/crates/d/gradus.svg)](https://crates.io/crates/gradus)
[![Documentation](https://docs.rs/gradus/badge.svg)](https://docs.rs/gradus)
[![CI](https://github.com/duskine/gradus/actions/workflows/ci.yml/badge.svg)](https://github.com/duskine/gradus/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![MSRV](https://img.shields.io/badge/MSRV-1.75-blue.svg)](#)

A fast, native build tool for the JVM, written in Rust.

Gradus compiles and packages Java and Kotlin projects. It reads its
configuration from TOML, resolves dependencies from Maven repositories,
and schedules tasks on a parallel DAG. It is not a wrapper around Gradle
and does not require a JVM to start.

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

Status: early development. The engine, dependency resolver, and JVM task
runner are in progress. See the roadmap below for what works today.

## Features

Planned and in progress:

- Native binary, no JVM bootstrap required to start a build
- Parallel task execution over a validated DAG, with cycle detection
- Incremental builds via input and output hashing
- Local and remote build cache
- Maven dependency resolution with parent POMs, BOM imports, and
  transitive conflict resolution
- Committed lockfile for reproducible, offline-capable builds
- First-class Java and Kotlin tasks: compile, test, jar, run
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

### Build, test, and run:

```sh
gradus build
gradus test
gradus run
gradus clean
```

### Inspect the build:

```sh
gradus tasks            # list the task graph
gradus dependencies     # print the resolved dependency tree
```

## A minimal build.gradus.toml:

```toml
gradus_version = 1

[project]
name = "example"
version = "0.1.0"
jvm = "17"

[dependencies]
implementation = ["com.google.guava:guava:33.0.0-jre"]

[dependencies.test]
implementation = ["org.junit.jupiter:junit-jupiter:5.10.0"]
```

## Flags

Global flags, valid on every command:

| Flag | Description |
|------|-------------|
| --parallel | Run independent tasks concurrently |
| --max-workers <N> | Upper bound on concurrent tasks (default: CPU count) |
| --offline | Resolve dependencies from the local cache only |
| --refresh-dependencies | Ignore cached dependency metadata and re-resolve |
| --continue | Continue running tasks after a failure |
| --no-build-cache | Disable the build cache; keep up-to-date checks |
| --no-incremental | Disable up-to-date checks; run every task |
| --no-daemon | Do not reuse a long-lived build process |
| --suppress | Suppress non-error output |
| --verbose | Increase log detail |
| --format <console\|json> | Output format for reports |
| --version | Print the version and exit |
| --help | Print help for a command |

Exit codes are stable: 0 success, 1 task failure, 2 configuration
error, 3 dependency resolution failure, 4 internal error.

Environment variables (`GRADUS_USER_HOME`, `GRADUS_CACHE_DIR`,
`GRADUS_OFFLINE`, `GRADUS_MAX_WORKERS`, and others) provide defaults for
the flags above. Command-line flags take precedence.

## License

MIT.

## Contribution

Contributions are welcome. Before opening a pull request:

- Open an issue for anything larger than a bug fix, so the design can be
  agreed on first
- Keep changes focused; one concern per pull request
- Run cargo fmt, cargo clippy -- -D warnings, and cargo test before
  submitting
- Add tests for new behavior, including fixture projects where relevant

Discussion happens in GitHub issues. By participating you agree to the
Code of Conduct.