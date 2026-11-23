# Gradus

[![Crates.io](https://img.shields.io/crates/v/gradus.svg)](https://crates.io/crates/gradus)
[![Downloads](https://img.shields.io/crates/d/gradus.svg)](https://crates.io/crates/gradus)
[![Documentation](https://docs.rs/gradus/badge.svg)](https://docs.rs/gradus)
[![CI](https://github.com/duskine/gradus/actions/workflows/ci.yml/badge.svg)](https://github.com/duskine/gradus/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](#license)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue.svg)](#)

A native JVM build tool written in Rust.

> **0.1.4 documentation draft:** The syntax and examples below describe the planned Gradus configuration and Kotlin DSL export direction. They do not mean that Gradus 0.1.4 has been released or that all illustrated Android features can already be built by the Gradus CLI. The latest published crate is 0.1.3.

## What Gradus is

Gradus is an early-stage native build tool for JVM projects. The current working Java path uses a local JDK to compile Java source and create a JAR. Gradus starts as a native Rust program and does not need a JVM just to launch the build tool.

Gradus is **not currently a drop-in Gradle replacement**. It does not execute arbitrary Gradle scripts or plugins, and its current CLI does not compile Kotlin or build Android APKs. The planned 0.1.4 configuration parser and Kotlin DSL exporter are a bridge for describing and exporting supported settings, not an Android compiler or packaging pipeline.

## Configuration formats

Gradus has two configuration syntaxes that map to a common project model:

- `settings.gradus` and `build.gradus` use the Gradus custom syntax: section headers, single-quoted strings, and assignments without `=`.
- `settings.gradus.toml` and `build.gradus.toml` use standard TOML: `=` assignments and double-quoted strings.

The file extension chooses the configuration parser. Source language selection is separate: configurations can describe Java, Kotlin, or mixed-language projects, though Kotlin compilation is not yet implemented in the Gradus CLI.

A project uses one settings file and one build file from the same format. If both alternatives are present in the same directory, Gradus reports an ambiguity rather than merging them.
The configuration schema defaults to version 1; users do not need to write a `gradus_version` field.

### Custom syntax: `build.gradus`

```text
[project]
name 'sample-app'
group 'com.example'
version '1.0.0'

[[plugins]]
id 'com.android.application' version '8.7.3'

[android]
namespace 'com.example.sample'
compile_sdk 35

[android.default_config]
application_id 'com.example.sample'
min_sdk 23
target_sdk 35
version_code 1
version_name '1.0.0'

[[dependencies]]
implementation 'androidx.core:core-ktx:1.15.0'
test_implementation 'junit:junit:4.13.2'
```

### TOML syntax: `build.gradus.toml`

```toml
[project]
name = "sample-app"
group = "com.example"
version = "1.0.0"

[[plugins]]
id = "com.android.application"
version = "8.7.3"

[android]
namespace = "com.example.sample"
compile_sdk = 35

[android.default_config]
application_id = "com.example.sample"
min_sdk = 23
target_sdk = 35
version_code = 1
version_name = "1.0.0"

[dependencies]
implementation = ["androidx.core:core-ktx:1.15.0"]
test_implementation = ["junit:junit:4.13.2"]
```

See the [Gradus 0.1.4 configuration guide](docs/gradus-0.1.4-guide.md) for the complete field reference, settings examples, Android build types and signing, multi-module configuration, Kotlin DSL export, and current limitations.

## Current capabilities and planned scope

The current Java build path supports Java source under `src/main/java`, uses `javac` and `jar` from the local JDK, packages a JAR under `build/libs/`, and resolves fixed-version Maven dependencies for the compile classpath. The resolver supports offline builds from `gradus.lock` and checksum verification, within its documented limits.

The 0.1.4 workstream is focused on the configuration parser/model, validation, and exporting supported Gradus settings to `build.gradle.kts` and `settings.gradle.kts`. Android configuration such as plugins, build types, repositories, and signing can be represented and translated. Gradus itself does **not yet** perform Android resource processing, DEX conversion, APK packaging, APK signing, or Kotlin compilation.

Incremental compilation, parallel execution, test execution, application launching, native Java/Kotlin compiler implementations, and broad Gradle plugin compatibility remain future work.

## CLI usage

For an existing Gradus installation, the current CLI commands include:

```sh
gradus validate
gradus tasks
gradus port gradle 8.7.3
gradus fix
gradus dependencies
gradus dependencies --offline
gradus build
gradus build --dry-run
gradus build --offline
gradus clean
```

The published crate is currently 0.1.3. After the 0.1.4 release is published, install it with:

```sh
cargo install gradus --version 0.1.4
```

`gradus fix` checks the Java project setup and can create `src/main/java`; it does not install a JDK or modify shell settings. The build currently requires `javac` and `jar` on `PATH`.

`gradus port gradle <version>` performs a read-only preflight scan of Gradle build and settings scripts. It reports known unsupported plugins and syntax with file/line locations and suggestions. It does not yet convert files or run Gradle, and a clean scan is not a guarantee of compatibility.

The dependency resolver supports fixed coordinates such as `org.apache.commons:commons-lang3:3.14.0`. Version ranges, snapshots, non-JAR dependency types, and several advanced Maven cases are not supported. Runtime, provided, test, and optional transitive scopes are not currently included in the compile classpath.

## Development

Before submitting a change, run:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

See [CONTRIBUTING guidance](#contribution) and the [full 0.1.4 guide](docs/gradus-0.1.4-guide.md).

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for the full text.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this project by you shall be licensed under the MIT License, without any additional terms or conditions.

## Contribution

Contributions are welcome. For anything larger than a bug fix, open an issue first so the design can be agreed on. Keep pull requests focused and add tests for new behavior, including fixture projects when relevant. Discussion happens in GitHub issues. By participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).
