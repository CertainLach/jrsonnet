use std::{
	collections::BTreeMap,
	fs,
	path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use rtk::commands;
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FixtureConfigLayer {
	extra_args: Option<Vec<String>>,
	args: Option<BTreeMap<String, Vec<String>>>,
}

#[derive(Debug, Default)]
struct FixtureConfig {
	extra_args: Vec<String>,
	args: BTreeMap<String, Vec<String>>,
}

impl FixtureConfig {
	fn merge_layer(&mut self, layer: FixtureConfigLayer) {
		if let Some(extra_args) = layer.extra_args {
			self.extra_args.extend(extra_args);
		}
		if let Some(args) = layer.args {
			for (command, new_args) in args {
				self.args.entry(command).or_default().extend(new_args);
			}
		}
	}

	fn args_for_command(&self, command: &str) -> Vec<String> {
		let mut args = self.extra_args.clone();
		if let Some(command_args) = self.args.get(command) {
			args.extend(command_args.clone());
		}
		args
	}
}

#[derive(Parser)]
struct GoldenCli {
	#[command(subcommand)]
	command: GoldenCommand,
}

#[derive(Subcommand)]
enum GoldenCommand {
	Export(commands::export::ExportArgs),
}

fn load_fixture_export_args(suite_path: &Path, env_path: &Path) -> Vec<String> {
	let mut config = FixtureConfig::default();

	// Load suite-level config first (defaults), then fixture-specific overrides
	let mut config_dirs = vec![suite_path];
	if env_path != suite_path {
		config_dirs.push(env_path);
	}

	for config_dir in config_dirs {
		let config_path = config_dir.join("tk-compare.toml");
		if !config_path.exists() {
			continue;
		}
		let content = fs::read_to_string(&config_path)
			.unwrap_or_else(|e| panic!("Failed to read {}: {}", config_path.display(), e));
		let layer: FixtureConfigLayer = toml::from_str(&content)
			.unwrap_or_else(|e| panic!("Failed to parse {}: {}", config_path.display(), e));
		config.merge_layer(layer);
	}

	config.args_for_command("export")
}

/// Helper function to get absolute path to test_fixtures
fn fixtures_path(subpath: &str) -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.unwrap()
		.parent()
		.unwrap()
		.join("test_fixtures")
		.join(subpath)
}

fn copy_golden_without_manifest(src: &Path, dst: &Path) {
	for entry in walkdir::WalkDir::new(src) {
		let entry = entry.expect("walkdir entry");
		let path = entry.path();
		if !entry.file_type().is_file() {
			continue;
		}
		let rel_path = path
			.strip_prefix(src)
			.expect("strip prefix")
			.to_string_lossy()
			.to_string();
		if rel_path == "manifest.json" {
			continue;
		}
		let target = dst.join(&rel_path);
		if let Some(parent) = target.parent() {
			fs::create_dir_all(parent).expect("create parent");
		}
		fs::copy(path, &target).expect("copy file");
	}
}

/// Discover all golden test environments in test_fixtures/golden_envs/
/// Returns a list of (env_name, env_path) tuples
fn discover_golden_envs() -> Vec<(String, PathBuf)> {
	let golden_envs_dir = fixtures_path("golden_envs");
	let mut envs = Vec::new();

	if !golden_envs_dir.exists() {
		return envs;
	}

	for entry in fs::read_dir(&golden_envs_dir).unwrap() {
		let entry = entry.unwrap();
		let path = entry.path();
		if path.is_dir() {
			let golden_dir = path.join("golden");
			// Only include directories that have a golden/ subdirectory
			if golden_dir.exists() && golden_dir.is_dir() {
				let name = path.file_name().unwrap().to_string_lossy().to_string();
				envs.push((name, path));
			}
		}
	}

	// Sort by name for consistent test ordering
	envs.sort_by(|a, b| a.0.cmp(&b.0));
	envs
}

fn run_rtk_export(env_path: &Path, output_dir: &Path, extra_args: &[String]) {
	let mut argv = vec![
		"rtk".to_string(),
		"export".to_string(),
		output_dir.to_string_lossy().to_string(),
		env_path.to_string_lossy().to_string(),
	];
	argv.extend(extra_args.iter().cloned());

	let cli = GoldenCli::try_parse_from(&argv)
		.unwrap_or_else(|e| panic!("failed to parse argv {:?}: {}", argv, e));
	let GoldenCommand::Export(args) = cli.command;
	let mut output = Vec::new();
	commands::export::run(args, &mut output).unwrap_or_else(|e| {
		panic!(
			"rtk export failed for argv {:?}\nstderr:\n{}error:\n{}",
			argv,
			String::from_utf8_lossy(&output),
			e
		)
	});
}

/// Run a golden test comparing rtk export output against tk-generated golden files
fn run_golden_test(env_path: &Path) {
	let temp_dir = tempfile::TempDir::new().unwrap();
	let output_dir = temp_dir.path();

	let golden_dir = env_path.join("golden");

	assert!(
		golden_dir.exists(),
		"Golden directory does not exist at {:?}. Run 'make update-golden-fixtures' to generate it.",
		golden_dir
	);

	let suite_path = env_path.parent().expect("env_path should have a parent");
	let mut extra_args = load_fixture_export_args(suite_path, env_path);
	extra_args.extend([
		"--skip-manifest".to_string(),
		"--parallel".to_string(),
		"1".to_string(),
	]);
	run_rtk_export(env_path, output_dir, &extra_args);

	let filtered_golden_dir = tempfile::TempDir::new().unwrap();
	copy_golden_without_manifest(&golden_dir, filtered_golden_dir.path());
	let comparison = rtk_diff::directory::compare_directories_detailed(
		filtered_golden_dir.path().to_string_lossy().as_ref(),
		output_dir.to_string_lossy().as_ref(),
	)
	.unwrap();

	if !comparison.matched {
		panic!(
			"Content mismatch for {} file(s):\n\n{}",
			comparison.differences.len(),
			comparison.differences.join("\n\n")
		);
	}
}

#[test]
fn test_all_golden_fixtures() {
	let envs = discover_golden_envs();

	assert!(
		!envs.is_empty(),
		"No golden test environments found in test_fixtures/golden_envs/"
	);

	println!("Discovered {} golden test environments:", envs.len());
	for (name, _) in &envs {
		println!("  - {}", name);
	}

	let mut failures = Vec::new();

	for (name, path) in &envs {
		println!("\n=== Testing {} ===", name);
		let result = std::panic::catch_unwind(|| {
			run_golden_test(path);
		});

		match result {
			Ok(()) => println!("✓ {} passed", name),
			Err(e) => {
				let msg = if let Some(s) = e.downcast_ref::<&str>() {
					s.to_string()
				} else if let Some(s) = e.downcast_ref::<String>() {
					s.clone()
				} else {
					"Unknown panic".to_string()
				};
				println!("✗ {} failed", name);
				failures.push((name.clone(), msg));
			}
		}
	}

	if !failures.is_empty() {
		let mut error_msg = format!("\n{} golden fixture test(s) failed:\n", failures.len());
		for (name, msg) in &failures {
			error_msg.push_str(&format!("\n=== {} ===\n{}\n", name, msg));
		}
		panic!("{}", error_msg);
	}
}
