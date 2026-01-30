// Tanka-compatible native functions
// These are wrappers around the existing stdlib functions to provide
// Tanka-compatible API accessible via std.native()

use std::{
	collections::HashMap,
	io::{BufReader, Read, Write},
	process::{Command, Stdio},
	sync::RwLock,
	thread,
};

use jrsonnet_evaluator::{
	error::{ErrorKind::*, Result},
	IStr, ObjValue, Val,
};
use jrsonnet_macros::builtin;
use jrsonnet_stdlib::RegexCacheInner;
use serde_json;
use sha2::{Digest, Sha256};

use std::rc::Rc;

// Global Helm template cache - caches raw YAML output from helm to avoid
// redundant helm invocations (same optimization as Go Tanka)
// We cache the raw YAML string rather than Val because Val doesn't implement Sync
static HELM_TEMPLATE_CACHE: RwLock<Option<HashMap<String, String>>> = RwLock::new(None);

/// Get or create the Helm template cache
fn get_helm_cache() -> &'static RwLock<Option<HashMap<String, String>>> {
	// Initialize the cache if needed
	{
		let read = HELM_TEMPLATE_CACHE.read().unwrap();
		if read.is_some() {
			return &HELM_TEMPLATE_CACHE;
		}
	}
	{
		let mut write = HELM_TEMPLATE_CACHE.write().unwrap();
		if write.is_none() {
			*write = Some(HashMap::new());
		}
	}
	&HELM_TEMPLATE_CACHE
}

/// Generate a key for a manifest using the nameFormat template
/// This is a simplified implementation that handles the common case where nameFormat
/// includes namespace in the key format
fn generate_manifest_key_from_val(val: &Val, name_format: Option<&str>) -> Result<String> {
	// Check if we should use nameFormat or default format
	let use_namespace_in_key = name_format
		.map(|fmt| fmt.contains("metadata.namespace") || fmt.contains(".or .metadata.namespace"))
		.unwrap_or(false);

	if let Val::Obj(ref obj) = val {
		let kind = obj
			.get("kind".into())
			.ok()
			.flatten()
			.and_then(|v| match v {
				Val::Str(s) => Some(to_snake_case(&s.to_string())),
				_ => None,
			})
			.unwrap_or_else(|| "unknown".to_string());

		let metadata = obj.get("metadata".into()).ok().flatten();

		if let Some(Val::Obj(meta)) = metadata {
			let name = meta
				.get("name".into())
				.ok()
				.flatten()
				.and_then(|v| match v {
					Val::Str(s) => Some(to_snake_case(&s.to_string())),
					_ => None,
				})
				.unwrap_or_else(|| "unknown".to_string());

			// If nameFormat suggests using namespace, include it in the key
			if use_namespace_in_key {
				let namespace = meta
					.get("namespace".into())
					.ok()
					.flatten()
					.and_then(|v| match v {
						Val::Str(s) => Some(to_snake_case(&s.to_string())),
						_ => None,
					})
					.unwrap_or_else(|| "cluster".to_string());

				return Ok(format!("{}_{}_{}", namespace, kind, name));
			} else {
				return Ok(format!("{}_{}", kind, name));
			}
		}
	}

	Ok("unknown".to_string())
}

/// Parse YAML output from helm into a Val object
fn parse_helm_yaml_output(yaml_content: &str, name_format: Option<&str>) -> Result<Val> {
	use jrsonnet_evaluator::ObjValueBuilder;
	let mut builder = ObjValueBuilder::new();
	// Use serde-saphyr which properly handles YAML 1.1 features including:
	// - Multiple merge keys (<<) in the same mapping
	// - Octal numbers (0755 -> 493)
	let options = serde_saphyr::Options {
		legacy_octal_numbers: true,
		budget: None, // Disable budget limits - we trust the YAML input
		..Default::default()
	};
	let documents: Vec<Val> = serde_saphyr::from_multiple_with_options(yaml_content, options)
		.map_err(|e| RuntimeError(format!("failed to parse helm output: {e}").into()))?;
	let mut seen_keys = HashMap::new();

	for val in documents {
		// Skip null documents
		if matches!(val, Val::Null) {
			continue;
		}

		// Skip non-object values
		if !matches!(val, Val::Obj(_)) {
			continue;
		}

		// Use the nameFormat-aware key generation
		let key = generate_manifest_key_from_val(&val, name_format)?;

		// Check for duplicate keys and add counter if needed
		let mut final_key = key.clone();
		let mut counter = 2;
		while seen_keys.contains_key(&final_key) {
			final_key = format!("{}_{}", key, counter);
			counter += 1;
		}
		seen_keys.insert(final_key.clone(), ());

		builder.field(&final_key).try_value(val)?;
	}

	Ok(Val::Obj(builder.build()))
}

/// Generate a cache key for Helm template
fn helm_cache_key(
	name: &str,
	chart_path: &str,
	namespace: Option<&str>,
	values_json: Option<&str>,
	include_crds: bool,
	api_versions: &[String],
) -> String {
	let mut hasher = Sha256::new();
	hasher.update(name.as_bytes());
	hasher.update(b"|");
	hasher.update(chart_path.as_bytes());
	hasher.update(b"|");
	if let Some(ns) = namespace {
		hasher.update(ns.as_bytes());
	}
	hasher.update(b"|");
	if let Some(v) = values_json {
		hasher.update(v.as_bytes());
	}
	hasher.update(b"|");
	hasher.update(if include_crds { b"1" } else { b"0" });
	hasher.update(b"|");
	for av in api_versions {
		hasher.update(av.as_bytes());
		hasher.update(b",");
	}
	format!("{:x}", hasher.finalize())
}

/// Convert a string to snake_case (lowercase with underscores)
/// Matches Go Tanka's naming behavior which inserts underscores:
/// - Before uppercase letters (CamelCase -> camel_case)
/// - Between letter-digit-letter sequences (k8s -> k_8s)
/// Note: Does NOT insert underscore when digit is at word boundary (flux2 stays flux2)
fn to_snake_case(s: &str) -> String {
	let mut result = String::new();
	let chars: Vec<char> = s.chars().collect();

	for (i, &ch) in chars.iter().enumerate() {
		if ch.is_uppercase() {
			// Add underscore before uppercase letters (except at start)
			if !result.is_empty() {
				result.push('_');
			}
			result.push(ch.to_lowercase().next().unwrap());
		} else if ch == '-' {
			// Replace hyphens with underscores
			result.push('_');
		} else if ch.is_ascii_digit() {
			// Add underscore between letter and digit ONLY if there's a letter eventually
			// after the consecutive digits. This matches Go Tanka:
			// - k8s -> k_8s (letter after digit)
			// - o11y -> o_11y (letter eventually after digits)
			// - flux2 -> flux2 (no letter after digit, at end or before hyphen)
			let prev_is_letter = i > 0 && chars[i - 1].is_ascii_alphabetic();
			if prev_is_letter {
				// Look ahead past all consecutive digits to see if there's a letter
				let has_letter_after_digits = chars[i..]
					.iter()
					.skip_while(|c| c.is_ascii_digit())
					.next()
					.map(|c| c.is_ascii_alphabetic())
					.unwrap_or(false);
				if has_letter_after_digits {
					result.push('_');
				}
			}
			result.push(ch);
		} else {
			result.push(ch);
		}
	}

	result
}

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
	// Use serde-saphyr which properly handles YAML 1.1 features including:
	// - Multiple merge keys (<<) in the same mapping
	// - Octal numbers (0755 -> 493)
	let options = serde_saphyr::Options {
		legacy_octal_numbers: true,
		budget: None, // Disable budget limits - we trust the YAML input
		..Default::default()
	};
	let documents: Vec<Val> = serde_saphyr::from_multiple_with_options(&yaml, options)
		.map_err(|e| RuntimeError(format!("failed to parse yaml: {e}").into()))?;

	Ok(Val::Arr(documents.into()))
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

/// Recursively sort JSON object keys using go-yaml v3's natural sort algorithm
/// This matches Go yaml.v3 behavior from sorter.go
fn sort_json_keys_numerically(value: serde_json::Value) -> serde_json::Value {
	match value {
		serde_json::Value::Object(map) => {
			// Collect and sort using go-yaml v3's natural sort
			let mut entries: Vec<(String, serde_json::Value)> = map.into_iter().collect();
			entries.sort_by(|(a, _), (b, _)| yaml_v3_key_compare(a, b));

			// Rebuild the map with sorted keys
			let sorted: serde_json::Map<String, serde_json::Value> = entries
				.into_iter()
				.map(|(k, v)| (k, sort_json_keys_numerically(v)))
				.collect();
			serde_json::Value::Object(sorted)
		}
		serde_json::Value::Array(arr) => {
			serde_json::Value::Array(arr.into_iter().map(sort_json_keys_numerically).collect())
		}
		other => other,
	}
}

/// Implements go-yaml v3's key comparison algorithm (from sorter.go)
/// This is a "natural sort" where:
/// - Numbers are sorted numerically
/// - Letters are sorted before non-letters when transitioning from digits
/// - Non-letters (like '_') are sorted before letters when not in digit context
fn yaml_v3_key_compare(a: &str, b: &str) -> std::cmp::Ordering {
	let ar: Vec<char> = a.chars().collect();
	let br: Vec<char> = b.chars().collect();
	let mut digits = false;

	let min_len = ar.len().min(br.len());
	for i in 0..min_len {
		if ar[i] == br[i] {
			digits = ar[i].is_ascii_digit();
			continue;
		}

		let al = ar[i].is_alphabetic();
		let bl = br[i].is_alphabetic();

		if al && bl {
			return ar[i].cmp(&br[i]);
		}

		if al || bl {
			// One is a letter, one is not
			if digits {
				// After digits: letters come first
				return if al {
					std::cmp::Ordering::Less
				} else {
					std::cmp::Ordering::Greater
				};
			} else {
				// Not after digits: non-letters come first
				return if bl {
					std::cmp::Ordering::Less
				} else {
					std::cmp::Ordering::Greater
				};
			}
		}

		// Both are non-letters - check for numeric sequences
		// Handle leading zeros
		let mut an: i64 = 0;
		let mut bn: i64 = 0;

		if ar[i] == '0' || br[i] == '0' {
			// Check if previous chars were non-zero digits
			let mut j = i;
			while j > 0 && ar[j - 1].is_ascii_digit() {
				j -= 1;
				if ar[j] != '0' {
					an = 1;
					bn = 1;
					break;
				}
			}
		}

		// Parse numeric sequences
		let mut ai = i;
		while ai < ar.len() && ar[ai].is_ascii_digit() {
			an = an * 10 + (ar[ai] as i64 - '0' as i64);
			ai += 1;
		}

		let mut bi = i;
		while bi < br.len() && br[bi].is_ascii_digit() {
			bn = bn * 10 + (br[bi] as i64 - '0' as i64);
			bi += 1;
		}

		if an != bn {
			return an.cmp(&bn);
		}
		if ai != bi {
			return ai.cmp(&bi);
		}
		return ar[i].cmp(&br[i]);
	}

	ar.len().cmp(&br.len())
}

/// Tanka-compatible manifestYamlFromJson
/// Converts JSON string to YAML using Go yaml.v3 compatible settings
#[builtin]
pub fn builtin_tanka_manifest_yaml_from_json(json: String) -> Result<String> {
	let parsed: serde_json::Value = serde_json::from_str(&json)
		.map_err(|e| RuntimeError(format!("failed to parse json: {e}").into()))?;

	// Sort keys numerically to match Go yaml.v3 behavior
	let sorted = sort_json_keys_numerically(parsed);

	// Use serde-saphyr with Go yaml.v3 compatible settings
	// This matches tk's manifestYamlFromJson which uses go-yaml v3
	// Go yaml.v3's yaml.Marshal() defaults to best_width = 2^31-1 (no wrapping)
	let options = serde_saphyr::SerializerOptions {
		indent_step: 4,     // go-yaml v3 uses 4-space indentation
		indent_array: None, // use indent_step for arrays too
		prefer_block_scalars: true,
		empty_map_as_braces: true,
		empty_array_as_brackets: true,
		block_scalar_indent_in_seq: Some(2), // 2 spaces absolute for block scalar body in arrays
		line_width: None,                    // go-yaml v3's Marshal() doesn't wrap lines by default
		scientific_notation_threshold: Some(1000000), // 1 million - large numbers use scientific notation
		scientific_notation_small_threshold: Some(0.0001), // 1e-4 - small numbers use scientific notation (Go yaml.v3)
		quote_numeric_strings: true,                       // Quote numeric string keys like "12345"
		..Default::default()
	};
	let mut output = String::new();
	serde_saphyr::to_fmt_writer_with_options(&mut output, &sorted, options)
		.map_err(|e| RuntimeError(format!("failed to serialize yaml: {e}").into()))?;

	// Add trailing newline to match Go's yaml.v3 behavior
	// This ensures the outer YAML serializer uses | instead of |-
	if !output.ends_with('\n') {
		output.push('\n');
	}

	Ok(output)
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
	regex::escape(&pattern)
}

/// Tanka-compatible regexMatch
/// Returns true if the string matches the regex pattern
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
			// Prevent absolute paths by prefixing with '.' if chart starts with '/'
			let chart_relative = if chart.starts_with('/') {
				format!(".{}", chart)
			} else {
				chart
			};
			// Join the chart path with the directory
			let chart_full = dir.join(&chart_relative);

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

	// Extract namespace for cache key
	let namespace = if let Some(ns) = opts.get("namespace".into())? {
		if let Val::Str(s) = ns {
			Some(s.to_string())
		} else {
			None
		}
	} else {
		None
	};

	// Extract values and serialize to JSON for cache key
	let values_json =
		if let Some(values) = opts.get("values".into())? {
			Some(serde_json::to_string(&values).map_err(|e| {
				RuntimeError(format!("failed to serialize values to json: {e}").into())
			})?)
		} else {
			None
		};

	// Extract nameFormat if present
	let name_format = if let Some(nf) = opts.get("nameFormat".into())? {
		if let Val::Str(s) = nf {
			Some(s.to_string())
		} else {
			None
		}
	} else {
		None
	};

	// Extract includeCrds if present (defaults to true, matching Go Tanka's behavior)
	// Go Tanka: "default IncludeCRDs to true, as this is the default in the `helm install`"
	let include_crds = if let Some(ic) = opts.get("includeCrds".into())? {
		matches!(ic, Val::Bool(true))
	} else {
		true
	};

	// Extract apiVersions if present (array of strings for --api-versions flag)
	let api_versions: Vec<String> = if let Some(av) = opts.get("apiVersions".into())? {
		if let Val::Arr(arr) = av {
			arr.iter()
				.filter_map(|v| {
					if let Ok(Val::Str(s)) = v {
						Some(s.to_string())
					} else {
						None
					}
				})
				.collect()
		} else {
			Vec::new()
		}
	} else {
		Vec::new()
	};

	// Check cache first
	let cache_key = helm_cache_key(
		&name,
		&chart_path,
		namespace.as_deref(),
		values_json.as_deref(),
		include_crds,
		&api_versions,
	);
	{
		let cache = get_helm_cache();
		let read = cache.read().unwrap();
		if let Some(ref map) = *read {
			if let Some(cached_yaml) = map.get(&cache_key) {
				// Cache hit - parse the cached YAML
				return parse_helm_yaml_output(cached_yaml, name_format.as_deref());
			}
		}
	}

	let mut cmd = Command::new("helm");
	cmd.arg("template");
	cmd.arg(&name);
	cmd.arg(&chart_path);

	// Add namespace if present
	if let Some(ref ns) = namespace {
		cmd.arg("--namespace");
		cmd.arg(ns);
	}

	// Add --include-crds if requested
	if include_crds {
		cmd.arg("--include-crds");
	}

	// Add --api-versions for each version specified
	for av in &api_versions {
		cmd.arg("--api-versions");
		cmd.arg(av);
	}

	// If we have values, configure stdin and add --values=-
	if values_json.is_some() {
		cmd.arg("--values=-");
		cmd.stdin(Stdio::piped());
	}
	cmd.stdout(Stdio::piped());
	cmd.stderr(Stdio::piped());

	let mut child = cmd
		.spawn()
		.map_err(|e| RuntimeError(format!("failed to execute helm: {e}").into()))?;

	// Write values to stdin if present, then close it
	if let Some(ref json) = values_json {
		if let Some(mut stdin) = child.stdin.take() {
			stdin.write_all(json.as_bytes()).map_err(|e| {
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

	// Spawn threads to collect stdout and stderr in parallel
	let stdout_handle = thread::spawn(move || {
		let mut stdout_buf = Vec::new();
		let mut stdout_reader = BufReader::new(stdout);
		stdout_reader.read_to_end(&mut stdout_buf).ok();
		stdout_buf
	});

	let stderr_handle = thread::spawn(move || {
		let mut stderr_buf = Vec::new();
		let mut stderr_reader = BufReader::new(stderr);
		stderr_reader.read_to_end(&mut stderr_buf).ok();
		stderr_buf
	});

	// Wait for the process to complete
	let status = child
		.wait()
		.map_err(|e| RuntimeError(format!("failed to wait for helm: {e}").into()))?;

	// Get stdout from the thread
	let stdout_buf = stdout_handle
		.join()
		.map_err(|_| RuntimeError("failed to join stdout thread".into()))?;

	// Get stderr from the thread
	let stderr_buf = stderr_handle
		.join()
		.map_err(|_| RuntimeError("failed to join stderr thread".into()))?;

	// Check if helm command succeeded
	if !status.success() {
		let stderr = String::from_utf8_lossy(&stderr_buf);
		return Err(RuntimeError(format!("helm template failed: {stderr}").into()).into());
	}

	// Convert stdout to string (YAML content)
	let yaml_content = String::from_utf8(stdout_buf)
		.map_err(|e| RuntimeError(format!("invalid UTF-8 in helm output: {e}").into()))?;

	// Store raw YAML in cache before parsing
	{
		let cache = get_helm_cache();
		let mut write = cache.write().unwrap();
		if let Some(ref mut map) = *write {
			map.insert(cache_key, yaml_content.clone());
		}
	}

	// Parse and return the YAML output
	parse_helm_yaml_output(&yaml_content, name_format.as_deref())
}

/// Tanka-compatible kustomizeBuild
/// Executes `kustomize build` and returns the rendered manifests as an object
/// Each manifest is keyed by "<snake_case_kind>_<snake_case_name>"
#[builtin]
pub fn builtin_tanka_kustomize_build(path: String, opts: ObjValue) -> Result<Val> {
	// calledFrom is required for proper path resolution
	let called_from = opts.get("calledFrom".into())?.ok_or_else(|| {
		RuntimeError("kustomizeBuild requires calledFrom field (usually std.thisFile)".into())
	})?;

	// Resolve kustomize path relative to calledFrom
	let kustomize_path = if let Val::Str(s) = called_from {
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
			// Prevent absolute paths by prefixing with '.' if path starts with '/'
			let path_relative = if path.starts_with('/') {
				format!(".{}", path)
			} else {
				path
			};
			// Join the kustomize path with the directory
			let kustomize_full = dir.join(&path_relative);

			// Check if the kustomize path exists
			if !kustomize_full.exists() {
				return Err(RuntimeError(
					format!(
						"kustomize path does not exist: {}",
						kustomize_full.display()
					)
					.into(),
				)
				.into());
			}

			kustomize_full
				.to_str()
				.ok_or_else(|| RuntimeError("invalid kustomize path".into()))?
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

	let mut cmd = Command::new("kustomize");
	cmd.arg("build");
	cmd.arg(&kustomize_path);
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

	// Read stdout and parse YAML output
	use jrsonnet_evaluator::ObjValueBuilder;
	let mut builder = ObjValueBuilder::new();
	let mut stdout_reader = BufReader::new(stdout);
	let mut yaml_content = String::new();
	stdout_reader
		.read_to_string(&mut yaml_content)
		.map_err(|e| RuntimeError(format!("failed to read kustomize output: {e}").into()))?;

	// Use serde-saphyr which properly handles YAML 1.1 features
	let options = serde_saphyr::Options {
		legacy_octal_numbers: true,
		budget: None, // Disable budget limits - we trust the YAML input
		..Default::default()
	};
	let documents: Vec<Val> = serde_saphyr::from_multiple_with_options(&yaml_content, options)
		.map_err(|e| RuntimeError(format!("failed to parse kustomize output: {e}").into()))?;
	let mut seen_keys = HashMap::new();

	for val in documents {
		// Skip null documents
		if matches!(val, Val::Null) {
			continue;
		}

		// Generate a key for this manifest: <snake_case_kind>_<snake_case_name>
		// Note: tk does NOT include namespace in the key, even when present
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

		// Check for duplicate keys and add counter if needed
		let mut final_key = key.clone();
		let mut counter = 2;
		while seen_keys.contains_key(&final_key) {
			final_key = format!("{}_{}", key, counter);
			counter += 1;
		}
		seen_keys.insert(final_key.clone(), ());

		builder.field(&final_key).try_value(val)?;
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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_yaml_octal_parsing() {
		// YAML 1.1 octal: 0755 -> 493 decimal
		let yaml = "myval: 0755";
		let result = builtin_tanka_parse_yaml(yaml.to_string()).unwrap();
		if let Val::Arr(arr) = result {
			let val = arr.get(0).unwrap().unwrap();
			if let Val::Obj(obj) = val {
				let myval = obj.get("myval".into()).unwrap().unwrap();
				if let Val::Num(n) = myval {
					assert_eq!(n.get(), 493.0);
				} else {
					panic!("Expected number, got {:?}", myval);
				}
			} else {
				panic!("Expected object");
			}
		} else {
			panic!("Expected array");
		}

		// Also test double-zero prefix (00755)
		let yaml = "myval: 00755";
		let result = builtin_tanka_parse_yaml(yaml.to_string()).unwrap();
		if let Val::Arr(arr) = result {
			let val = arr.get(0).unwrap().unwrap();
			if let Val::Obj(obj) = val {
				let myval = obj.get("myval".into()).unwrap().unwrap();
				if let Val::Num(n) = myval {
					assert_eq!(n.get(), 493.0);
				} else {
					panic!("Expected number, got {:?}", myval);
				}
			} else {
				panic!("Expected object");
			}
		} else {
			panic!("Expected array");
		}
	}
}
