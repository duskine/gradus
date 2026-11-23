# Gradus 0.1.4 configuration guide (draft)

> **Release status:** This guide describes the proposed configuration syntax and Kotlin DSL export scope for Gradus 0.1.4. It is not evidence that 0.1.4 has been released or that every example can already be built by the Gradus CLI. The published crate baseline is 0.1.3. In particular, Android settings can be described and exported, but Gradus does not yet compile Kotlin or package APKs itself.

## 1. Configuration files and formats

Gradus supports two configuration file pairs:

| Custom Gradus syntax | TOML syntax |
| --- | --- |
| `settings.gradus` | `settings.gradus.toml` |
| `build.gradus` | `build.gradus.toml` |

The custom `.gradus` syntax uses section headers, single-quoted strings, and assignments without `=`. The `.gradus.toml` files use standard TOML, with `=` assignments and double-quoted basic strings. The extension selects the parser; it does not select the source compiler. Either configuration format can describe Java, Kotlin, or mixed projects. Kotlin compilation is not currently implemented by the Gradus CLI.

Use one settings file and one build file in each project. Do not create both alternatives in the same directory. Gradus treats that as ambiguous instead of silently choosing or merging one.

### Custom syntax conventions

- `[section]` opens a section; `[[section]]` denotes a repeatable table such as a plugin or dependency block.
- A regular field is written as `key 'value'`, `key 17`, `key true`, or `key ['one', 'two']`.
- The custom parser also accepts `key = value`, but examples omit `=` for consistency with the chosen style.
- Strings use single quotes. Values and arrays use TOML-compatible scalar syntax.
- Comments start with `#` when the first non-whitespace character on a line is `#`.
- A plugin declaration has the compact form `id 'plugin.id' version '1.2.3'` or `id 'plugin.id'`.
- Dependency entries use `configuration 'group:artifact:version'` inside a `[[dependencies]]` block.
- The configuration schema defaults to version 1. Users do not need to write a `gradus_version` field in either format.

### TOML conventions

TOML files must be valid TOML. Plugin declarations use array-of-table entries such as `[[plugins]]`. Dependencies are lists under `[dependencies]`. The schema defaults to version 1 when `gradus_version` is omitted.

## 2. Multi-project settings

The custom settings syntax supports a `[root_project]` section with `name` and `include` fields:

### `settings.gradus`

```text
[root_project]
name 'sample-app'
include ['app', 'core', 'feature/login']

[repositories]
google true
maven_central true
gradle_plugin_portal true

[dependency_resolution]
mode 'centralized'
```

The corresponding TOML settings file uses a top-level root name and include list:

### `settings.gradus.toml`

```toml
root_project = "sample-app"
include = ["app", "core", "feature/login"]

[repositories]
google = true
maven_central = true
gradle_plugin_portal = true

[dependency_resolution]
mode = "centralized"
```

Module paths are project-relative. During Kotlin DSL export, `feature/login` becomes the Gradle project path `:feature:login`.

The repository switches shown here map to the Gradle `google()`, `mavenCentral()`, and `gradlePluginPortal()` repositories. `mode = "centralized"` exports `RepositoriesMode.FAIL_ON_PROJECT_REPOS`, requiring project repositories to be declared centrally in settings. `mode = "project"` leaves repository placement to project build files. The configuration/export layer does not itself guarantee that Gradus's current Maven resolver can fetch from every listed repository.

## 3. Project metadata, plugins, and dependencies

### `build.gradus`

```text
[project]
name 'sample-app'
group 'com.example'
version '1.0.0'
jvm '17'

[[plugins]]
id 'com.android.application' version '8.7.3'

[[plugins]]
id 'org.jetbrains.kotlin.android' version '2.1.0'

[[dependencies]]
implementation 'androidx.core:core-ktx:1.15.0'
implementation 'androidx.appcompat:appcompat:1.7.0'
test_implementation 'junit:junit:4.13.2'
```

### `build.gradus.toml`

```toml
[project]
name = "sample-app"
group = "com.example"
version = "1.0.0"
jvm = "17"

[[plugins]]
id = "com.android.application"
version = "8.7.3"

[[plugins]]
id = "org.jetbrains.kotlin.android"
version = "2.1.0"

[dependencies]
implementation = [
  "androidx.core:core-ktx:1.15.0",
  "androidx.appcompat:appcompat:1.7.0",
]
test_implementation = ["junit:junit:4.13.2"]
```

`group` and `version` become Gradle project properties. `jvm` represents the Java target/toolchain field for the Java build path. Plugins are declarations for translation; Gradus does not execute arbitrary plugin implementations.

The initial dependency syntax recognizes `implementation` and `test_implementation`. The Gradus CLI's current Java resolver is narrower than Gradle's dependency engine and should not be assumed to resolve AndroidX artifacts from Google's Maven repository. Dependency scopes are also not proof of test task support: Gradus does not yet execute tests.

## 4. Android application configuration

Android configuration is represented under `[android]`. Here is a fuller custom-syntax example:

```text
[project]
name 'sample-app'
group 'com.example'
version '1.0.0'

[[plugins]]
id 'com.android.application' version '8.7.3'

[[plugins]]
id 'org.jetbrains.kotlin.android' version '2.1.0'

[android]
namespace 'com.example.sample'
compile_sdk 35

[android.default_config]
application_id 'com.example.sample'
min_sdk 23
target_sdk 35
version_code 1
version_name '1.0.0'

[android.build_types.debug]
debuggable true
minify_enabled false
application_id_suffix '.debug'

[android.build_types.release]
debuggable false
minify_enabled true
shrink_resources true
proguard_files ['proguard-android-optimize.txt', 'proguard-rules.pro']
```

Equivalent TOML build-type tables:

```toml
[android]
namespace = "com.example.sample"
compile_sdk = 35

[android.default_config]
application_id = "com.example.sample"
min_sdk = 23
target_sdk = 35
version_code = 1
version_name = "1.0.0"

[android.build_types.debug]
debuggable = true
minify_enabled = false
application_id_suffix = ".debug"

[android.build_types.release]
debuggable = false
minify_enabled = true
shrink_resources = true
proguard_files = ["proguard-android-optimize.txt", "proguard-rules.pro"]
```

The exporter maps these names to Android Gradle Plugin Kotlin DSL properties, including `compileSdk`, `applicationId`, `minSdk`, `targetSdk`, `versionCode`, `versionName`, `isMinifyEnabled`, and `isShrinkResources`. Build type names are emitted with `getByName`, so this shape targets the standard Android `debug` and `release` build types unless a compatible plugin provides another type.

## 5. Signing configuration and environment variables

Never commit keystores, aliases, or passwords to a repository. Store signing data in environment variables and refer to variable names in the Gradus configuration:

```text
[android.signing]
release_keystore '${env.ANDROID_KEYSTORE_PATH}'
release_key_alias '${env.ANDROID_KEY_ALIAS}'
release_store_password '${env.ANDROID_KEYSTORE_PASSWORD}'
release_key_password '${env.ANDROID_KEY_PASSWORD}'
```

The `.gradus.toml` equivalent is:

```toml
[android.signing]
release_keystore = "${env.ANDROID_KEYSTORE_PATH}"
release_key_alias = "${env.ANDROID_KEY_ALIAS}"
release_store_password = "${env.ANDROID_KEYSTORE_PASSWORD}"
release_key_password = "${env.ANDROID_KEY_PASSWORD}"
```

The Kotlin DSL exporter converts those references to Gradle provider access, for example:

```kotlin
storeFile = file(providers.environmentVariable("ANDROID_KEYSTORE_PATH").get())
storePassword = providers.environmentVariable("ANDROID_KEYSTORE_PASSWORD").get()
```

Environment variables must be available when Gradle configures the exported project; `.get()` fails if a required value is absent. The exporter attaches the generated `release` signing config to the `release` build type when both are present. This is Kotlin DSL export only; Gradus does not store credentials or sign APKs itself.

## 6. Kotlin DSL export

The library translation functions convert supported Gradus configuration text into Kotlin DSL source:

```rust
let build_kts = gradus_config::build_gradus_to_kotlin_dsl(build_gradus_text)?;
let settings_kts = gradus_config::settings_gradus_to_kotlin_dsl(settings_gradus_text)?;
```

The expected files are `build.gradle.kts` and `settings.gradle.kts`. The generated scripts can contain plugin declarations, project `group` and `version`, supported Java toolchain settings, Android namespace/default config/build types/signing, dependency declarations, included projects, and supported repository settings.

This translator is not a general Gradle transpiler. It does not translate arbitrary Gradus statements, execute build scripts, generate custom Gradle plugins, or guarantee that every generated script works with every Android Gradle Plugin version. Review generated files and use a matching Android Gradle Plugin/Kotlin plugin/JDK combination in the Gradle project.

## 7. Current status and limitations

The Gradus CLI's currently implemented build path is Java compilation and JAR packaging via local `javac` and `jar`, plus bounded Maven dependency resolution. The 0.1.4 configuration and export work should not be confused with full build execution support.

Not implemented by the Gradus CLI in this milestone:

- Native Rust replacements for `javac` or `kotlinc`.
- Kotlin compilation or Java/Kotlin mixed-source compilation.
- Android resource processing, manifest merging, DEX conversion, APK/AAB packaging, APK alignment, or APK signing.
- Full Gradle Groovy/Kotlin DSL compatibility or arbitrary Gradle plugin execution.
- Test execution, incremental compilation, or parallel task execution.

The settings repository flags and Android plugin fields are for describing projects and exporting Gradle Kotlin DSL. They do not expand the current native Maven resolver or Java build engine by themselves.

## 8. Suggested validation workflow

When the 0.1.4 parser and translator are wired into the executable, the intended workflow is:

```sh
gradus validate
gradus tasks
gradus build --dry-run
```

For a Gradle export, generate or save the translated scripts, inspect the diff, then test them with the project's Gradle wrapper and Android SDK. Until a Gradus CLI export command is documented, invoke the library API or use the available project tooling; do not assume `gradus export` exists.

## 9. Gradle migration preflight

The proposed CLI command `gradus port gradle <version>` performs a read-only scan of root `build.gradle`, `build.gradle.kts`, `settings.gradle`, and `settings.gradle.kts` files. It does not execute Gradle, convert the project, create Gradus files, or overwrite existing files.

```sh
gradus port gradle 8.7.3
```

Findings include a severity, stable diagnostic code, path and line number, explanation, and suggested action. Unsupported or dynamic plugin declarations and custom Gradle task/build logic are treated as blockers. Plugin declarations that are syntactically recognizable may still produce warnings: recording a plugin ID does not make its Gradle plugin behavior available in Gradus. Some Android settings or dependency forms that cannot be represented in the current Gradus schema are also reported as warnings.

The scan is intentionally conservative and not a full Groovy/Kotlin parser. A clean report means only that the scanner found no known unsupported patterns; it is not proof that a project can be migrated without behavior changes. Conversion and explicit file-writing modes are future work.

## 10. Current CLI commands

The existing CLI includes:

```sh
gradus init
gradus validate
gradus tasks
gradus fix
gradus port gradle 8.7.3
gradus dependencies [--offline]
gradus build [--dry-run] [--offline]
gradus clean
```

See the repository README for the current Java build path and dependency resolver limitations.
