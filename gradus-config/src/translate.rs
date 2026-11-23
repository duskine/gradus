//! Deterministic export of Gradus's supported configuration model to Gradle Kotlin DSL.
//!
//! This is an export layer, not a Gradle execution engine. It translates only
//! fields represented by Gradus's model and does not invoke Gradle.

use gradus_model::{
    AndroidBuildType, AndroidDefaultConfig, AndroidSection, AndroidSigningConfig, BuildFile,
    DependenciesSection,
};

use crate::{custom, SettingsFile};

#[derive(Debug, thiserror::Error)]
pub enum TranslationError {
    #[error("invalid Gradus syntax: {0}")]
    CustomSyntax(String),
    #[error("invalid Gradus configuration: {0}")]
    Configuration(String),
    #[error("cannot translate JVM target `{0}`: expected a positive integer")]
    InvalidJvmTarget(String),
    #[error("unsupported dependency resolution mode `{0}`")]
    UnsupportedResolutionMode(String),
    #[error("invalid environment variable reference `{0}`; expected `${{env.NAME}}`")]
    InvalidEnvironmentReference(String),
}

/// Parse custom `.gradus` source and emit a standard `build.gradle.kts` script.
pub fn build_gradus_to_kotlin_dsl(input: &str) -> Result<String, TranslationError> {
    let normalized = custom::to_toml(input)
        .map_err(|error| TranslationError::CustomSyntax(error.to_string()))?;
    let model: BuildFile = toml::from_str(&normalized)
        .map_err(|error| TranslationError::Configuration(error.to_string()))?;
    build_model_to_kotlin_dsl(&model)
}

/// Parse custom `.gradus` settings source and emit a standard `settings.gradle.kts` script.
pub fn settings_gradus_to_kotlin_dsl(input: &str) -> Result<String, TranslationError> {
    let normalized = custom::to_toml(input)
        .map_err(|error| TranslationError::CustomSyntax(error.to_string()))?;
    let model: SettingsFile = toml::from_str(&normalized)
        .map_err(|error| TranslationError::Configuration(error.to_string()))?;
    settings_model_to_kotlin_dsl(&model)
}

/// Emit Kotlin DSL from the internal build model.
pub fn build_model_to_kotlin_dsl(model: &BuildFile) -> Result<String, TranslationError> {
    let mut out = String::new();
    out.push_str("plugins {\n");
    if model.plugins.is_empty() {
        out.push_str("    java\n");
    } else {
        for plugin in &model.plugins {
            out.push_str(&format!("    id({})", kotlin_string(&plugin.id)));
            if let Some(version) = plugin.version.as_deref() {
                out.push_str(&format!(" version {}", kotlin_string(version)));
            }
            out.push('\n');
        }
    }
    out.push_str("}\n\n");
    if let Some(group) = model.project.group.as_deref() {
        out.push_str(&format!("group = {}\n", kotlin_string(group)));
    }
    out.push_str(&format!(
        "version = {}\n",
        kotlin_string(&model.project.version)
    ));

    if let Some(target) = model.project.jvm.as_deref() {
        let target_number = target
            .parse::<u32>()
            .ok()
            .filter(|number| *number > 0)
            .ok_or_else(|| TranslationError::InvalidJvmTarget(target.to_string()))?;
        out.push_str("\njava {\n    toolchain {\n");
        out.push_str(&format!(
            "        languageVersion.set(JavaLanguageVersion.of({target_number}))\n"
        ));
        out.push_str("    }\n}\n");
    }

    if let Some(android) = model.android.as_ref() {
        append_android(&mut out, android)?;
    }
    append_dependencies(&mut out, &model.dependencies);
    Ok(out)
}

/// Emit Kotlin DSL from the internal settings model.
pub fn settings_model_to_kotlin_dsl(model: &SettingsFile) -> Result<String, TranslationError> {
    let mut out = String::new();
    if model.repositories.google
        || model.repositories.maven_central
        || model.repositories.gradle_plugin_portal
    {
        out.push_str("pluginManagement {\n    repositories {\n");
        append_repositories(&mut out, &model.repositories, "        ");
        out.push_str("    }\n}\n\n");

        out.push_str("dependencyResolutionManagement {\n");
        match model
            .dependency_resolution
            .as_ref()
            .map(|settings| settings.mode.as_str())
        {
            Some("centralized") => {
                out.push_str("    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)\n")
            }
            Some("project") | None => {}
            Some(other) => return Err(TranslationError::UnsupportedResolutionMode(other.into())),
        }
        out.push_str("    repositories {\n");
        append_repositories(&mut out, &model.repositories, "        ");
        out.push_str("    }\n}\n\n");
    } else if let Some(mode) = model.dependency_resolution.as_ref() {
        if mode.mode != "project" && mode.mode != "centralized" {
            return Err(TranslationError::UnsupportedResolutionMode(
                mode.mode.clone(),
            ));
        }
        if mode.mode == "centralized" {
            out.push_str("dependencyResolutionManagement {\n");
            out.push_str("    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)\n");
            out.push_str("}\n\n");
        }
    }

    if let Some(name) = model.root_project.as_deref() {
        out.push_str(&format!("rootProject.name = {}\n", kotlin_string(name)));
    }
    if !model.include.is_empty() {
        let includes = model
            .include
            .iter()
            .map(|path| kotlin_string(&gradle_project_path(path)))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("include({includes})\n"));
    }
    Ok(out)
}

fn append_repositories(out: &mut String, repositories: &crate::RepositorySettings, indent: &str) {
    if repositories.google {
        out.push_str(&format!("{indent}google()\n"));
    }
    if repositories.maven_central {
        out.push_str(&format!("{indent}mavenCentral()\n"));
    }
    if repositories.gradle_plugin_portal {
        out.push_str(&format!("{indent}gradlePluginPortal()\n"));
    }
}

fn append_android(out: &mut String, android: &AndroidSection) -> Result<(), TranslationError> {
    out.push_str("\nandroid {\n");
    if let Some(namespace) = android.namespace.as_deref() {
        out.push_str(&format!("    namespace = {}\n", kotlin_string(namespace)));
    }
    if let Some(compile_sdk) = android.compile_sdk {
        out.push_str(&format!("    compileSdk = {compile_sdk}\n"));
    }
    if let Some(default_config) = android.default_config.as_ref() {
        append_default_config(out, default_config);
    }
    if let Some(signing) = android.signing.as_ref() {
        append_signing_config(out, signing)?;
    }
    if !android.build_types.is_empty() {
        out.push_str("\n    buildTypes {\n");
        for (name, build_type) in &android.build_types {
            append_build_type(out, name, build_type, android.signing.as_ref())?;
        }
        out.push_str("    }\n");
    }
    out.push_str("}\n");
    Ok(())
}

fn append_default_config(out: &mut String, config: &AndroidDefaultConfig) {
    out.push_str("\n    defaultConfig {\n");
    if let Some(application_id) = config.application_id.as_deref() {
        out.push_str(&format!(
            "        applicationId = {}\n",
            kotlin_string(application_id)
        ));
    }
    if let Some(min_sdk) = config.min_sdk {
        out.push_str(&format!("        minSdk = {min_sdk}\n"));
    }
    if let Some(target_sdk) = config.target_sdk {
        out.push_str(&format!("        targetSdk = {target_sdk}\n"));
    }
    if let Some(version_code) = config.version_code {
        out.push_str(&format!("        versionCode = {version_code}\n"));
    }
    if let Some(version_name) = config.version_name.as_deref() {
        out.push_str(&format!(
            "        versionName = {}\n",
            kotlin_string(version_name)
        ));
    }
    out.push_str("    }\n");
}

fn append_signing_config(
    out: &mut String,
    config: &AndroidSigningConfig,
) -> Result<(), TranslationError> {
    out.push_str("\n    signingConfigs {\n        create(\"release\") {\n");
    if let Some(value) = config.release_keystore.as_deref() {
        out.push_str(&format!(
            "            storeFile = file({})\n",
            env_provider(value)?
        ));
    }
    if let Some(value) = config.release_key_alias.as_deref() {
        out.push_str(&format!(
            "            keyAlias = {}\n",
            env_provider(value)?
        ));
    }
    if let Some(value) = config.release_store_password.as_deref() {
        out.push_str(&format!(
            "            storePassword = {}\n",
            env_provider(value)?
        ));
    }
    if let Some(value) = config.release_key_password.as_deref() {
        out.push_str(&format!(
            "            keyPassword = {}\n",
            env_provider(value)?
        ));
    }
    out.push_str("        }\n    }\n");
    Ok(())
}

fn append_build_type(
    out: &mut String,
    name: &str,
    config: &AndroidBuildType,
    signing: Option<&AndroidSigningConfig>,
) -> Result<(), TranslationError> {
    out.push_str(&format!("        getByName({}) {{\n", kotlin_string(name)));
    if let Some(value) = config.debuggable {
        out.push_str(&format!("            isDebuggable = {value}\n"));
    }
    if let Some(value) = config.minify_enabled {
        out.push_str(&format!("            isMinifyEnabled = {value}\n"));
    }
    if let Some(value) = config.shrink_resources {
        out.push_str(&format!("            isShrinkResources = {value}\n"));
    }
    if let Some(value) = config.application_id_suffix.as_deref() {
        out.push_str(&format!(
            "            applicationIdSuffix = {}\n",
            kotlin_string(value)
        ));
    }
    if !config.proguard_files.is_empty() {
        let files = config
            .proguard_files
            .iter()
            .map(|path| kotlin_string(path))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("            proguardFiles({files})\n"));
    }
    if name == "release" && signing.is_some() {
        out.push_str("            signingConfig = signingConfigs.getByName(\"release\")\n");
    }
    out.push_str("        }\n");
    Ok(())
}

fn env_provider(value: &str) -> Result<String, TranslationError> {
    let name = value
        .strip_prefix("${env.")
        .and_then(|rest| rest.strip_suffix('}'))
        .ok_or_else(|| TranslationError::InvalidEnvironmentReference(value.into()))?;
    if name.is_empty()
        || !name
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
    {
        return Err(TranslationError::InvalidEnvironmentReference(value.into()));
    }
    Ok(format!(
        "providers.environmentVariable({}).get()",
        kotlin_string(name)
    ))
}

fn append_dependencies(out: &mut String, dependencies: &DependenciesSection) {
    let mut entries = Vec::new();
    entries.extend(
        dependencies
            .implementation
            .iter()
            .map(|coordinate| ("implementation", coordinate)),
    );
    entries.extend(
        dependencies
            .test_implementation
            .iter()
            .map(|coordinate| ("testImplementation", coordinate)),
    );
    if let Some(test) = dependencies.test.as_ref() {
        entries.extend(
            test.implementation
                .iter()
                .map(|coordinate| ("testImplementation", coordinate)),
        );
    }
    if entries.is_empty() {
        return;
    }
    out.push_str("\ndependencies {\n");
    for (configuration, coordinate) in entries {
        out.push_str(&format!(
            "    {configuration}({})\n",
            kotlin_string(coordinate)
        ));
    }
    out.push_str("}\n");
}

fn gradle_project_path(path: &str) -> String {
    let normalized = path.trim_matches(':').replace('/', ":");
    format!(":{normalized}")
}

fn kotlin_string(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::{build_gradus_to_kotlin_dsl, settings_gradus_to_kotlin_dsl};

    #[test]
    fn translates_group_plugin_and_android_configuration() {
        let gradus = "[project]\nname 'sample-app'\ngroup 'com.example'\nversion '1.0.0'\n[[plugins]]\nid 'com.android.application' version '8.7.3'\n[android]\nnamespace 'com.example.sample'\ncompile_sdk 35\n[android.default_config]\napplication_id 'com.example.sample'\nmin_sdk 23\ntarget_sdk 35\nversion_code 1\nversion_name '1.0.0'\n[dependencies]\nimplementation ['androidx.core:core-ktx:1.15.0']\n";
        let output = build_gradus_to_kotlin_dsl(gradus).unwrap();
        assert!(output.contains("id(\"com.android.application\") version \"8.7.3\""));
        assert!(output.contains("group = \"com.example\""));
        assert!(output.contains("namespace = \"com.example.sample\""));
        assert!(output.contains("compileSdk = 35"));
        assert!(output.contains("applicationId = \"com.example.sample\""));
        assert!(output.contains("minSdk = 23"));
        assert!(output.contains("implementation(\"androidx.core:core-ktx:1.15.0\")"));
    }

    #[test]
    fn translates_settings_and_normalizes_module_paths() {
        let gradus = "root_project 'sample-app'\ninclude ['app', 'feature/login']\n";
        let output = settings_gradus_to_kotlin_dsl(gradus).unwrap();
        assert!(output.contains("rootProject.name = \"sample-app\""));
        assert!(output.contains("include(\":app\", \":feature:login\")"));
    }

    #[test]
    fn translates_build_types_and_environment_based_signing() {
        let gradus = "[project]\nname 'sample'\nversion '1.0'\n[[plugins]]\nid 'com.android.application' version '8.7.3'\n[android]\nnamespace 'com.example.sample'\ncompile_sdk 35\n[android.signing]\nrelease_keystore '${env.ANDROID_KEYSTORE_PATH}'\nrelease_key_alias '${env.ANDROID_KEY_ALIAS}'\nrelease_store_password '${env.ANDROID_KEYSTORE_PASSWORD}'\nrelease_key_password '${env.ANDROID_KEY_PASSWORD}'\n[android.build_types.release]\ndebuggable false\nminify_enabled true\nshrink_resources true\napplication_id_suffix '.release'\nproguard_files ['proguard-rules.pro']\n";
        let output = build_gradus_to_kotlin_dsl(gradus).unwrap();
        assert!(output.contains("create(\"release\")"));
        assert!(output.contains("providers.environmentVariable(\"ANDROID_KEYSTORE_PATH\").get()"));
        assert!(output.contains("isMinifyEnabled = true"));
        assert!(output.contains("isShrinkResources = true"));
        assert!(output.contains("signingConfig = signingConfigs.getByName(\"release\")"));
    }

    #[test]
    fn translates_repository_resolution_settings() {
        let gradus = "[repositories]\ngoogle true\nmaven_central true\n[dependency_resolution]\nmode 'centralized'\n";
        let output = settings_gradus_to_kotlin_dsl(gradus).unwrap();
        assert!(output.contains("google()"));
        assert!(output.contains("mavenCentral()"));
        assert!(output.contains("RepositoriesMode.FAIL_ON_PROJECT_REPOS"));
    }
}
