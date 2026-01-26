use std::{
	fs,
	path::{Path, PathBuf},
};

use rtk::{
	discover::find_environments,
	eval::EvalOpts,
	export::{export, ExportOpts},
};
use similar::{ChangeTag, TextDiff};

/// The export format for golden fixtures - matches GOLDEN_EXPORT_FORMAT in Makefile
/// Uses default for namespace to handle cluster-scoped resources (CRDs, ClusterRoles, etc.)
const EXPORT_FORMAT: &str =
	"{{ .metadata.namespace | default \"_cluster\" }}/{{.kind}}-{{.metadata.name}}";

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

/// Recursively collect all files in a directory with their relative paths
fn collect_files(dir: &Path) -> std::collections::HashMap<String, String> {
	let mut files = std::collections::HashMap::new();
	if !dir.exists() {
		return files;
	}
	for entry in walkdir::WalkDir::new(dir) {
		let entry = entry.unwrap();
		if entry.file_type().is_file() {
			let rel_path = entry
				.path()
				.strip_prefix(dir)
				.unwrap()
				.to_string_lossy()
				.to_string();
			let content = fs::read_to_string(entry.path()).unwrap();
			files.insert(rel_path, content);
		}
	}
	files
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

	let envs = find_environments(&[env_path.to_string_lossy().to_string()]).unwrap();
	let env_count = envs.len();
	let recursive = env_count > 1;

	let opts = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "golden".to_string(),
		format: EXPORT_FORMAT.to_string(),
		parallelism: 1,
		eval_opts: EvalOpts::default(),
		name: None,
		recursive,
		skip_manifest: true,
		..Default::default()
	};

	let result = export(&[env_path.to_string_lossy().to_string()], opts).unwrap();

	assert_eq!(
		result.successful, env_count,
		"Should export {} environment(s)",
		env_count
	);
	assert_eq!(result.failed, 0, "Should have no failures");

	let golden_files: std::collections::HashMap<_, _> = collect_files(&golden_dir)
		.into_iter()
		.filter(|(k, _)| k != "manifest.json")
		.collect();
	let output_files = collect_files(output_dir);

	let golden_keys: std::collections::HashSet<_> = golden_files.keys().collect();
	let output_keys: std::collections::HashSet<_> = output_files.keys().collect();

	assert_eq!(
		golden_keys, output_keys,
		"File sets should match.\nGolden: {:?}\nOutput: {:?}",
		golden_keys, output_keys
	);

	let mut all_failures = Vec::new();
	let mut sorted_paths: Vec<_> = golden_files.keys().collect();
	sorted_paths.sort();

	for path in sorted_paths {
		let golden_content = golden_files.get(path).unwrap();
		let output_content = output_files.get(path).unwrap();
		if golden_content != output_content {
			let diff = TextDiff::from_lines(golden_content, output_content);
			let mut diff_output = String::new();
			// Only show changed lines with line numbers
			for (idx, group) in diff.grouped_ops(3).iter().enumerate() {
				if idx > 0 {
					diff_output.push_str("...\n");
				}
				for op in group {
					for change in diff.iter_changes(op) {
						let (sign, line_num) = match change.tag() {
							ChangeTag::Delete => ("-", change.old_index().map(|i| i + 1)),
							ChangeTag::Insert => ("+", change.new_index().map(|i| i + 1)),
							ChangeTag::Equal => continue, // Skip unchanged lines
						};
						let line_str = line_num.map(|n| format!("{:>5}", n)).unwrap_or_default();
						diff_output.push_str(&format!("{} {}| {}", sign, line_str, change));
					}
				}
			}
			all_failures.push(format!(
				"=== {} ===\n--- golden (expected)\n+++ output (actual)\n\n{}",
				path, diff_output
			));
		}
	}

	if !all_failures.is_empty() {
		panic!(
			"Content mismatch for {} file(s):\n\n{}",
			all_failures.len(),
			all_failures.join("\n\n")
		);
	}
}

/// Main test that discovers and runs all golden fixture tests
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
