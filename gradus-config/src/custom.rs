//! Parser for Gradus's custom `.gradus` configuration syntax.
//!
//! This data-only format supports section headers and no-equals assignments.
//! Strings are single-quoted. Parsed values are normalized to TOML so the
//! existing serde model remains the source of truth.

use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for SyntaxError {}

/// Convert custom `.gradus` text into TOML for deserialization.
///
/// Supports `[section]`, `[[array.table]]`, `key value`, and `key = value`.
/// Plugin blocks accept `id 'plugin' version 'x'`; dependency blocks accept
/// one or more `configuration 'group:artifact:version'` declarations.
pub fn to_toml(input: &str) -> Result<String, SyntaxError> {
    let mut output = String::new();
    let mut saw_version = false;
    let mut current_section = String::new();
    let mut grouped_dependencies: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for (index, raw_line) in input.lines().enumerate() {
        let line_no = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') {
            validate_header(line).map_err(|message| SyntaxError { line: line_no, message })?;
            current_section = line.trim_matches('[').trim_matches(']').to_owned();
            if current_section != "dependencies" && current_section != "root_project" {
                output.push_str(line);
                output.push('\n');
            }
            continue;
        }

        if current_section == "plugins" {
            let (id, version) = parse_plugin(line).map_err(|message| SyntaxError {
                line: line_no,
                message,
            })?;
            output.push_str("id = ");
            output.push_str(&id);
            output.push('\n');
            if let Some(version) = version {
                output.push_str("version = ");
                output.push_str(&version);
                output.push('\n');
            }
            continue;
        }

        let (key, value) = split_assignment(line).ok_or_else(|| SyntaxError {
            line: line_no,
            message: "expected `key value` (the `=` is optional)".into(),
        })?;
        validate_key(key).map_err(|message| SyntaxError { line: line_no, message })?;
        validate_value(value).map_err(|message| SyntaxError { line: line_no, message })?;

        if current_section == "dependencies" {
            let parsed: toml::Value = toml::from_str(&format!("value = {value}\n"))
                .expect("value validated immediately above");
            let parsed = parsed["value"].clone();
            match parsed {
                toml::Value::String(coordinate) => grouped_dependencies
                    .entry(key.to_owned())
                    .or_default()
                    .push(coordinate),
                toml::Value::Array(values) => {
                    let bucket = grouped_dependencies.entry(key.to_owned()).or_default();
                    for value in values {
                        if let Some(coordinate) = value.as_str() {
                            bucket.push(coordinate.to_owned());
                        } else {
                            return Err(SyntaxError {
                                line: line_no,
                                message: "dependency lists must contain only strings".into(),
                            });
                        }
                    }
                }
                _ => {
                    return Err(SyntaxError {
                        line: line_no,
                        message: "dependency values must be a coordinate string or list of strings".into(),
                    });
                }
            }
        } else if current_section == "root_project" && (key == "name" || key == "include") {
            let normalized_key = if key == "name" { "root_project" } else { "include" };
            output.push_str(normalized_key);
            output.push_str(" = ");
            output.push_str(value);
            output.push('\n');
        } else {
            if current_section.is_empty() && key == "gradus_version" {
                saw_version = true;
            }
            output.push_str(key);
            output.push_str(" = ");
            output.push_str(value);
            output.push('\n');
        }
    }
    flush_dependencies(&mut output, &mut grouped_dependencies);

    if !saw_version {
        if output.is_empty() {
            output.push_str("gradus_version = 1\n");
        } else {
            output.insert_str(0, "gradus_version = 1\n\n");
        }
    }
    Ok(output)
}

fn flush_dependencies(output: &mut String, entries: &mut BTreeMap<String, Vec<String>>) {
    if entries.is_empty() {
        return;
    }
    output.push_str("[dependencies]\n");
    for (key, values) in std::mem::take(entries) {
        output.push_str(key.as_str());
        output.push_str(" = [");
        output.push_str(
            &values
                .iter()
                .map(|value| format!("{}", toml_string(value)))
                .collect::<Vec<_>>()
                .join(", "),
        );
        output.push_str("]\n");
    }
}

fn parse_plugin(line: &str) -> Result<(String, Option<String>), String> {
    let rest = line
        .strip_prefix("id")
        .filter(|rest| rest.chars().next().is_some_and(char::is_whitespace))
        .ok_or_else(|| "plugin declaration must start with `id 'plugin-id'`".to_owned())?
        .trim_start();
    let (id, remaining) = take_value(rest)?;
    let remaining = remaining.trim();
    let version = if remaining.is_empty() {
        None
    } else {
        let version_value = remaining
            .strip_prefix("version")
            .filter(|rest| rest.chars().next().is_some_and(char::is_whitespace))
            .ok_or_else(|| "expected `version 'x.y.z'` after plugin id".to_owned())?
            .trim_start();
        let (value, trailing) = take_value(version_value)?;
        if !trailing.trim().is_empty() {
            return Err("unexpected text after plugin version".into());
        }
        Some(value)
    };
    Ok((id, version))
}

fn take_value(input: &str) -> Result<(String, &str), String> {
    let first = input.chars().next().ok_or_else(|| "expected a value".to_owned())?;
    if first == '\'' || first == '"' {
        let end = input[1..]
            .find(first)
            .map(|offset| offset + 1)
            .ok_or_else(|| "unterminated quoted string".to_owned())?;
        let raw = &input[..=end];
        let parsed: toml::Value = toml::from_str(&format!("value = {raw}\n"))
            .map_err(|error| format!("invalid quoted value: {error}"))?;
        Ok((toml_string(parsed["value"].as_str().unwrap_or_default()), &input[end + 1..]))
    } else {
        let end = input.find(char::is_whitespace).unwrap_or(input.len());
        Ok((input[..end].to_owned(), &input[end..]))
    }
}

fn toml_string(value: &str) -> String {
    toml::Value::String(value.to_owned()).to_string()
}

fn validate_header(line: &str) -> Result<(), String> {
    let is_array = line.starts_with("[[");
    let (open, close) = if is_array { ("[[", "]]" ) } else { ("[", "]") };
    if !line.starts_with(open) || !line.ends_with(close) {
        return Err("malformed section header".into());
    }
    let name = &line[open.len()..line.len() - close.len()];
    if name.is_empty()
        || name.split('.').any(|part| {
            part.is_empty()
                || !part
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        })
    {
        return Err("invalid section name".into());
    }
    Ok(())
}

fn split_assignment(line: &str) -> Option<(&str, &str)> {
    let split_at = line.find(char::is_whitespace)?;
    let key = line[..split_at].trim();
    let mut rest = line[split_at..].trim_start();
    if let Some(after_equals) = rest.strip_prefix('=') {
        rest = after_equals.trim_start();
    }
    if key.is_empty() || rest.is_empty() {
        None
    } else {
        Some((key, rest))
    }
}

fn validate_key(key: &str) -> Result<(), String> {
    if key.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-') {
        Ok(())
    } else {
        Err(format!("invalid key `{key}`"))
    }
}

fn validate_value(value: &str) -> Result<(), String> {
    toml::from_str::<toml::Value>(&format!("value = {value}\n"))
        .map(|_| ())
        .map_err(|error| format!("invalid value: {error}"))
}

#[cfg(test)]
mod tests {
    use super::to_toml;

    #[test]
    fn converts_custom_assignments_and_defaults_format_version() {
        let parsed = to_toml("[project]\nname 'sample-app'\nversion '1.0.0'\n").unwrap();
        let value: toml::Value = toml::from_str(&parsed).unwrap();
        assert_eq!(value["project"]["name"].as_str(), Some("sample-app"));
        assert_eq!(value["gradus_version"].as_integer(), Some(1));
    }

    #[test]
    fn parses_plugin_and_android_statements() {
        let parsed = to_toml("[[plugins]]\nid 'com.android.application' version '8.7.3'\n[android]\nnamespace 'com.example'\ncompile_sdk 35\n").unwrap();
        let value: toml::Value = toml::from_str(&parsed).unwrap();
        assert_eq!(value["plugins"][0]["id"].as_str(), Some("com.android.application"));
        assert_eq!(value["plugins"][0]["version"].as_str(), Some("8.7.3"));
        assert_eq!(value["android"]["compile_sdk"].as_integer(), Some(35));
    }

    #[test]
    fn groups_dependency_declarations_under_one_table() {
        let parsed = to_toml("[[dependencies]]\nimplementation 'org.example:one:1'\ntest_implementation 'org.example:test:1'\n").unwrap();
        let value: toml::Value = toml::from_str(&parsed).unwrap();
        assert_eq!(value["dependencies"]["implementation"][0].as_str(), Some("org.example:one:1"));
        assert_eq!(value["dependencies"]["test_implementation"][0].as_str(), Some("org.example:test:1"));
    }
}
