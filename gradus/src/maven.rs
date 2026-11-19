use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

const MAVEN_CENTRAL: &str = "https://repo.maven.apache.org/maven2";
const LOCK_FILE: &str = "gradus.lock";
const LOCK_FORMAT: u32 = 1;
const MAX_DOWNLOAD_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub(crate) struct Coordinate {
    pub group: String,
    pub artifact: String,
    pub version: String,
    #[serde(default)]
    pub classifier: Option<String>,
}

impl Coordinate {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        let parts: Vec<_> = value.split(':').collect();
        if parts.len() != 3 {
            return Err(format!(
                "invalid dependency '{value}'; expected group:artifact:version"
            ));
        }
        Self::new(parts[0], parts[1], parts[2], None)
    }

    fn new(
        group: impl Into<String>,
        artifact: impl Into<String>,
        version: impl Into<String>,
        classifier: Option<String>,
    ) -> Result<Self, String> {
        let group = group.into();
        let artifact = artifact.into();
        let version = version.into();
        let valid_group = !group.is_empty()
            && group
                .split('.')
                .all(|part| !part.is_empty() && part.chars().all(is_component_char));
        if !valid_group || !is_component(&artifact) || !is_version(&version) {
            return Err(format!(
                "invalid Maven coordinate '{}:{}:{}'",
                group, artifact, version
            ));
        }
        if classifier
            .as_deref()
            .is_some_and(|value| !is_component(value))
        {
            return Err(format!(
                "invalid Maven classifier in '{group}:{artifact}:{version}'"
            ));
        }
        Ok(Self {
            group,
            artifact,
            version,
            classifier,
        })
    }

    fn conflict_key(&self) -> String {
        format!(
            "{}:{}:{}",
            self.group,
            self.artifact,
            self.classifier.as_deref().unwrap_or("")
        )
    }

    fn validate(&self) -> Result<(), String> {
        Self::new(
            self.group.clone(),
            self.artifact.clone(),
            self.version.clone(),
            self.classifier.clone(),
        )
        .map(|_| ())
    }

    fn cache_relative_path(&self, extension: &str) -> PathBuf {
        let mut path = PathBuf::new();
        for segment in self.group.split('.') {
            path.push(segment);
        }
        path.push(&self.artifact);
        path.push(&self.version);
        path.push(self.file_name(extension));
        path
    }

    fn repository_relative_path(&self, extension: &str) -> String {
        let group_path = self.group.split('.').collect::<Vec<_>>().join("/");
        format!(
            "{group_path}/{}/{}/{}",
            self.artifact,
            self.version,
            self.file_name(extension)
        )
    }

    fn file_name(&self, extension: &str) -> String {
        let classifier = self
            .classifier
            .as_deref()
            .map(|value| format!("-{value}"))
            .unwrap_or_default();
        format!(
            "{}-{}{}.{}",
            self.artifact, self.version, classifier, extension
        )
    }
}

impl std::fmt::Display for Coordinate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}:{}:{}",
            self.group, self.artifact, self.version
        )?;
        if let Some(classifier) = &self.classifier {
            write!(formatter, ":{classifier}")?;
        }
        Ok(())
    }
}

fn is_component(value: &str) -> bool {
    !value.is_empty() && value.chars().all(is_component_char)
}

fn is_component_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
}

fn is_version(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_' | '+')
        })
        && !value.contains("..")
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedArtifact {
    pub coordinate: Coordinate,
    pub jar_path: PathBuf,
    pub depth: usize,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Lockfile {
    format_version: u32,
    roots: Vec<String>,
    artifacts: Vec<LockedArtifact>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct LockedArtifact {
    coordinate: Coordinate,
    depth: usize,
    sha256: String,
}

#[derive(Clone, Debug)]
struct ResolverSettings {
    repository: Url,
    cache_root: PathBuf,
}

impl ResolverSettings {
    fn for_project(project_root: &Path) -> Result<Self, String> {
        let repository =
            std::env::var("GRADUS_MAVEN_REPOSITORY").unwrap_or_else(|_| MAVEN_CENTRAL.to_owned());
        let cache_root = if let Some(path) = std::env::var_os("GRADUS_CACHE_DIR") {
            PathBuf::from(path).join("maven-central")
        } else if let Some(home) =
            std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
        {
            PathBuf::from(home).join(".gradus/cache/maven-central")
        } else {
            project_root.join(".gradus/cache/maven-central")
        };
        let repository = Url::parse(&format!("{}/", repository.trim_end_matches('/')))
            .map_err(|error| format!("invalid GRADUS_MAVEN_REPOSITORY URL: {error}"))?;
        if !matches!(repository.scheme(), "https" | "http") {
            return Err("GRADUS_MAVEN_REPOSITORY must be an HTTP or HTTPS URL".to_owned());
        }
        Ok(Self {
            repository,
            cache_root,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
struct Exclusion {
    group: String,
    artifact: String,
}

impl Exclusion {
    fn excludes(&self, coordinate: &Coordinate) -> bool {
        (self.group == "*" || self.group == coordinate.group)
            && (self.artifact == "*" || self.artifact == coordinate.artifact)
    }
}

#[derive(Clone, Debug)]
struct PomDependency {
    coordinate: Coordinate,
    scope: String,
    optional: bool,
    kind: String,
    exclusions: Vec<Exclusion>,
}

#[derive(Clone, Debug)]
struct RawDependency {
    group: Option<String>,
    artifact: Option<String>,
    version: Option<String>,
    scope: Option<String>,
    kind: Option<String>,
    classifier: Option<String>,
    optional: bool,
    exclusions: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
struct RawPom {
    parent: Option<Coordinate>,
    group: Option<String>,
    artifact: Option<String>,
    version: Option<String>,
    properties: HashMap<String, String>,
    managed_dependencies: Vec<RawDependency>,
    dependencies: Vec<RawDependency>,
}

#[derive(Clone, Debug)]
struct EffectivePom {
    properties: HashMap<String, String>,
    managed_dependencies: HashMap<String, PomDependency>,
    dependencies: Vec<PomDependency>,
}

pub(crate) fn resolve_dependencies(
    project_root: &Path,
    roots: &[String],
    offline: bool,
) -> Result<Vec<ResolvedArtifact>, String> {
    if roots.is_empty() {
        return Ok(Vec::new());
    }
    let settings = ResolverSettings::for_project(project_root)?;
    resolve_with_settings(project_root, roots, offline, &settings)
}

fn resolve_with_settings(
    project_root: &Path,
    roots: &[String],
    offline: bool,
    settings: &ResolverSettings,
) -> Result<Vec<ResolvedArtifact>, String> {
    let root_coordinates = roots
        .iter()
        .map(|root| Coordinate::parse(root))
        .collect::<Result<Vec<_>, _>>()?;
    if offline {
        return resolve_from_lock(project_root, roots, settings);
    }

    let agent = ureq::AgentBuilder::new()
        .timeout_read(Duration::from_secs(30))
        .timeout_write(Duration::from_secs(30))
        .build();
    let mut queue = VecDeque::new();
    for coordinate in root_coordinates {
        queue.push_back((coordinate, 0usize, Vec::<Exclusion>::new()));
    }

    let mut selected = HashMap::<String, Coordinate>::new();
    let mut expanded = HashSet::<String>::new();
    let mut effective_poms = HashMap::<Coordinate, EffectivePom>::new();
    let mut artifacts = Vec::new();

    while let Some((coordinate, depth, exclusions)) = queue.pop_front() {
        if exclusions
            .iter()
            .any(|exclusion| exclusion.excludes(&coordinate))
        {
            continue;
        }
        let key = coordinate.conflict_key();
        match selected.get(&key) {
            Some(existing) if existing != &coordinate => continue,
            None => {
                selected.insert(key, coordinate.clone());
            }
            Some(_) => {}
        }

        let state = expansion_state(&coordinate, &exclusions);
        if !expanded.insert(state) {
            continue;
        }

        let effective = if let Some(pom) = effective_poms.get(&coordinate) {
            pom.clone()
        } else {
            let mut visiting = HashSet::new();
            let pom = load_effective_pom(&coordinate, settings, &agent, &mut visiting, 0)?;
            effective_poms.insert(coordinate.clone(), pom.clone());
            pom
        };

        let jar_path = cached_path(settings, &coordinate, "jar");
        let jar_bytes = read_or_download(&jar_path, &coordinate, "jar", settings, &agent, false)?;
        let checksum = sha256(&jar_bytes);
        if !artifacts
            .iter()
            .any(|artifact: &ResolvedArtifact| artifact.coordinate == coordinate)
        {
            artifacts.push(ResolvedArtifact {
                coordinate: coordinate.clone(),
                jar_path,
                depth,
                sha256: checksum,
            });
        }

        for dependency in effective.dependencies {
            if dependency.optional || dependency.scope != "compile" {
                continue;
            }
            if dependency.kind != "jar" {
                return Err(format!(
                    "dependency '{}' uses unsupported Maven type '{}'; only JAR dependencies are supported",
                    dependency.coordinate, dependency.kind
                ));
            }
            if exclusions
                .iter()
                .any(|exclusion| exclusion.excludes(&dependency.coordinate))
            {
                continue;
            }
            let mut child_exclusions = exclusions.clone();
            child_exclusions.extend(dependency.exclusions);
            child_exclusions.sort();
            child_exclusions.dedup();
            queue.push_back((dependency.coordinate, depth + 1, child_exclusions));
        }
    }

    write_lock(project_root, roots, &artifacts)?;
    Ok(artifacts)
}

fn expansion_state(coordinate: &Coordinate, exclusions: &[Exclusion]) -> String {
    let mut sorted = exclusions.to_vec();
    sorted.sort();
    format!("{coordinate}|{sorted:?}")
}

fn load_effective_pom(
    coordinate: &Coordinate,
    settings: &ResolverSettings,
    agent: &ureq::Agent,
    visiting: &mut HashSet<Coordinate>,
    depth: usize,
) -> Result<EffectivePom, String> {
    if depth > 32 {
        return Err(format!(
            "POM inheritance/import chain is too deep at '{coordinate}'"
        ));
    }
    if !visiting.insert(coordinate.clone()) {
        return Err(format!("cyclic POM parent/import chain at '{coordinate}'"));
    }

    let pom_path = cached_path(settings, coordinate, "pom");
    let bytes = read_or_download(&pom_path, coordinate, "pom", settings, agent, false)?;
    let xml = std::str::from_utf8(&bytes)
        .map_err(|error| format!("POM for '{coordinate}' is not UTF-8: {error}"))?;
    let raw =
        parse_raw_pom(xml).map_err(|error| format!("invalid POM for '{coordinate}': {error}"))?;

    let parent = if let Some(parent_coordinate) = raw.parent.as_ref() {
        Some(load_effective_pom(
            parent_coordinate,
            settings,
            agent,
            visiting,
            depth + 1,
        )?)
    } else {
        None
    };
    let mut properties = parent
        .as_ref()
        .map(|pom| pom.properties.clone())
        .unwrap_or_default();
    properties.extend(raw.properties.clone());
    let group = raw
        .group
        .as_deref()
        .map(|value| interpolate(value, &properties))
        .transpose()?
        .unwrap_or_else(|| coordinate.group.clone());
    let artifact = raw
        .artifact
        .as_deref()
        .map(|value| interpolate(value, &properties))
        .transpose()?
        .unwrap_or_else(|| coordinate.artifact.clone());
    let version = raw
        .version
        .as_deref()
        .map(|value| interpolate(value, &properties))
        .transpose()?
        .unwrap_or_else(|| coordinate.version.clone());

    insert_project_properties(
        &mut properties,
        &group,
        &artifact,
        &version,
        raw.parent.as_ref(),
    );

    let mut managed_dependencies = parent
        .as_ref()
        .map(|pom| pom.managed_dependencies.clone())
        .unwrap_or_default();
    for raw_dependency in &raw.managed_dependencies {
        let dependency =
            resolve_pom_dependency(raw_dependency, &properties, &managed_dependencies)?;
        if dependency.scope == "import" && dependency.kind == "pom" {
            let imported =
                load_effective_pom(&dependency.coordinate, settings, agent, visiting, depth + 1)?;
            for (key, imported_dependency) in imported.managed_dependencies {
                managed_dependencies
                    .entry(key)
                    .or_insert(imported_dependency);
            }
        } else {
            managed_dependencies.insert(dependency.coordinate.conflict_key(), dependency);
        }
    }

    let mut dependencies = parent
        .as_ref()
        .map(|pom| pom.dependencies.clone())
        .unwrap_or_default();
    for raw_dependency in &raw.dependencies {
        dependencies.push(resolve_pom_dependency(
            raw_dependency,
            &properties,
            &managed_dependencies,
        )?);
    }

    visiting.remove(coordinate);

    Ok(EffectivePom {
        properties,
        managed_dependencies,
        dependencies,
    })
}

fn insert_project_properties(
    properties: &mut HashMap<String, String>,
    group: &str,
    artifact: &str,
    version: &str,
    parent: Option<&Coordinate>,
) {
    for (key, value) in [
        ("project.groupId", group),
        ("pom.groupId", group),
        ("project.artifactId", artifact),
        ("pom.artifactId", artifact),
        ("project.version", version),
        ("pom.version", version),
        ("version", version),
    ] {
        properties.insert(key.to_owned(), value.to_owned());
    }
    if let Some(parent) = parent {
        properties.insert("project.parent.groupId".to_owned(), parent.group.clone());
        properties.insert(
            "project.parent.artifactId".to_owned(),
            parent.artifact.clone(),
        );
        properties.insert("project.parent.version".to_owned(), parent.version.clone());
    }
}

fn interpolate(value: &str, properties: &HashMap<String, String>) -> Result<String, String> {
    let mut output = value.to_owned();
    for _ in 0..32 {
        let Some(start) = output.find("${") else {
            return Ok(output);
        };
        let end = output[start + 2..]
            .find('}')
            .map(|offset| start + 2 + offset)
            .ok_or_else(|| format!("unterminated Maven property in '{value}'"))?;
        let key = &output[start + 2..end];
        let replacement = properties
            .get(key)
            .ok_or_else(|| format!("unresolved Maven property '${{{key}}}' in '{value}'"))?;
        output.replace_range(start..=end, replacement);
    }
    if output.contains("${") {
        Err(format!(
            "Maven property expansion is recursive in '{value}'"
        ))
    } else {
        Ok(output)
    }
}

fn resolve_pom_dependency(
    raw: &RawDependency,
    properties: &HashMap<String, String>,
    managed: &HashMap<String, PomDependency>,
) -> Result<PomDependency, String> {
    let group = interpolate(
        raw.group
            .as_deref()
            .ok_or_else(|| "dependency is missing groupId".to_owned())?,
        properties,
    )?;
    let artifact = interpolate(
        raw.artifact
            .as_deref()
            .ok_or_else(|| "dependency is missing artifactId".to_owned())?,
        properties,
    )?;
    let classifier = raw
        .classifier
        .as_deref()
        .map(|value| interpolate(value, properties))
        .transpose()?;
    let lookup = format!("{group}:{artifact}:{}", classifier.as_deref().unwrap_or(""));
    let managed_dependency = managed.get(&lookup);
    let version = raw
        .version
        .as_deref()
        .or_else(|| managed_dependency.map(|dependency| dependency.coordinate.version.as_str()))
        .ok_or_else(|| {
            format!(
                "dependency '{group}:{artifact}' has no version and no matching dependencyManagement entry"
            )
        })?;
    let version = interpolate(version, properties)?;
    let kind = raw
        .kind
        .as_deref()
        .or_else(|| managed_dependency.map(|dependency| dependency.kind.as_str()))
        .unwrap_or("jar")
        .to_owned();
    let scope = raw
        .scope
        .as_deref()
        .or_else(|| managed_dependency.map(|dependency| dependency.scope.as_str()))
        .unwrap_or("compile")
        .to_owned();
    let optional = raw.optional || managed_dependency.is_some_and(|dependency| dependency.optional);
    let mut exclusions = managed_dependency
        .map(|dependency| dependency.exclusions.clone())
        .unwrap_or_default();
    for (excluded_group, excluded_artifact) in &raw.exclusions {
        exclusions.push(Exclusion {
            group: interpolate(excluded_group, properties)?,
            artifact: interpolate(excluded_artifact, properties)?,
        });
    }
    let coordinate = Coordinate::new(group, artifact, version, classifier)?;
    Ok(PomDependency {
        coordinate,
        scope,
        optional,
        kind,
        exclusions,
    })
}

fn parse_raw_pom(xml: &str) -> Result<RawPom, String> {
    let document = roxmltree::Document::parse(xml).map_err(|error| error.to_string())?;
    let project = document.root_element();
    let parent_node = child(project, "parent");
    let parent = parent_node
        .map(|node| {
            let group = text_child(node, "groupId")
                .ok_or_else(|| "parent POM is missing groupId".to_owned())?;
            let artifact = text_child(node, "artifactId")
                .ok_or_else(|| "parent POM is missing artifactId".to_owned())?;
            let version = text_child(node, "version")
                .ok_or_else(|| "parent POM is missing version".to_owned())?;
            if [&group, &artifact, &version]
                .iter()
                .any(|value| value.contains("${"))
            {
                return Err("property-based parent coordinates are not supported".to_owned());
            }
            Coordinate::new(group, artifact, version, None)
        })
        .transpose()?;

    let mut properties = HashMap::new();
    if let Some(node) = child(project, "properties") {
        for property in node.children().filter(|node| node.is_element()) {
            if let Some(value) = property.text() {
                properties.insert(
                    property.tag_name().name().to_owned(),
                    value.trim().to_owned(),
                );
            }
        }
    }

    let managed_dependencies = child(project, "dependencyManagement")
        .and_then(|node| child(node, "dependencies"))
        .map(parse_dependency_list)
        .unwrap_or_default();
    let dependencies = child(project, "dependencies")
        .map(parse_dependency_list)
        .unwrap_or_default();

    Ok(RawPom {
        parent,
        group: text_child(project, "groupId"),
        artifact: text_child(project, "artifactId"),
        version: text_child(project, "version"),
        properties,
        managed_dependencies,
        dependencies,
    })
}

fn parse_dependency_list(node: roxmltree::Node<'_, '_>) -> Vec<RawDependency> {
    node.children()
        .filter(|child| child.is_element() && child.tag_name().name() == "dependency")
        .map(|dependency| {
            let exclusions = child(dependency, "exclusions")
                .into_iter()
                .flat_map(|node| node.children())
                .filter(|node| node.is_element() && node.tag_name().name() == "exclusion")
                .filter_map(|node| {
                    Some((
                        text_child(node, "groupId")?,
                        text_child(node, "artifactId")?,
                    ))
                })
                .collect();
            RawDependency {
                group: text_child(dependency, "groupId"),
                artifact: text_child(dependency, "artifactId"),
                version: text_child(dependency, "version"),
                scope: text_child(dependency, "scope"),
                kind: text_child(dependency, "type"),
                classifier: text_child(dependency, "classifier"),
                optional: text_child(dependency, "optional")
                    .is_some_and(|value| value.eq_ignore_ascii_case("true")),
                exclusions,
            }
        })
        .collect()
}

fn child<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    name: &str,
) -> Option<roxmltree::Node<'a, 'input>> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == name)
}

fn text_child(node: roxmltree::Node<'_, '_>, name: &str) -> Option<String> {
    child(node, name)
        .and_then(|child| child.text())
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

fn resolve_from_lock(
    project_root: &Path,
    roots: &[String],
    settings: &ResolverSettings,
) -> Result<Vec<ResolvedArtifact>, String> {
    let lock_path = project_root.join(LOCK_FILE);
    reject_symlink(&lock_path, "lockfile")?;
    let lock_text = fs::read_to_string(&lock_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!("offline mode requires '{LOCK_FILE}'; run a normal build once while online")
        } else {
            format!("failed to read '{}': {error}", lock_path.display())
        }
    })?;
    let lock: Lockfile = toml::from_str(&lock_text)
        .map_err(|error| format!("invalid '{}': {error}", lock_path.display()))?;
    if lock.format_version != LOCK_FORMAT {
        return Err(format!(
            "unsupported '{}' format version {}; expected {LOCK_FORMAT}",
            lock_path.display(),
            lock.format_version
        ));
    }
    if lock.roots != roots {
        return Err(format!(
            "'{LOCK_FILE}' does not match the declared dependencies; run a normal build while online to update it"
        ));
    }

    let mut artifacts = Vec::new();
    for locked in lock.artifacts {
        locked
            .coordinate
            .validate()
            .map_err(|error| format!("invalid dependency coordinate in '{LOCK_FILE}': {error}"))?;
        if locked.sha256.len() != 64
            || !locked
                .sha256
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        {
            return Err(format!(
                "invalid SHA-256 checksum for '{}' in '{LOCK_FILE}'",
                locked.coordinate
            ));
        }
        let path = cached_path(settings, &locked.coordinate, "jar");
        reject_symlink(&path, "cached JAR")?;
        let bytes = fs::read(&path).map_err(|error| {
            format!(
                "offline dependency '{}' is not cached at '{}': {error}",
                locked.coordinate,
                path.display()
            )
        })?;
        let actual = sha256(&bytes);
        if actual != locked.sha256 {
            return Err(format!(
                "cached JAR checksum mismatch for '{}': lockfile {}, actual {}",
                locked.coordinate, locked.sha256, actual
            ));
        }
        artifacts.push(ResolvedArtifact {
            coordinate: locked.coordinate,
            jar_path: path,
            depth: locked.depth,
            sha256: actual,
        });
    }
    Ok(artifacts)
}

fn write_lock(
    project_root: &Path,
    roots: &[String],
    artifacts: &[ResolvedArtifact],
) -> Result<(), String> {
    let lock_path = project_root.join(LOCK_FILE);
    reject_symlink(&lock_path, "lockfile")?;
    let lock = Lockfile {
        format_version: LOCK_FORMAT,
        roots: roots.to_vec(),
        artifacts: artifacts
            .iter()
            .map(|artifact| LockedArtifact {
                coordinate: artifact.coordinate.clone(),
                depth: artifact.depth,
                sha256: artifact.sha256.clone(),
            })
            .collect(),
    };
    let text = toml::to_string_pretty(&lock)
        .map_err(|error| format!("failed to encode '{LOCK_FILE}': {error}"))?;
    atomic_write(&lock_path, text.as_bytes())
        .map_err(|error| format!("failed to write '{}': {error}", lock_path.display()))
}

fn cached_path(settings: &ResolverSettings, coordinate: &Coordinate, extension: &str) -> PathBuf {
    settings
        .cache_root
        .join(coordinate.cache_relative_path(extension))
}

fn read_or_download(
    path: &Path,
    coordinate: &Coordinate,
    extension: &str,
    settings: &ResolverSettings,
    agent: &ureq::Agent,
    offline: bool,
) -> Result<Vec<u8>, String> {
    reject_symlink(path, "cached Maven file")?;
    match fs::read(path) {
        Ok(bytes) => return Ok(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && offline => {
            return Err(format!("offline mode: '{}' is not cached", path.display()));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("failed to read '{}': {error}", path.display())),
    }

    let relative = coordinate.repository_relative_path(extension);
    let url = settings
        .repository
        .join(&relative)
        .map_err(|error| format!("failed to construct Maven URL: {error}"))?
        .to_string();
    let response = agent
        .get(&url)
        .set("User-Agent", "Gradus/0.1.3")
        .call()
        .map_err(|error| match error {
            ureq::Error::Status(code, _) => {
                format!("Maven repository returned HTTP {code} for '{url}'")
            }
            ureq::Error::Transport(error) => format!("failed to download '{url}': {error}"),
        })?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_DOWNLOAD_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read response from '{url}': {error}"))?;
    if bytes.len() as u64 > MAX_DOWNLOAD_BYTES {
        return Err(format!("download from '{url}' exceeded the size limit"));
    }
    atomic_write(path, &bytes)
        .map_err(|error| format!("failed to cache '{}': {error}", path.display()))?;
    Ok(bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "path has no parent directory",
        )
    })?;
    fs::create_dir_all(parent)?;
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("gradus-temp");
    let temporary = parent.join(format!(".{filename}.tmp-{}", std::process::id()));
    fs::write(&temporary, bytes)?;
    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(error)
        }
    }
}

fn reject_symlink(path: &Path, description: &str) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "refusing to use '{}' because it is a symbolic link",
            path.display()
        )),
        Ok(metadata) if !metadata.is_file() && path.file_name().is_some() => Err(format!(
            "refusing to use '{}': expected a regular file",
            path.display()
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to inspect {description} '{}': {error}",
            path.display()
        )),
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn parses_and_validates_coordinates() {
        let coordinate = Coordinate::parse("org.example:demo:1.2.3").unwrap();
        assert_eq!(coordinate.to_string(), "org.example:demo:1.2.3");
        assert!(Coordinate::parse("org.example:../demo:1.0").is_err());
        assert!(Coordinate::parse("org.example:demo:[1.0,2.0)").is_err());
    }

    #[test]
    fn rejects_path_traversal_coordinates_in_lockfiles() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project");
        fs::create_dir_all(&project).unwrap();
        let root = "org.example:root:1.0".to_owned();
        let lock = Lockfile {
            format_version: LOCK_FORMAT,
            roots: vec![root.clone()],
            artifacts: vec![LockedArtifact {
                coordinate: Coordinate {
                    group: "org.example".to_owned(),
                    artifact: "../../outside".to_owned(),
                    version: "1.0".to_owned(),
                    classifier: None,
                },
                depth: 0,
                sha256: "0".repeat(64),
            }],
        };
        fs::write(project.join(LOCK_FILE), toml::to_string(&lock).unwrap()).unwrap();
        let settings = ResolverSettings {
            repository: Url::parse("https://repo.example/").unwrap(),
            cache_root: temp.path().join("cache"),
        };

        let error = resolve_with_settings(&project, &[root], true, &settings).unwrap_err();
        assert!(error.contains("invalid dependency coordinate in 'gradus.lock'"));
    }

    #[test]
    fn parses_a_pom_with_namespaces_and_properties() {
        let xml = r#"<project xmlns="http://maven.apache.org/POM/4.0.0">
            <groupId>org.example</groupId><artifactId>sample</artifactId><version>1.0</version>
            <properties><lib.version>2.4.1</lib.version></properties>
            <dependencies><dependency><groupId>org.example</groupId><artifactId>child</artifactId>
            <version>${lib.version}</version></dependency></dependencies>
        </project>"#;
        let raw = parse_raw_pom(xml).unwrap();
        assert_eq!(raw.dependencies.len(), 1);
        let mut properties = raw.properties.clone();
        insert_project_properties(&mut properties, "org.example", "sample", "1.0", None);
        let dependency =
            resolve_pom_dependency(&raw.dependencies[0], &properties, &HashMap::new()).unwrap();
        assert_eq!(dependency.coordinate.to_string(), "org.example:child:2.4.1");
    }

    #[test]
    fn resolves_transitive_artifacts_and_reuses_locked_cache_offline() {
        let temp = tempfile::tempdir().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let root_coordinate = Coordinate::parse("org.example:root:1.0").unwrap();
        let child_coordinate = Coordinate::parse("org.example:child:2.0").unwrap();
        let root_pom = br#"<project><groupId>org.example</groupId><artifactId>root</artifactId><version>1.0</version><dependencies><dependency><groupId>org.example</groupId><artifactId>child</artifactId><version>2.0</version></dependency></dependencies></project>"#.to_vec();
        let child_pom = br#"<project><groupId>org.example</groupId><artifactId>child</artifactId><version>2.0</version></project>"#.to_vec();
        let files = HashMap::from([
            (root_coordinate.repository_relative_path("pom"), root_pom),
            (
                root_coordinate.repository_relative_path("jar"),
                b"root-jar".to_vec(),
            ),
            (child_coordinate.repository_relative_path("pom"), child_pom),
            (
                child_coordinate.repository_relative_path("jar"),
                b"child-jar".to_vec(),
            ),
        ]);
        let server = thread::spawn(move || {
            for _ in 0..4 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request_line = String::new();
                {
                    let mut reader = BufReader::new(&mut stream);
                    reader.read_line(&mut request_line).unwrap();
                    loop {
                        let mut line = String::new();
                        reader.read_line(&mut line).unwrap();
                        if line == "\r\n" || line.is_empty() {
                            break;
                        }
                    }
                }
                let path = request_line.split_whitespace().nth(1).unwrap_or("/");
                let body = files.get(path.trim_start_matches('/')).cloned();
                let (status, body) = match body {
                    Some(body) => ("200 OK", body),
                    None => ("404 Not Found", Vec::new()),
                };
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
            }
        });

        let settings = ResolverSettings {
            repository: Url::parse(&format!("http://{address}/")).unwrap(),
            cache_root: temp.path().join("cache"),
        };
        let project = temp.path().join("project");
        fs::create_dir_all(&project).unwrap();
        let roots = vec!["org.example:root:1.0".to_owned()];
        let resolved = resolve_with_settings(&project, &roots, false, &settings).unwrap();
        server.join().unwrap();
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].depth, 0);
        assert_eq!(resolved[1].depth, 1);
        assert!(project.join(LOCK_FILE).is_file());

        let offline = resolve_with_settings(&project, &roots, true, &settings).unwrap();
        assert_eq!(offline.len(), 2);
        assert_eq!(offline[0].sha256, resolved[0].sha256);
    }
}
