// Tanka-compatible native functions
// These are wrappers around the existing stdlib functions to provide
// Tanka-compatible API accessible via std.native()

#[cfg(feature = "exp-regex")]
use jrsonnet_evaluator::IStr;
use jrsonnet_evaluator::{
	error::{ErrorKind::*, Result},
	ObjValue, Val,
};
use jrsonnet_macros::builtin;
use serde::Deserialize;
use serde_json;
use serde_yaml_with_quirks as serde_yaml;
use sha2::{Digest, Sha256};
use std::io::{BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::thread;

/// Convert a string to snake_case (lowercase with underscores)
fn to_snake_case(s: &str) -> String {
	let mut result = String::new();
	let mut chars = s.chars().peekable();

	while let Some(ch) = chars.next() {
		if ch.is_uppercase() {
			// Add underscore before uppercase letters (except at start)
			if !result.is_empty() {
				result.push('_');
			}
			result.push(ch.to_lowercase().next().unwrap());
		} else if ch == '-' {
			// Replace hyphens with underscores
			result.push('_');
		} else {
			result.push(ch);
		}
	}

	result
}

#[cfg(feature = "exp-regex")]
use crate::regex::RegexCacheInner;
#[cfg(feature = "exp-regex")]
use std::rc::Rc;

/// Tanka-compatible parseJson
/// Parses a JSON string into a value
#[builtin]
pub fn builtin_tanka_parse_json(json: String) -> Result<Val> {
	serde_json::from_str(&json)
		.map_err(|e| RuntimeError(format!("failed to parse json: {e}").into()).into())
}

/// Tanka-compatible parseYaml
/// Parses a YAML string (potentially multiple documents) into an array of values
#[builtin]
pub fn builtin_tanka_parse_yaml(yaml: String) -> Result<Val> {
	let mut ret = Vec::new();
	let deserializer = serde_yaml::Deserializer::from_str(&yaml);

	for document in deserializer {
		let val: Val = Val::deserialize(document)
			.map_err(|e| RuntimeError(format!("failed to parse yaml: {e}").into()))?;
		ret.push(val);
	}

	Ok(Val::Arr(ret.into()))
}

/// Tanka-compatible manifestJsonFromJson
/// Reserializes JSON with custom indentation
#[builtin]
pub fn builtin_tanka_manifest_json_from_json(json: String, indent: usize) -> Result<String> {
	let parsed: serde_json::Value = serde_json::from_str(&json)
		.map_err(|e| RuntimeError(format!("failed to parse json: {e}").into()))?;

	let indentation = " ".repeat(indent);
	let formatter = serde_json::ser::PrettyFormatter::with_indent(indentation.as_bytes());
	let mut buf = Vec::new();
	let mut serializer = serde_json::Serializer::with_formatter(&mut buf, formatter);

	serde::Serialize::serialize(&parsed, &mut serializer)
		.map_err(|e| RuntimeError(format!("failed to serialize json: {e}").into()))?;

	buf.push(b'\n');
	String::from_utf8(buf)
		.map_err(|e| RuntimeError(format!("failed to convert to utf8: {e}").into()).into())
}

/// Tanka-compatible manifestYamlFromJson
/// Converts JSON string to YAML
#[builtin]
pub fn builtin_tanka_manifest_yaml_from_json(json: String) -> Result<String> {
	let parsed: serde_json::Value = serde_json::from_str(&json)
		.map_err(|e| RuntimeError(format!("failed to parse json: {e}").into()))?;

	serde_yaml::to_string(&parsed)
		.map_err(|e| RuntimeError(format!("failed to serialize yaml: {e}").into()).into())
}

/// Tanka-compatible sha256
/// Computes SHA256 hash of a string
#[builtin]
pub fn builtin_tanka_sha256(str: String) -> String {
	let mut hasher = Sha256::new();
	hasher.update(str.as_bytes());
	format!("{:x}", hasher.finalize())
}

/// Tanka-compatible escapeStringRegex
/// Escapes regex special characters
#[builtin]
pub fn builtin_escape_string_regex(pattern: String) -> String {
	#[cfg(feature = "exp-regex")]
	{
		regex::escape(&pattern)
	}
	#[cfg(not(feature = "exp-regex"))]
	{
		panic!("exp-regex feature is not enabled")
	}
}

/// Tanka-compatible regexMatch
/// Returns true if the string matches the regex pattern
#[cfg(feature = "exp-regex")]
#[builtin(fields(
    cache: Rc<RegexCacheInner>,
))]
pub fn builtin_tanka_regex_match(
	this: &builtin_tanka_regex_match,
	regex: IStr,
	string: String,
) -> Result<bool> {
	let regex = this.cache.parse(regex)?;
	Ok(regex.is_match(&string))
}

/// Tanka-compatible regexSubst
/// Replaces all matches of regex with replacement string
#[cfg(feature = "exp-regex")]
#[builtin(fields(
    cache: Rc<RegexCacheInner>,
))]
pub fn builtin_tanka_regex_subst(
	this: &builtin_tanka_regex_subst,
	regex: IStr,
	src: String,
	repl: String,
) -> Result<String> {
	let regex = this.cache.parse(regex)?;
	let replaced = regex.replace_all(&src, repl.as_str());
	Ok(replaced.to_string())
}

/// Tanka-compatible helmTemplate
/// Executes `helm template` and returns the rendered manifests as an object
/// Each manifest is keyed by "<snake_case_kind>_<snake_case_name>"
#[builtin]
pub fn builtin_tanka_helm_template(name: String, chart: String, opts: ObjValue) -> Result<Val> {
	// calledFrom is required for proper path resolution

	let called_from = opts.get("calledFrom".into())?.ok_or_else(|| {
		RuntimeError("helmTemplate requires calledFrom field (usually std.thisFile)".into())
	})?;

	// Resolve chart path relative to calledFrom
	let chart_path = if let Val::Str(s) = called_from {
		let called_from_str = s.to_string();

		// Check that calledFrom is not empty
		if called_from_str.is_empty() {
			return Err(RuntimeError("calledFrom cannot be an empty string".into()).into());
		}

		let called_from_path = std::path::Path::new(&called_from_str);
		// Get the directory containing the calling file
		if let Some(dir) = called_from_path.parent() {
			// Check if directory exists
			if !dir.exists() {
				return Err(RuntimeError(
					format!("calledFrom directory does not exist: {}", dir.display()).into(),
				)
				.into());
			}
			// Join the chart path with the directory
			let chart_full = dir.join(&chart);

			// Check if the chart path exists
			if !chart_full.exists() {
				return Err(RuntimeError(
					format!("chart path does not exist: {}", chart_full.display()).into(),
				)
				.into());
			}

			chart_full
				.to_str()
				.ok_or_else(|| RuntimeError("invalid chart path".into()))?
				.to_string()
		} else {
			return Err(RuntimeError(
				format!("calledFrom has no parent directory: {}", called_from_str).into(),
			)
			.into());
		}
	} else {
		return Err(RuntimeError("calledFrom must be a string".into()).into());
	};

	let mut cmd = Command::new("helm");
	cmd.arg("template");
	cmd.arg(&name);
	cmd.arg(&chart_path);

	// Parse other options
	// namespace
	if let Some(ns) = opts.get("namespace".into())? {
		if let Val::Str(s) = ns {
			cmd.arg("--namespace");
			cmd.arg(&s.to_string());
		}
	}

	// values - marshal as JSON (which is valid YAML) and pipe to helm via stdin
	let values_yaml = if let Some(values) = opts.get("values".into())? {
		let json_str = serde_json::to_string(&values)
			.map_err(|e| RuntimeError(format!("failed to serialize values to json: {e}").into()))?;
		Some(json_str)
	} else {
		None
	};

	// If we have values, configure stdin and add --values=-
	if values_yaml.is_some() {
		cmd.arg("--values=-");
		cmd.stdin(Stdio::piped());
	}
	cmd.stdout(Stdio::piped());
	cmd.stderr(Stdio::piped());

	let mut child = cmd
		.spawn()
		.map_err(|e| RuntimeError(format!("failed to execute helm: {e}").into()))?;

	// Write values to stdin if present, then close it
	if let Some(yaml) = values_yaml {
		if let Some(mut stdin) = child.stdin.take() {
			stdin.write_all(yaml.as_bytes()).map_err(|e| {
				RuntimeError(format!("failed to write values to helm stdin: {e}").into())
			})?;
			// Close stdin explicitly
			drop(stdin);
		}
	}

	// Take stdout and stderr handles
	let stdout = child
		.stdout
		.take()
		.ok_or_else(|| RuntimeError("failed to capture helm stdout".into()))?;
	let stderr = child
		.stderr
		.take()
		.ok_or_else(|| RuntimeError("failed to capture helm stderr".into()))?;

	// Spawn a thread to collect stderr
	let stderr_handle = thread::spawn(move || {
		let mut stderr_buf = Vec::new();
		let mut stderr_reader = BufReader::new(stderr);
		stderr_reader.read_to_end(&mut stderr_buf).ok();
		stderr_buf
	});

	// Parse YAML output while streaming from stdout
	use jrsonnet_evaluator::ObjValueBuilder;
	let mut builder = ObjValueBuilder::new();
	let stdout_reader = BufReader::new(stdout);
	let deserializer = serde_yaml::Deserializer::from_reader(stdout_reader);

	for document in deserializer {
		let val: Val = Val::deserialize(document)
			.map_err(|e| RuntimeError(format!("failed to parse helm output: {e}").into()))?;
		// Skip null documents
		if matches!(val, Val::Null) {
			continue;
		}

		// Generate a key for this manifest: <snake_case_kind>_<snake_case_name>
		let key = if let Val::Obj(ref obj) = val {
			let kind = obj
				.get("kind".into())?
				.and_then(|v| match v {
					Val::Str(s) => Some(to_snake_case(&s.to_string())),
					_ => None,
				})
				.unwrap_or_else(|| "unknown".to_string());

			let metadata = obj.get("metadata".into())?;
			let name = if let Some(Val::Obj(meta)) = metadata {
				meta.get("name".into())?
					.and_then(|v| match v {
						Val::Str(s) => Some(to_snake_case(&s.to_string())),
						_ => None,
					})
					.unwrap_or_else(|| "unknown".to_string())
			} else {
				"unknown".to_string()
			};

			format!("{}_{}", kind, name)
		} else {
			"unknown".to_string()
		};

		builder.field(&key).try_value(val)?;
	}

	// Wait for the process to complete
	let status = child
		.wait()
		.map_err(|e| RuntimeError(format!("failed to wait for helm: {e}").into()))?;

	// Get stderr from the thread
	let stderr_buf = stderr_handle
		.join()
		.map_err(|_| RuntimeError("failed to join stderr thread".into()))?;

	// Check if helm command succeeded
	if !status.success() {
		let stderr = String::from_utf8_lossy(&stderr_buf);
		return Err(RuntimeError(format!("helm template failed: {stderr}").into()).into());
	}

	Ok(Val::Obj(builder.build()))
}

/// Tanka-compatible kustomizeBuild
/// Executes `kustomize build` and returns the rendered manifests as an object
/// Each manifest is keyed by "<snake_case_kind>_<snake_case_name>"
#[builtin]
pub fn builtin_tanka_kustomize_build(path: String) -> Result<Val> {
	let mut cmd = Command::new("kustomize");
	cmd.arg("build");
	cmd.arg(&path);
	cmd.stdout(Stdio::piped());
	cmd.stderr(Stdio::piped());

	let mut child = cmd
		.spawn()
		.map_err(|e| RuntimeError(format!("failed to execute kustomize: {e}").into()))?;

	// Take stdout and stderr handles
	let stdout = child
		.stdout
		.take()
		.ok_or_else(|| RuntimeError("failed to capture kustomize stdout".into()))?;
	let stderr = child
		.stderr
		.take()
		.ok_or_else(|| RuntimeError("failed to capture kustomize stderr".into()))?;

	// Spawn a thread to collect stderr
	let stderr_handle = thread::spawn(move || {
		let mut stderr_buf = Vec::new();
		let mut stderr_reader = BufReader::new(stderr);
		stderr_reader.read_to_end(&mut stderr_buf).ok();
		stderr_buf
	});

	// Parse YAML output while streaming from stdout
	use jrsonnet_evaluator::ObjValueBuilder;
	let mut builder = ObjValueBuilder::new();
	let stdout_reader = BufReader::new(stdout);
	let deserializer = serde_yaml::Deserializer::from_reader(stdout_reader);

	for document in deserializer {
		let val: Val = Val::deserialize(document)
			.map_err(|e| RuntimeError(format!("failed to parse kustomize output: {e}").into()))?;
		// Skip null documents
		if matches!(val, Val::Null) {
			continue;
		}

		// Generate a key for this manifest: <snake_case_kind>_<snake_case_name>
		let key = if let Val::Obj(ref obj) = val {
			let kind = obj
				.get("kind".into())?
				.and_then(|v| match v {
					Val::Str(s) => Some(to_snake_case(&s.to_string())),
					_ => None,
				})
				.unwrap_or_else(|| "unknown".to_string());

			let metadata = obj.get("metadata".into())?;
			let name = if let Some(Val::Obj(meta)) = metadata {
				meta.get("name".into())?
					.and_then(|v| match v {
						Val::Str(s) => Some(to_snake_case(&s.to_string())),
						_ => None,
					})
					.unwrap_or_else(|| "unknown".to_string())
			} else {
				"unknown".to_string()
			};

			format!("{}_{}", kind, name)
		} else {
			"unknown".to_string()
		};

		builder.field(&key).try_value(val)?;
	}

	// Wait for the process to complete
	let status = child
		.wait()
		.map_err(|e| RuntimeError(format!("failed to wait for kustomize: {e}").into()))?;

	// Get stderr from the thread
	let stderr_buf = stderr_handle
		.join()
		.map_err(|_| RuntimeError("failed to join stderr thread".into()))?;

	// Check if kustomize command succeeded
	if !status.success() {
		let stderr = String::from_utf8_lossy(&stderr_buf);
		return Err(RuntimeError(format!("kustomize build failed: {stderr}").into()).into());
	}

	Ok(Val::Obj(builder.build()))
}
