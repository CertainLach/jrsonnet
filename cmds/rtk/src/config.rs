//! Configuration file support for rtk
//!
//! Supports `.rtk-config.yaml` files that can be placed anywhere in the directory
//! hierarchy. rtk searches from the environment directory upward to the filesystem root.

use std::{
	fs,
	path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::Deserialize;

/// The name of the config file rtk looks for
pub const CONFIG_FILE_NAME: &str = ".rtk-config.yaml";

/// Root configuration structure for .rtk-config.yaml
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RtkConfig {
	/// Output format settings for various Jsonnet functions
	#[serde(default)]
	pub output_format: OutputFormatConfig,

	/// When true, disables Tanka-specific native functions (manifestYamlFromJson,
	/// parseYaml, parseJson, etc.). This is useful when tk uses jrsonnet binary
	/// via exportJsonnetImplementation, where these native functions are not available
	/// and the jsonnet code falls back to std.manifestYamlDoc.
	#[serde(default)]
	pub disable_tanka_native_functions: bool,
}

/// Output format configuration for Jsonnet evaluation
///
/// Use "jrsonnet" values for environments that use tk with exportJsonnetImplementation
/// pointing to a jrsonnet binary, to match the output format.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputFormatConfig {
	/// Controls float formatting in std.toString and related functions.
	///
	/// - "go-jsonnet" (default): Use Go's %.17g format (e.g., 0.59999999999999998)
	/// - "jrsonnet": Use shortest representation (e.g., 0.6)
	#[serde(default)]
	pub floats: Option<JsonnetImplementation>,

	/// Controls the output format for std.manifestYamlDoc.
	///
	/// - "go-jsonnet" (default): values are always quoted, regardless of quote_keys setting
	/// - "jrsonnet": quote_values follows quote_keys (when quote_keys=false, quote_values=false)
	#[serde(default, rename = "std.manifestYamlDoc")]
	pub std_manifest_yaml_doc: Option<JsonnetImplementation>,

	/// Controls the output format for std.manifestYamlStream with empty arrays.
	///
	/// - "go-jsonnet" (default): Empty arrays produce "---\n\n" (document marker + empty line)
	/// - "jrsonnet": Empty arrays produce "\n" (just a newline)
	#[serde(default, rename = "std.manifestYamlStream")]
	pub std_manifest_yaml_stream: Option<JsonnetImplementation>,
}

/// Specifies which jsonnet implementation's behavior to match
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JsonnetImplementation {
	/// Match go-jsonnet behavior (default)
	#[default]
	GoJsonnet,
	/// Match jrsonnet binary behavior
	Jrsonnet,
}

impl RtkConfig {
	/// Load config by searching from the given directory upward
	pub fn load_from_directory(start_dir: &Path) -> Result<Option<Self>> {
		if let Some(config_path) = find_config_file(start_dir) {
			let config = Self::load_from_file(&config_path)?;
			Ok(Some(config))
		} else {
			Ok(None)
		}
	}

	/// Load config from a specific file path
	pub fn load_from_file(path: &Path) -> Result<Self> {
		let content = fs::read_to_string(path)
			.with_context(|| format!("failed to read config file: {}", path.display()))?;
		let config: RtkConfig = serde_yaml_with_quirks::from_str(&content)
			.with_context(|| format!("failed to parse config file: {}", path.display()))?;
		Ok(config)
	}

	/// Create config with jrsonnet defaults.
	/// Used when spec.exportJsonnetImplementation points to a jrsonnet binary.
	pub fn jrsonnet_defaults() -> Self {
		Self {
			disable_tanka_native_functions: true,
			output_format: OutputFormatConfig {
				floats: Some(JsonnetImplementation::Jrsonnet),
				std_manifest_yaml_doc: Some(JsonnetImplementation::Jrsonnet),
				std_manifest_yaml_stream: Some(JsonnetImplementation::Jrsonnet),
			},
		}
	}

	/// Merge file config over this config (file values override defaults where set)
	pub fn merge_from(&mut self, file_config: &RtkConfig) {
		// File config explicitly set disable_tanka_native_functions
		// Since it's a bool with default false, we can't distinguish "not set" from "set to false"
		// So we always take the file value if it's true, otherwise keep self
		if file_config.disable_tanka_native_functions {
			self.disable_tanka_native_functions = true;
		}

		// For Option fields, only override if file config has Some value
		if file_config.output_format.floats.is_some() {
			self.output_format.floats = file_config.output_format.floats;
		}
		if file_config.output_format.std_manifest_yaml_doc.is_some() {
			self.output_format.std_manifest_yaml_doc =
				file_config.output_format.std_manifest_yaml_doc;
		}
		if file_config.output_format.std_manifest_yaml_stream.is_some() {
			self.output_format.std_manifest_yaml_stream =
				file_config.output_format.std_manifest_yaml_stream;
		}
	}
}

/// Check if exportJsonnetImplementation indicates jrsonnet binary usage
pub fn uses_jrsonnet_binary(export_impl: Option<&str>) -> bool {
	export_impl
		.map(|s| s.starts_with("binary:") && s.contains("jrsonnet"))
		.unwrap_or(false)
}

/// Search for a config file starting from `start_dir` and walking up to the filesystem root
pub fn find_config_file(start_dir: &Path) -> Option<PathBuf> {
	let mut current = start_dir.to_path_buf();

	// Canonicalize if possible to handle relative paths
	if let Ok(canonical) = current.canonicalize() {
		current = canonical;
	}

	loop {
		let config_path = current.join(CONFIG_FILE_NAME);
		if config_path.exists() {
			return Some(config_path);
		}

		// Move up to parent directory
		if let Some(parent) = current.parent() {
			if parent == current {
				// Reached root
				break;
			}
			current = parent.to_path_buf();
		} else {
			break;
		}
	}

	None
}

#[cfg(test)]
mod tests {
	use tempfile::TempDir;

	use super::*;

	#[test]
	fn test_find_config_in_current_dir() {
		let temp = TempDir::new().unwrap();
		let config_path = temp.path().join(CONFIG_FILE_NAME);
		fs::write(
			&config_path,
			"outputFormat:\n  std.manifestYamlDoc: jrsonnet",
		)
		.unwrap();

		let found = find_config_file(temp.path());
		// Compare file names only to avoid canonicalization issues on macOS
		assert!(found.is_some());
		assert_eq!(found.unwrap().file_name(), config_path.file_name());
	}

	#[test]
	fn test_find_config_in_parent_dir() {
		let temp = TempDir::new().unwrap();
		let config_path = temp.path().join(CONFIG_FILE_NAME);
		fs::write(
			&config_path,
			"outputFormat:\n  std.manifestYamlDoc: jrsonnet",
		)
		.unwrap();

		// Create a subdirectory
		let subdir = temp.path().join("subdir");
		fs::create_dir(&subdir).unwrap();

		let found = find_config_file(&subdir);
		// Compare file names only to avoid canonicalization issues on macOS
		assert!(found.is_some());
		assert_eq!(found.unwrap().file_name(), config_path.file_name());
	}

	#[test]
	fn test_no_config_found() {
		let temp = TempDir::new().unwrap();
		let found = find_config_file(temp.path());
		assert!(found.is_none());
	}

	#[test]
	fn test_load_config_jrsonnet_manifest_yaml_doc() {
		let temp = TempDir::new().unwrap();
		let config_path = temp.path().join(CONFIG_FILE_NAME);
		fs::write(
			&config_path,
			"outputFormat:\n  std.manifestYamlDoc: jrsonnet",
		)
		.unwrap();

		let config = RtkConfig::load_from_file(&config_path).unwrap();
		assert_eq!(
			config.output_format.std_manifest_yaml_doc,
			Some(JsonnetImplementation::Jrsonnet)
		);
	}

	#[test]
	fn test_load_config_go_jsonnet_manifest_yaml_doc() {
		let temp = TempDir::new().unwrap();
		let config_path = temp.path().join(CONFIG_FILE_NAME);
		fs::write(
			&config_path,
			"outputFormat:\n  std.manifestYamlDoc: go-jsonnet",
		)
		.unwrap();

		let config = RtkConfig::load_from_file(&config_path).unwrap();
		assert_eq!(
			config.output_format.std_manifest_yaml_doc,
			Some(JsonnetImplementation::GoJsonnet)
		);
	}

	#[test]
	fn test_load_config_empty_object() {
		let temp = TempDir::new().unwrap();
		let config_path = temp.path().join(CONFIG_FILE_NAME);
		// Empty YAML object, not empty file
		fs::write(&config_path, "{}").unwrap();

		let config = RtkConfig::load_from_file(&config_path).unwrap();
		assert!(config.output_format.std_manifest_yaml_doc.is_none());
		assert!(config.output_format.floats.is_none());
	}

	#[test]
	fn test_load_config_partial() {
		let temp = TempDir::new().unwrap();
		let config_path = temp.path().join(CONFIG_FILE_NAME);
		// Config with empty outputFormat
		fs::write(&config_path, "outputFormat: {}").unwrap();

		let config = RtkConfig::load_from_file(&config_path).unwrap();
		assert!(config.output_format.std_manifest_yaml_doc.is_none());
		assert!(config.output_format.floats.is_none());
	}

	#[test]
	fn test_load_config_from_directory() {
		let temp = TempDir::new().unwrap();
		let config_path = temp.path().join(CONFIG_FILE_NAME);
		fs::write(
			&config_path,
			"outputFormat:\n  std.manifestYamlDoc: jrsonnet",
		)
		.unwrap();

		let subdir = temp.path().join("env").join("default");
		fs::create_dir_all(&subdir).unwrap();

		let config = RtkConfig::load_from_directory(&subdir).unwrap();
		assert!(config.is_some());
		assert_eq!(
			config.unwrap().output_format.std_manifest_yaml_doc,
			Some(JsonnetImplementation::Jrsonnet)
		);
	}

	#[test]
	fn test_load_config_floats_jrsonnet() {
		let temp = TempDir::new().unwrap();
		let config_path = temp.path().join(CONFIG_FILE_NAME);
		fs::write(&config_path, "outputFormat:\n  floats: jrsonnet").unwrap();

		let config = RtkConfig::load_from_file(&config_path).unwrap();
		assert_eq!(
			config.output_format.floats,
			Some(JsonnetImplementation::Jrsonnet)
		);
	}

	#[test]
	fn test_load_config_floats_go_jsonnet() {
		let temp = TempDir::new().unwrap();
		let config_path = temp.path().join(CONFIG_FILE_NAME);
		fs::write(&config_path, "outputFormat:\n  floats: go-jsonnet").unwrap();

		let config = RtkConfig::load_from_file(&config_path).unwrap();
		assert_eq!(
			config.output_format.floats,
			Some(JsonnetImplementation::GoJsonnet)
		);
	}

	#[test]
	fn test_load_config_full() {
		let temp = TempDir::new().unwrap();
		let config_path = temp.path().join(CONFIG_FILE_NAME);
		fs::write(
			&config_path,
			"outputFormat:\n  floats: jrsonnet\n  std.manifestYamlDoc: jrsonnet",
		)
		.unwrap();

		let config = RtkConfig::load_from_file(&config_path).unwrap();
		assert_eq!(
			config.output_format.floats,
			Some(JsonnetImplementation::Jrsonnet)
		);
		assert_eq!(
			config.output_format.std_manifest_yaml_doc,
			Some(JsonnetImplementation::Jrsonnet)
		);
	}

	#[test]
	fn test_load_config_disable_tanka_native_functions() {
		let temp = TempDir::new().unwrap();
		let config_path = temp.path().join(CONFIG_FILE_NAME);
		fs::write(&config_path, "disableTankaNativeFunctions: true").unwrap();

		let config = RtkConfig::load_from_file(&config_path).unwrap();
		assert!(config.disable_tanka_native_functions);
	}

	#[test]
	fn test_load_config_disable_tanka_native_functions_default() {
		let temp = TempDir::new().unwrap();
		let config_path = temp.path().join(CONFIG_FILE_NAME);
		fs::write(&config_path, "{}").unwrap();

		let config = RtkConfig::load_from_file(&config_path).unwrap();
		assert!(!config.disable_tanka_native_functions);
	}
}
