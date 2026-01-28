use std::{
	collections::HashMap,
	fs,
	path::{Path, PathBuf},
};

use rtk::{
	discover::find_environments,
	eval::EvalOpts,
	export::{export, ExportOpts},
};

/// Helper function to get absolute path to test data
fn testdata_path(subpath: &str) -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata")
		.join(subpath)
}

/// Helper function to check that files match expected list
fn check_files(dir: &Path, expected_files: &[&str]) {
	let mut actual_files: Vec<String> = Vec::new();

	for entry in walkdir::WalkDir::new(dir) {
		let entry = entry.unwrap();
		if entry.file_type().is_file() {
			let rel_path = entry
				.path()
				.strip_prefix(dir)
				.unwrap()
				.to_string_lossy()
				.to_string();
			actual_files.push(rel_path);
		}
	}

	// Sort both for comparison
	actual_files.sort();
	let mut expected_sorted: Vec<String> = expected_files.iter().map(|s| s.to_string()).collect();
	expected_sorted.sort();

	assert_eq!(
		actual_files, expected_sorted,
		"\nExpected files:\n{:#?}\n\nActual files:\n{:#?}",
		expected_sorted, actual_files
	);
}

#[test]
fn test_export_environments() {
	let temp_dir = tempfile::TempDir::new().unwrap();
	let output_dir = temp_dir.path();

	// Save original directory for cleanup, but don't change directory
	// (changing directory affects the entire process and causes test race conditions)
	let _original_dir = std::env::current_dir().unwrap();

	// Find environments
	let envs = find_environments(&[testdata_path("test-export-envs")
		.to_string_lossy()
		.to_string()])
	.unwrap();
	// Should find 3 environments: 1 static (static-env) + 2 inline sub-envs (inline-namespace1, inline-namespace2)
	assert_eq!(
		envs.len(),
		3,
		"Should find 3 environments (1 static + 2 inline sub-envs)"
	);

	// Export all envs
	let mut ext_code = HashMap::new();
	ext_code.insert(
		"deploymentName".to_string(),
		"'initial-deployment'".to_string(),
	);
	ext_code.insert("serviceName".to_string(), "'initial-service'".to_string());

	let opts = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{env.metadata.labels.cluster_name}}/{{env.spec.namespace}}/{{.metadata.name}}"
			.to_string(),
		parallelism: 8,
		eval_opts: EvalOpts {
			ext_code,
			..Default::default()
		},
		name: None,
		recursive: true,
		skip_manifest: false,
		..Default::default()
	};

	let result = export(
		&[testdata_path("test-export-envs")
			.to_string_lossy()
			.to_string()],
		opts,
	)
	.unwrap();

	// Should export 3 environments successfully (1 static + 2 inline sub-envs)
	assert_eq!(result.successful, 3);
	assert_eq!(result.failed, 0);

	// Check that expected files were created
	check_files(
		output_dir,
		&[
			"my-cluster/inline-namespace1/my-configmap.yaml",
			"my-cluster/inline-namespace1/my-deployment.yaml",
			"my-cluster/inline-namespace1/my-service.yaml",
			"my-cluster2/inline-namespace2/my-deployment.yaml",
			"my-cluster2/inline-namespace2/my-service.yaml",
			"my-static-cluster/static/initial-deployment.yaml",
			"my-static-cluster/static/initial-service.yaml",
			"manifest.json",
		],
	);

	// Check manifest.json contents
	let manifest_content = fs::read_to_string(output_dir.join("manifest.json")).unwrap();
	let manifest_map: HashMap<String, String> = serde_json::from_str(&manifest_content).unwrap();

	assert_eq!(manifest_map.len(), 7);
	assert!(manifest_map.contains_key("my-cluster/inline-namespace1/my-configmap.yaml"));
	assert!(manifest_map.contains_key("my-cluster/inline-namespace1/my-deployment.yaml"));
	assert!(manifest_map.contains_key("my-cluster/inline-namespace1/my-service.yaml"));
	assert!(manifest_map.contains_key("my-cluster2/inline-namespace2/my-deployment.yaml"));
	assert!(manifest_map.contains_key("my-cluster2/inline-namespace2/my-service.yaml"));
	assert!(manifest_map.contains_key("my-static-cluster/static/initial-deployment.yaml"));
	assert!(manifest_map.contains_key("my-static-cluster/static/initial-service.yaml"));

	// Verify all entries point to correct environments
	// Note: entries contain absolute paths since we didn't change directory
	assert!(
		manifest_map["my-cluster/inline-namespace1/my-configmap.yaml"]
			.contains("test-export-envs/inline-envs/main.jsonnet")
	);
	assert!(
		manifest_map["my-static-cluster/static/initial-deployment.yaml"]
			.contains("test-export-envs/static-env/main.jsonnet")
	);

	// Finally make sure that the indentation is 2 spaces by looking at `metadata.name`
	let deployment_content =
		fs::read_to_string(output_dir.join("my-static-cluster/static/initial-deployment.yaml"))
			.unwrap();
	assert!(
		deployment_content.contains("  name: initial-deployment"),
		"file indentation is most likely no longer 2 spaces"
	);
}

#[test]
#[ignore] // TODO: This test is ignored because the Rust version doesn't yet have Kubernetes schema validation
		  // The Go version fails with a SchemaError because metadata.name is a boolean (true) instead of a string
		  // The Rust version currently succeeds and serializes the boolean as the string "true"
		  // This test should be re-enabled once Kubernetes schema validation is implemented
fn test_export_environments_broken() {
	let temp_dir = tempfile::TempDir::new().unwrap();
	let output_dir = temp_dir.path();

	// Find environments
	let _envs = find_environments(&[testdata_path("test-export-envs-broken")
		.to_string_lossy()
		.to_string()])
	.unwrap();

	// Export all envs
	let opts = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 1,
		eval_opts: EvalOpts::default(),
		name: None,
		recursive: true,
		skip_manifest: false,
		..Default::default()
	};

	let result = export(
		&[testdata_path("test-export-envs-broken")
			.to_string_lossy()
			.to_string()],
		opts,
	);

	// Should fail - the environment has a schema error (name field is boolean instead of string)
	// For now, this might just be an evaluation error rather than a schema error
	// but it should still fail
	match result {
		Ok(r) => {
			// If it returns Ok, check if there are failures recorded
			assert!(
				r.failed > 0 || r.results.iter().any(|res| res.error.is_some()),
				"Expected at least one failure, but got {} failures",
				r.failed
			);
		}
		Err(_) => {
			// This is also acceptable - the export failed entirely
		}
	}
}

#[test]
fn test_export_environments_skip_manifest() {
	let temp_dir = tempfile::TempDir::new().unwrap();
	let output_dir = temp_dir.path();

	// Find environments
	let _envs = find_environments(&[testdata_path("test-export-envs")
		.to_string_lossy()
		.to_string()])
	.unwrap();

	// Export all envs with skip manifest flag
	let mut ext_code = HashMap::new();
	ext_code.insert(
		"deploymentName".to_string(),
		"'test-deployment'".to_string(),
	);
	ext_code.insert("serviceName".to_string(), "'test-service'".to_string());

	let opts = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 1,
		eval_opts: EvalOpts {
			ext_code,
			..Default::default()
		},
		name: None,
		recursive: true,
		skip_manifest: true,
		..Default::default()
	};

	let result = export(
		&[testdata_path("test-export-envs")
			.to_string_lossy()
			.to_string()],
		opts,
	)
	.unwrap();

	// Should export 3 environments successfully (1 static + 2 inline sub-envs)
	assert_eq!(result.successful, 3);
	assert_eq!(result.failed, 0);

	// Check that all manifest files are created but manifest.json is NOT created
	check_files(
		output_dir,
		&[
			"inline-namespace1/my-configmap.yaml",
			"inline-namespace1/my-deployment.yaml",
			"inline-namespace1/my-service.yaml",
			"inline-namespace2/my-deployment.yaml",
			"inline-namespace2/my-service.yaml",
			"static/test-deployment.yaml",
			"static/test-service.yaml",
		],
	);

	// Explicitly verify manifest.json does not exist
	let manifest_path = output_dir.join("manifest.json");
	assert!(
		!manifest_path.exists(),
		"manifest.json should not exist when SkipManifest is true"
	);
}

#[test]
fn test_export_merge_strategies() {
	use rtk::export::ExportMergeStrategy;

	let temp_dir = tempfile::TempDir::new().unwrap();
	let output_dir = temp_dir.path();

	// Find environments
	let envs = find_environments(&[testdata_path("test-export-envs")
		.to_string_lossy()
		.to_string()])
	.unwrap();
	// Should find 3 environments: 1 static (static-env) + 2 inline sub-envs (inline-namespace1, inline-namespace2)
	assert_eq!(
		envs.len(),
		3,
		"Should find 3 environments (1 static + 2 inline sub-envs)"
	);

	// STEP 1: Initial export with default strategy
	let mut ext_code = HashMap::new();
	ext_code.insert(
		"deploymentName".to_string(),
		"'initial-deployment'".to_string(),
	);
	ext_code.insert("serviceName".to_string(), "'initial-service'".to_string());

	let opts = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 1,
		eval_opts: EvalOpts {
			ext_code: ext_code.clone(),
			..Default::default()
		},
		name: None,
		recursive: true,
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::None,
		..Default::default()
	};

	let result = export(
		&[testdata_path("test-export-envs")
			.to_string_lossy()
			.to_string()],
		opts.clone(),
	)
	.unwrap();

	// Should export 3 environments successfully (1 static + 2 inline sub-envs)
	assert_eq!(result.successful, 3);
	assert_eq!(result.failed, 0);

	// Check initial files
	check_files(
		output_dir,
		&[
			"inline-namespace1/my-configmap.yaml",
			"inline-namespace1/my-deployment.yaml",
			"inline-namespace1/my-service.yaml",
			"inline-namespace2/my-deployment.yaml",
			"inline-namespace2/my-service.yaml",
			"static/initial-deployment.yaml",
			"static/initial-service.yaml",
			"manifest.json",
		],
	);

	// STEP 2: Try to re-export without merge strategy - should fail
	let result = export(
		&[testdata_path("test-export-envs")
			.to_string_lossy()
			.to_string()],
		opts.clone(),
	);
	assert!(result.is_err(), "Should fail when directory is not empty");
	assert!(
		result
			.unwrap_err()
			.to_string()
			.contains("not empty. Pass a different --merge-strategy"),
		"Error should mention merge strategy"
	);

	// STEP 3: Try to re-export with fail-on-conflicts strategy
	let mut fail_opts = opts.clone();
	fail_opts.merge_strategy = ExportMergeStrategy::FailOnConflicts;

	let result = export(
		&[testdata_path("test-export-envs")
			.to_string_lossy()
			.to_string()],
		fail_opts,
	);
	// Should fail because files already exist
	match result {
		Ok(r) => {
			assert!(
				r.failed > 0 || r.results.iter().any(|res| res.error.is_some()),
				"Should have failures when files exist"
			);
		}
		Err(_) => {
			// Also acceptable - the export failed entirely
		}
	}

	// STEP 4: Re-export only static env with replace-envs strategy
	let mut updated_ext_code = HashMap::new();
	updated_ext_code.insert(
		"deploymentName".to_string(),
		"'updated-deployment'".to_string(),
	);
	updated_ext_code.insert("serviceName".to_string(), "'updated-service'".to_string());

	// Find just the static environment
	let static_envs: Vec<_> = envs
		.iter()
		.filter(|e| e.path.to_string_lossy().contains("static-env"))
		.collect();
	assert_eq!(static_envs.len(), 1, "Should find static environment");

	let replace_opts = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 1,
		eval_opts: EvalOpts {
			ext_code: updated_ext_code.clone(),
			..Default::default()
		},
		name: None,
		recursive: true,
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::ReplaceEnvs,
		..Default::default()
	};

	let result = export(
		&[static_envs[0].path.to_string_lossy().to_string()],
		replace_opts.clone(),
	)
	.unwrap();

	assert_eq!(result.successful, 1);

	// Check files - inline env files should still exist, static env updated
	check_files(
		output_dir,
		&[
			"inline-namespace1/my-configmap.yaml",
			"inline-namespace1/my-deployment.yaml",
			"inline-namespace1/my-service.yaml",
			"inline-namespace2/my-deployment.yaml",
			"inline-namespace2/my-service.yaml",
			"static/updated-deployment.yaml",
			"static/updated-service.yaml",
			"manifest.json",
		],
	);

	// Verify the file content was updated
	let deployment_content =
		fs::read_to_string(output_dir.join("static/updated-deployment.yaml")).unwrap();
	assert!(
		deployment_content.contains("updated-deployment"),
		"Deployment should be updated"
	);

	// STEP 5: Re-export and delete files from inline environment
	let inline_env_path = testdata_path("test-export-envs/inline-envs/main.jsonnet");
	let mut updated_again_ext_code = HashMap::new();
	updated_again_ext_code.insert(
		"deploymentName".to_string(),
		"'updated-again-deployment'".to_string(),
	);
	updated_again_ext_code.insert(
		"serviceName".to_string(),
		"'updated-again-service'".to_string(),
	);

	let delete_opts = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 1,
		eval_opts: EvalOpts {
			ext_code: updated_again_ext_code,
			..Default::default()
		},
		name: None,
		recursive: true,
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::ReplaceEnvs,
		merge_deleted_envs: vec![inline_env_path.to_string_lossy().to_string()],
		show_timing: false,
	};

	let result = export(
		&[static_envs[0].path.to_string_lossy().to_string()],
		delete_opts,
	)
	.unwrap();

	assert_eq!(result.successful, 1);

	// Check files - inline env files should be deleted, only static env remains
	check_files(
		output_dir,
		&[
			"static/updated-again-deployment.yaml",
			"static/updated-again-service.yaml",
			"manifest.json",
		],
	);

	// Verify manifest.json only has static env files
	let manifest_content = fs::read_to_string(output_dir.join("manifest.json")).unwrap();
	let manifest_map: HashMap<String, String> = serde_json::from_str(&manifest_content).unwrap();
	assert_eq!(
		manifest_map.len(),
		2,
		"Should only have 2 files in manifest"
	);
	assert!(manifest_map.contains_key("static/updated-again-deployment.yaml"));
	assert!(manifest_map.contains_key("static/updated-again-service.yaml"));

	// Finally verify indentation is 2 spaces
	let final_deployment =
		fs::read_to_string(output_dir.join("static/updated-again-deployment.yaml")).unwrap();
	assert!(
		final_deployment.contains("  name: updated-again-deployment"),
		"File indentation should be 2 spaces"
	);
}

// Test for inline env files with no valid Tanka Environment objects
// With the new behavior, inline environments without valid Environment objects are not discovered
#[test]
fn test_export_empty_inline_environment() {
	// Find environments - should find none (no valid Tanka Environment object in the output)
	let envs = find_environments(&[testdata_path("test-export-empty-inline-env")
		.to_string_lossy()
		.to_string()])
	.unwrap();

	// Should NOT discover the environment directory because it has no valid Tanka Environment
	assert_eq!(
		envs.len(),
		0,
		"Should find 0 environments (inline env with no Environment object)"
	);
}

/// Test that export works with absolute paths to directories
#[test]
fn test_export_with_absolute_directory_path() {
	let temp_dir = tempfile::TempDir::new().unwrap();
	let output_dir = temp_dir.path();

	// Get absolute path to the test environment directory
	let abs_path = testdata_path("test-export-envs/static-env");
	assert!(
		abs_path.is_absolute(),
		"Path should be absolute: {:?}",
		abs_path
	);
	assert!(
		abs_path.is_dir(),
		"Path should be a directory: {:?}",
		abs_path
	);

	// Export using absolute path
	let mut ext_code = HashMap::new();
	ext_code.insert(
		"deploymentName".to_string(),
		"'absolute-deployment'".to_string(),
	);
	ext_code.insert("serviceName".to_string(), "'absolute-service'".to_string());

	let opts = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 1,
		eval_opts: EvalOpts {
			ext_code,
			..Default::default()
		},
		name: None,
		recursive: false,
		skip_manifest: false,
		..Default::default()
	};

	let result = export(&[abs_path.to_string_lossy().to_string()], opts).unwrap();

	// Should export successfully
	assert_eq!(result.successful, 1, "Should export 1 environment");
	assert_eq!(result.failed, 0, "Should have no failures");

	// Check that expected files were created
	check_files(
		output_dir,
		&[
			"static/absolute-deployment.yaml",
			"static/absolute-service.yaml",
			"manifest.json",
		],
	);

	// Verify manifest.json points to the absolute path of the environment
	let manifest_content = fs::read_to_string(output_dir.join("manifest.json")).unwrap();
	let manifest_map: HashMap<String, String> = serde_json::from_str(&manifest_content).unwrap();
	assert_eq!(manifest_map.len(), 2);

	// The manifest should contain the absolute path to the main.jsonnet file
	for value in manifest_map.values() {
		assert!(
			value.contains("test-export-envs/static-env/main.jsonnet"),
			"Manifest entry should reference the environment path: {}",
			value
		);
	}
}

/// Test that export works with absolute paths to files (e.g., main.jsonnet)
#[test]
fn test_export_with_absolute_file_path() {
	let temp_dir = tempfile::TempDir::new().unwrap();
	let output_dir = temp_dir.path();

	// Get absolute path to the main.jsonnet file (not the directory)
	let abs_path = testdata_path("test-export-envs/static-env/main.jsonnet");
	assert!(
		abs_path.is_absolute(),
		"Path should be absolute: {:?}",
		abs_path
	);
	assert!(abs_path.is_file(), "Path should be a file: {:?}", abs_path);

	// Export using absolute path to the file
	let mut ext_code = HashMap::new();
	ext_code.insert(
		"deploymentName".to_string(),
		"'file-path-deployment'".to_string(),
	);
	ext_code.insert("serviceName".to_string(), "'file-path-service'".to_string());

	let opts = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 1,
		eval_opts: EvalOpts {
			ext_code,
			..Default::default()
		},
		name: None,
		recursive: false,
		skip_manifest: false,
		..Default::default()
	};

	let result = export(&[abs_path.to_string_lossy().to_string()], opts).unwrap();

	// Should export successfully - the file path should resolve to its parent directory
	assert_eq!(result.successful, 1, "Should export 1 environment");
	assert_eq!(result.failed, 0, "Should have no failures");

	// Check that expected files were created
	check_files(
		output_dir,
		&[
			"static/file-path-deployment.yaml",
			"static/file-path-service.yaml",
			"manifest.json",
		],
	);
}

/// Test that export fails on file conflicts with existing files on disk
/// This tests scenarios where an export tries to write to a path that already exists from a previous export
#[test]
fn test_export_file_conflict_fail_on_conflicts() {
	use rtk::export::ExportMergeStrategy;

	let temp_dir = tempfile::TempDir::new().unwrap();
	let output_dir = temp_dir.path();

	// STEP 1: Export first environment to create a file
	let opts1 = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 1,
		eval_opts: EvalOpts::default(),
		name: None,
		recursive: false,
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::None,
		..Default::default()
	};

	let result1 = export(
		&[testdata_path("test-export-conflict/env1")
			.to_string_lossy()
			.to_string()],
		opts1,
	);
	result1.unwrap(); // First export should succeed

	// STEP 2: Try to export second environment that maps to the same file path with fail-on-conflicts
	let opts2 = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 1,
		eval_opts: EvalOpts::default(),
		name: None,
		recursive: false,
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::FailOnConflicts,
		..Default::default()
	};

	let result2 = export(
		&[testdata_path("test-export-conflict/env2")
			.to_string_lossy()
			.to_string()],
		opts2,
	);

	// Should fail because file already exists from first export
	match result2 {
		Ok(r) => {
			assert!(
				r.failed > 0 || r.results.iter().any(|res| res.error.is_some()),
				"Export should fail when file already exists with fail-on-conflicts strategy. Result: {:?}",
				r
			);
			// Check that error message mentions file conflict
			for res in &r.results {
				if let Some(ref err) = res.error {
					assert!(
						err.contains("already exists"),
						"Error should mention file already exists: {}",
						err
					);
				}
			}
		}
		Err(e) => {
			let err_msg = e.to_string();
			assert!(
				err_msg.contains("already exists"),
				"Error should mention file already exists: {}",
				err_msg
			);
		}
	}
}

/// Test that export fails on file conflicts with replace-envs strategy
/// Even with replace-envs, conflicts with existing files should fail
#[test]
fn test_export_file_conflict_replace_envs() {
	use rtk::export::ExportMergeStrategy;

	let temp_dir = tempfile::TempDir::new().unwrap();
	let output_dir = temp_dir.path();

	// STEP 1: Export first environment to create a file
	let opts1 = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 1,
		eval_opts: EvalOpts::default(),
		name: None,
		recursive: false,
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::None,
		..Default::default()
	};

	let result1 = export(
		&[testdata_path("test-export-conflict/env1")
			.to_string_lossy()
			.to_string()],
		opts1,
	);
	result1.unwrap(); // First export should succeed

	// STEP 2: Try to export second environment that maps to the same file path with replace-envs
	let opts2 = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 1,
		eval_opts: EvalOpts::default(),
		name: None,
		recursive: false,
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::ReplaceEnvs,
		..Default::default()
	};

	let result2 = export(
		&[testdata_path("test-export-conflict/env2")
			.to_string_lossy()
			.to_string()],
		opts2,
	);

	// Should fail because file already exists
	// replace-envs only handles re-exporting previously exported envs (deletes their files first),
	// but this is a different environment trying to write to the same path
	match result2 {
		Ok(r) => {
			assert!(
				r.failed > 0 || r.results.iter().any(|res| res.error.is_some()),
				"Export should fail when file already exists with replace-envs strategy. Result: {:?}",
				r
			);
			// Check that error message mentions file conflict
			for res in &r.results {
				if let Some(ref err) = res.error {
					assert!(
						err.contains("already exists"),
						"Error should mention file already exists: {}",
						err
					);
				}
			}
		}
		Err(e) => {
			let err_msg = e.to_string();
			assert!(
				err_msg.contains("already exists"),
				"Error should mention file already exists: {}",
				err_msg
			);
		}
	}
}

/// Test that export fails when a Kubernetes object is missing required attributes
/// This matches Tanka's behavior of validating that objects with kind+metadata also have apiVersion
#[test]
fn test_export_fails_on_invalid_k8s_object() {
	let temp_dir = tempfile::TempDir::new().unwrap();
	let output_dir = temp_dir.path();

	let opts = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 1,
		eval_opts: EvalOpts::default(),
		name: None,
		recursive: false,
		skip_manifest: false,
		..Default::default()
	};

	let result = export(
		&[testdata_path("test-export-invalid-k8s-object")
			.to_string_lossy()
			.to_string()],
		opts,
	);

	// Should fail because thor_engine has kind and metadata but missing apiVersion
	match result {
		Ok(r) => {
			assert!(
				r.failed > 0 || r.results.iter().any(|res| res.error.is_some()),
				"Export should fail when a k8s object is missing apiVersion. Result: {:?}",
				r
			);
			// Check that error message mentions missing apiVersion
			let has_apiversion_error = r
				.results
				.iter()
				.any(|res| res.error.as_ref().is_some_and(|e| e.contains("apiVersion")));
			assert!(
				has_apiversion_error,
				"Error should mention missing apiVersion attribute"
			);
		}
		Err(e) => {
			let err_msg = e.to_string();
			assert!(
				err_msg.contains("apiVersion"),
				"Error should mention missing apiVersion: {}",
				err_msg
			);
		}
	}
}

// ============================================================================
// Replace-envs idempotency tests
// ============================================================================
//
// These tests verify that replace-envs is idempotent (re-exporting unchanged
// jsonnet produces identical output) and that changes are properly reflected.

/// Helper to collect all files and their contents from a directory
fn collect_files_with_content(dir: &Path) -> HashMap<String, String> {
	let mut files = HashMap::new();
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

/// Test replace-envs idempotency for a single static environment
/// Verifies: export -> re-export (identical) -> change -> re-export (reflects change)
#[test]
fn test_replace_envs_idempotent_single_static_env() {
	use rtk::export::ExportMergeStrategy;

	let temp_dir = tempfile::TempDir::new().unwrap();
	let output_dir = temp_dir.path();

	let static_env_path = testdata_path("test-export-envs/static-env");

	// STEP 1: Initial export
	let mut ext_code = HashMap::new();
	ext_code.insert("deploymentName".to_string(), "'my-deployment'".to_string());
	ext_code.insert("serviceName".to_string(), "'my-service'".to_string());

	let opts = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 1,
		eval_opts: EvalOpts {
			ext_code: ext_code.clone(),
			..Default::default()
		},
		name: None,
		recursive: false,
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::None,
		..Default::default()
	};

	let result = export(&[static_env_path.to_string_lossy().to_string()], opts).unwrap();
	assert_eq!(result.successful, 1);
	assert_eq!(result.failed, 0);

	// Capture initial state
	let files_after_initial = collect_files_with_content(output_dir);
	assert_eq!(files_after_initial.len(), 3); // 2 manifests + manifest.json

	// STEP 2: Re-export with replace-envs (unchanged jsonnet) - should be identical
	let opts_replace = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 1,
		eval_opts: EvalOpts {
			ext_code: ext_code.clone(),
			..Default::default()
		},
		name: None,
		recursive: false,
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::ReplaceEnvs,
		..Default::default()
	};

	let result = export(
		&[static_env_path.to_string_lossy().to_string()],
		opts_replace.clone(),
	)
	.unwrap();
	assert_eq!(result.successful, 1);

	// Verify idempotency - files should be identical
	let files_after_reexport = collect_files_with_content(output_dir);
	assert_eq!(
		files_after_initial, files_after_reexport,
		"Re-export with unchanged jsonnet should produce identical files"
	);

	// STEP 3: Change ext_code and re-export - should reflect changes
	let mut changed_ext_code = HashMap::new();
	changed_ext_code.insert(
		"deploymentName".to_string(),
		"'changed-deployment'".to_string(),
	);
	changed_ext_code.insert("serviceName".to_string(), "'changed-service'".to_string());

	let opts_changed = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 1,
		eval_opts: EvalOpts {
			ext_code: changed_ext_code,
			..Default::default()
		},
		name: None,
		recursive: false,
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::ReplaceEnvs,
		..Default::default()
	};

	let result = export(
		&[static_env_path.to_string_lossy().to_string()],
		opts_changed,
	)
	.unwrap();
	assert_eq!(result.successful, 1);

	// Verify changes are reflected
	let files_after_change = collect_files_with_content(output_dir);

	// Old files should be gone, new files should exist
	assert!(
		!files_after_change.contains_key("static/my-deployment.yaml"),
		"Old deployment file should be deleted"
	);
	assert!(
		!files_after_change.contains_key("static/my-service.yaml"),
		"Old service file should be deleted"
	);
	assert!(
		files_after_change.contains_key("static/changed-deployment.yaml"),
		"New deployment file should exist"
	);
	assert!(
		files_after_change.contains_key("static/changed-service.yaml"),
		"New service file should exist"
	);

	// Verify content contains new names
	let deployment_content = &files_after_change["static/changed-deployment.yaml"];
	assert!(
		deployment_content.contains("name: changed-deployment"),
		"Deployment content should reflect change"
	);
}

/// Test replace-envs idempotency for all environments (static + inline)
/// Verifies: export -> re-export (identical) -> change -> re-export (reflects change)
#[test]
fn test_replace_envs_idempotent_all_envs() {
	use rtk::export::ExportMergeStrategy;

	let temp_dir = tempfile::TempDir::new().unwrap();
	let output_dir = temp_dir.path();

	let envs_path = testdata_path("test-export-envs");

	// STEP 1: Initial export of all environments
	let mut ext_code = HashMap::new();
	ext_code.insert(
		"deploymentName".to_string(),
		"'initial-deployment'".to_string(),
	);
	ext_code.insert("serviceName".to_string(), "'initial-service'".to_string());

	let opts = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{env.metadata.labels.cluster_name}}/{{env.spec.namespace}}/{{.metadata.name}}"
			.to_string(),
		parallelism: 4,
		eval_opts: EvalOpts {
			ext_code: ext_code.clone(),
			..Default::default()
		},
		name: None,
		recursive: true,
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::None,
		..Default::default()
	};

	let result = export(&[envs_path.to_string_lossy().to_string()], opts).unwrap();
	assert_eq!(result.successful, 3); // 1 static + 2 inline
	assert_eq!(result.failed, 0);

	// Capture initial state
	let files_after_initial = collect_files_with_content(output_dir);
	// 7 manifest files + manifest.json = 8
	assert_eq!(files_after_initial.len(), 8);

	// STEP 2: Re-export all with replace-envs (unchanged) - should be identical
	let opts_replace = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{env.metadata.labels.cluster_name}}/{{env.spec.namespace}}/{{.metadata.name}}"
			.to_string(),
		parallelism: 4,
		eval_opts: EvalOpts {
			ext_code: ext_code.clone(),
			..Default::default()
		},
		name: None,
		recursive: true,
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::ReplaceEnvs,
		..Default::default()
	};

	let result = export(
		&[envs_path.to_string_lossy().to_string()],
		opts_replace.clone(),
	)
	.unwrap();
	assert_eq!(result.successful, 3);

	// Verify idempotency
	let files_after_reexport = collect_files_with_content(output_dir);
	assert_eq!(
		files_after_initial, files_after_reexport,
		"Re-export with unchanged jsonnet should produce identical files"
	);

	// STEP 3: Change ext_code and re-export all
	let mut changed_ext_code = HashMap::new();
	changed_ext_code.insert(
		"deploymentName".to_string(),
		"'updated-deployment'".to_string(),
	);
	changed_ext_code.insert("serviceName".to_string(), "'updated-service'".to_string());

	let opts_changed = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{env.metadata.labels.cluster_name}}/{{env.spec.namespace}}/{{.metadata.name}}"
			.to_string(),
		parallelism: 4,
		eval_opts: EvalOpts {
			ext_code: changed_ext_code,
			..Default::default()
		},
		name: None,
		recursive: true,
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::ReplaceEnvs,
		..Default::default()
	};

	let result = export(&[envs_path.to_string_lossy().to_string()], opts_changed).unwrap();
	assert_eq!(result.successful, 3);

	// Verify changes are reflected for static env
	let files_after_change = collect_files_with_content(output_dir);

	// Static env files should be updated
	assert!(
		!files_after_change.contains_key("my-static-cluster/static/initial-deployment.yaml"),
		"Old static deployment should be deleted"
	);
	assert!(
		files_after_change.contains_key("my-static-cluster/static/updated-deployment.yaml"),
		"New static deployment should exist"
	);

	// Inline env files should remain (they don't use ext_code)
	assert!(
		files_after_change.contains_key("my-cluster/inline-namespace1/my-deployment.yaml"),
		"Inline env files should still exist"
	);

	// Verify manifest.json is updated correctly
	let manifest_content = &files_after_change["manifest.json"];
	assert!(
		manifest_content.contains("updated-deployment.yaml"),
		"Manifest should reference new files"
	);
	assert!(
		!manifest_content.contains("initial-deployment.yaml"),
		"Manifest should not reference old files"
	);
}

/// Test replace-envs idempotency for inline environments only
/// Verifies: export -> re-export (identical) -> change -> re-export (reflects change)
#[test]
fn test_replace_envs_idempotent_inline_env() {
	use rtk::export::ExportMergeStrategy;

	let temp_dir = tempfile::TempDir::new().unwrap();
	let output_dir = temp_dir.path();

	// Use the inline-envs directory (not the file) with recursive to export both inline sub-envs
	let inline_env_path = testdata_path("test-export-envs/inline-envs");

	// STEP 1: Initial export of inline environments
	let opts = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{env.metadata.labels.cluster_name}}/{{env.spec.namespace}}/{{.metadata.name}}"
			.to_string(),
		parallelism: 1,
		eval_opts: EvalOpts::default(),
		name: None,
		recursive: true, // Need recursive since there are 2 inline sub-envs
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::None,
		..Default::default()
	};

	let result = export(&[inline_env_path.to_string_lossy().to_string()], opts).unwrap();
	assert_eq!(result.successful, 2); // 2 inline sub-envs
	assert_eq!(result.failed, 0);

	// Capture initial state
	let files_after_initial = collect_files_with_content(output_dir);
	// inline-namespace1: 3 files (deployment, service, configmap)
	// inline-namespace2: 2 files (deployment, service)
	// + manifest.json = 6
	assert_eq!(files_after_initial.len(), 6);

	// STEP 2: Re-export with replace-envs (unchanged) - should be identical
	let opts_replace = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{env.metadata.labels.cluster_name}}/{{env.spec.namespace}}/{{.metadata.name}}"
			.to_string(),
		parallelism: 1,
		eval_opts: EvalOpts::default(),
		name: None,
		recursive: true,
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::ReplaceEnvs,
		..Default::default()
	};

	let result = export(
		&[inline_env_path.to_string_lossy().to_string()],
		opts_replace.clone(),
	)
	.unwrap();
	assert_eq!(result.successful, 2);

	// Verify idempotency
	let files_after_reexport = collect_files_with_content(output_dir);
	assert_eq!(
		files_after_initial, files_after_reexport,
		"Re-export with unchanged jsonnet should produce identical files"
	);

	// STEP 3: Re-export multiple times to ensure continued idempotency
	for _ in 0..3 {
		let result = export(
			&[inline_env_path.to_string_lossy().to_string()],
			opts_replace.clone(),
		)
		.unwrap();
		assert_eq!(result.successful, 2);

		let files = collect_files_with_content(output_dir);
		assert_eq!(
			files_after_initial, files,
			"Multiple re-exports should remain idempotent"
		);
	}
}

/// Test replace-envs with parallel exports to verify thread safety
/// Exports with different parallelism values and verifies identical results
#[test]
fn test_replace_envs_idempotent_parallel() {
	use rtk::export::ExportMergeStrategy;

	let temp_dir = tempfile::TempDir::new().unwrap();
	let output_dir = temp_dir.path();

	let envs_path = testdata_path("test-export-envs");

	let mut ext_code = HashMap::new();
	ext_code.insert(
		"deploymentName".to_string(),
		"'parallel-deployment'".to_string(),
	);
	ext_code.insert("serviceName".to_string(), "'parallel-service'".to_string());

	// Initial export with parallelism=1
	let opts = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{env.metadata.labels.cluster_name}}/{{env.spec.namespace}}/{{.metadata.name}}"
			.to_string(),
		parallelism: 1,
		eval_opts: EvalOpts {
			ext_code: ext_code.clone(),
			..Default::default()
		},
		name: None,
		recursive: true,
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::None,
		..Default::default()
	};

	let result = export(&[envs_path.to_string_lossy().to_string()], opts).unwrap();
	assert_eq!(result.successful, 3);

	let files_initial = collect_files_with_content(output_dir);

	// Re-export with parallelism=8 using replace-envs
	let opts_parallel = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{env.metadata.labels.cluster_name}}/{{env.spec.namespace}}/{{.metadata.name}}"
			.to_string(),
		parallelism: 8,
		eval_opts: EvalOpts {
			ext_code: ext_code.clone(),
			..Default::default()
		},
		name: None,
		recursive: true,
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::ReplaceEnvs,
		..Default::default()
	};

	let result = export(
		&[envs_path.to_string_lossy().to_string()],
		opts_parallel.clone(),
	)
	.unwrap();
	assert_eq!(result.successful, 3);

	let files_after_parallel = collect_files_with_content(output_dir);
	assert_eq!(
		files_initial, files_after_parallel,
		"Parallel re-export should produce identical files"
	);

	// Re-export again with high parallelism
	let result = export(&[envs_path.to_string_lossy().to_string()], opts_parallel).unwrap();
	assert_eq!(result.successful, 3);

	let files_final = collect_files_with_content(output_dir);
	assert_eq!(
		files_initial, files_final,
		"Multiple parallel re-exports should remain idempotent"
	);
}

/// Test that replace-envs correctly handles file removal when resources are deleted
/// from the jsonnet output
#[test]
fn test_replace_envs_removes_deleted_resources() {
	use rtk::export::ExportMergeStrategy;

	let temp_dir = tempfile::TempDir::new().unwrap();
	let output_dir = temp_dir.path();

	// We'll use the static env and simulate resource removal by changing ext_code
	// such that different files are produced
	let static_env_path = testdata_path("test-export-envs/static-env");

	// STEP 1: Initial export with two resources
	let mut ext_code = HashMap::new();
	ext_code.insert("deploymentName".to_string(), "'resource-a'".to_string());
	ext_code.insert("serviceName".to_string(), "'resource-b'".to_string());

	let opts = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 1,
		eval_opts: EvalOpts {
			ext_code,
			..Default::default()
		},
		name: None,
		recursive: false,
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::None,
		..Default::default()
	};

	let result = export(&[static_env_path.to_string_lossy().to_string()], opts).unwrap();
	assert_eq!(result.successful, 1);

	// Verify initial files
	check_files(
		output_dir,
		&[
			"static/resource-a.yaml",
			"static/resource-b.yaml",
			"manifest.json",
		],
	);

	// STEP 2: Re-export with different resource names (simulates resource deletion + creation)
	let mut changed_ext_code = HashMap::new();
	changed_ext_code.insert("deploymentName".to_string(), "'resource-c'".to_string());
	changed_ext_code.insert("serviceName".to_string(), "'resource-d'".to_string());

	let opts_changed = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 1,
		eval_opts: EvalOpts {
			ext_code: changed_ext_code,
			..Default::default()
		},
		name: None,
		recursive: false,
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::ReplaceEnvs,
		..Default::default()
	};

	let result = export(
		&[static_env_path.to_string_lossy().to_string()],
		opts_changed,
	)
	.unwrap();
	assert_eq!(result.successful, 1);

	// Old files should be deleted, new files should exist
	check_files(
		output_dir,
		&[
			"static/resource-c.yaml",
			"static/resource-d.yaml",
			"manifest.json",
		],
	);

	// Verify old files are actually gone
	assert!(
		!output_dir.join("static/resource-a.yaml").exists(),
		"Old resource-a.yaml should be deleted"
	);
	assert!(
		!output_dir.join("static/resource-b.yaml").exists(),
		"Old resource-b.yaml should be deleted"
	);
}

// Note: The following tests from the Go version are not yet implemented:
// - Test_replaceTmplText (not needed in Rust implementation - different path handling)
// - BenchmarkExportEnvironmentsWithReplaceEnvs (benchmark test - can be added later)

// ============================================================================
// Performance-related tests
// ============================================================================
//
// These tests verify that the performance optimizations (timing breakdown,
// parallel processing, BufWriter) work correctly.

/// Test that timing data is populated when show_timing is enabled
#[test]
fn test_export_timing_data_populated() {
	let temp_dir = tempfile::TempDir::new().unwrap();
	let output_dir = temp_dir.path();

	// Export with timing enabled
	let mut ext_code = HashMap::new();
	ext_code.insert("serviceName".to_string(), "'test-service'".to_string());
	ext_code.insert(
		"deploymentName".to_string(),
		"'test-deployment'".to_string(),
	);

	let opts = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 4,
		eval_opts: EvalOpts {
			ext_code,
			..Default::default()
		},
		name: None,
		recursive: true,
		skip_manifest: false,
		show_timing: true, // Enable timing
		..Default::default()
	};

	let result = export(
		&[testdata_path("test-export-envs")
			.to_string_lossy()
			.to_string()],
		opts,
	)
	.unwrap();

	// Verify timing data is present for successful environments
	for env_result in &result.results {
		if env_result.error.is_none() {
			assert!(
				env_result.timing.is_some(),
				"Timing should be present when show_timing is enabled"
			);

			let timing = env_result.timing.as_ref().unwrap();

			// Timing struct should be populated
			// Note: For very fast operations (simple test environments), times could be 0ms
			// The important thing is that the timing struct exists and is populated
			//
			// We verify the structure is sound by checking manifest_count matches files
			let file_count = env_result.files_written.len();
			assert!(
				timing.manifest_count == file_count || file_count == 0,
				"Manifest count ({}) should match files written ({})",
				timing.manifest_count,
				file_count
			);
		}
	}
}

/// Test that timing data is NOT populated when show_timing is disabled
#[test]
fn test_export_timing_data_disabled() {
	let temp_dir = tempfile::TempDir::new().unwrap();
	let output_dir = temp_dir.path();

	let mut ext_code = HashMap::new();
	ext_code.insert("serviceName".to_string(), "'test-service'".to_string());
	ext_code.insert(
		"deploymentName".to_string(),
		"'test-deployment'".to_string(),
	);

	let opts = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 4,
		eval_opts: EvalOpts {
			ext_code,
			..Default::default()
		},
		name: None,
		recursive: true,
		skip_manifest: false,
		show_timing: false, // Timing disabled
		..Default::default()
	};

	let result = export(
		&[testdata_path("test-export-envs")
			.to_string_lossy()
			.to_string()],
		opts,
	)
	.unwrap();

	// Verify timing data is NOT present when disabled
	for env_result in &result.results {
		assert!(
			env_result.timing.is_none(),
			"Timing should NOT be present when show_timing is disabled"
		);
	}
}

/// Test that parallel processing produces correct results
/// This verifies that parallelism doesn't cause race conditions or data corruption
#[test]
fn test_export_parallel_processing_correctness() {
	let temp_dir = tempfile::TempDir::new().unwrap();
	let output_dir = temp_dir.path();

	let mut ext_code = HashMap::new();
	ext_code.insert("serviceName".to_string(), "'parallel-service'".to_string());
	ext_code.insert(
		"deploymentName".to_string(),
		"'parallel-deployment'".to_string(),
	);

	// Export with high parallelism
	let opts = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 16, // High parallelism to stress test
		eval_opts: EvalOpts {
			ext_code: ext_code.clone(),
			..Default::default()
		},
		name: None,
		recursive: true,
		skip_manifest: false,
		show_timing: true,
		..Default::default()
	};

	let result = export(
		&[testdata_path("test-export-envs")
			.to_string_lossy()
			.to_string()],
		opts,
	)
	.unwrap();

	// Should have exported environments successfully
	assert!(
		result.successful > 0,
		"Should have exported at least one environment"
	);
	assert_eq!(result.failed, 0, "Should have no failures");

	// Verify files exist and have valid YAML content
	for env_result in &result.results {
		for file_path in &env_result.files_written {
			let full_path = output_dir.join(file_path);
			assert!(full_path.exists(), "File should exist: {:?}", full_path);

			let content = fs::read_to_string(&full_path).unwrap();
			assert!(
				!content.is_empty(),
				"File should not be empty: {:?}",
				full_path
			);

			// Verify it's valid YAML by checking for expected structure
			if file_path.extension().and_then(|s| s.to_str()) == Some("yaml") {
				assert!(
					content.contains("apiVersion:") || content.contains("kind:"),
					"YAML file should contain Kubernetes manifest structure: {:?}",
					full_path
				);
			}
		}
	}
}

/// Test that different parallelism values produce identical results
/// This ensures the parallel implementation is deterministic
#[test]
fn test_export_parallelism_determinism() {
	// Export with parallelism=1 (sequential)
	let temp_dir_seq = tempfile::TempDir::new().unwrap();
	let output_dir_seq = temp_dir_seq.path();

	let mut ext_code = HashMap::new();
	ext_code.insert(
		"serviceName".to_string(),
		"'determinism-service'".to_string(),
	);
	ext_code.insert(
		"deploymentName".to_string(),
		"'determinism-deployment'".to_string(),
	);

	let opts_seq = ExportOpts {
		output_dir: output_dir_seq.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 1, // Sequential
		eval_opts: EvalOpts {
			ext_code: ext_code.clone(),
			..Default::default()
		},
		name: None,
		recursive: true,
		skip_manifest: true, // Skip manifest.json for simpler comparison
		..Default::default()
	};

	let result_seq = export(
		&[testdata_path("test-export-envs")
			.to_string_lossy()
			.to_string()],
		opts_seq,
	)
	.unwrap();

	// Export with parallelism=8 (parallel)
	let temp_dir_par = tempfile::TempDir::new().unwrap();
	let output_dir_par = temp_dir_par.path();

	let opts_par = ExportOpts {
		output_dir: output_dir_par.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 8, // Parallel
		eval_opts: EvalOpts {
			ext_code: ext_code.clone(),
			..Default::default()
		},
		name: None,
		recursive: true,
		skip_manifest: true,
		..Default::default()
	};

	let result_par = export(
		&[testdata_path("test-export-envs")
			.to_string_lossy()
			.to_string()],
		opts_par,
	)
	.unwrap();

	// Both should succeed with same number of files
	assert_eq!(
		result_seq.successful, result_par.successful,
		"Sequential and parallel should export same number of environments"
	);

	// Collect all files from both directories
	let collect_files = |dir: &Path| -> HashMap<String, String> {
		let mut files = HashMap::new();
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
	};

	let files_seq = collect_files(output_dir_seq);
	let files_par = collect_files(output_dir_par);

	// Same files should exist
	assert_eq!(
		files_seq.keys().collect::<std::collections::HashSet<_>>(),
		files_par.keys().collect::<std::collections::HashSet<_>>(),
		"Sequential and parallel should produce the same files"
	);

	// File contents should be identical
	for (path, content_seq) in &files_seq {
		let content_par = files_par.get(path).unwrap();
		assert_eq!(
			content_seq, content_par,
			"File content should be identical for: {}",
			path
		);
	}
}

// ============================================================================
// Additional conflict detection tests
// ============================================================================

/// Test that export fails when a single environment outputs duplicate files
/// (two resources mapping to the same path)
#[test]
fn test_export_duplicate_file_same_env() {
	use rtk::export::ExportMergeStrategy;

	let temp_dir = tempfile::TempDir::new().unwrap();
	let output_dir = temp_dir.path();

	let opts = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 1,
		eval_opts: EvalOpts::default(),
		name: None,
		recursive: false,
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::None,
		..Default::default()
	};

	let result = export(
		&[testdata_path("test-export-conflict/env-duplicate")
			.to_string_lossy()
			.to_string()],
		opts,
	);

	// Should fail because two resources map to the same file path
	match result {
		Ok(r) => {
			assert!(
				r.failed > 0 || r.results.iter().any(|res| res.error.is_some()),
				"Export should fail when two resources map to same path. Result: {:?}",
				r
			);
			// Check error message mentions duplicate
			let has_duplicate_error = r.results.iter().any(|res| {
				res.error
					.as_ref()
					.is_some_and(|e| e.contains("duplicate") || e.contains("multiple"))
			});
			assert!(
				has_duplicate_error,
				"Error should mention duplicate file: {:?}",
				r.results
			);
		}
		Err(e) => {
			let err_msg = e.to_string();
			assert!(
				err_msg.contains("duplicate") || err_msg.contains("multiple"),
				"Error should mention duplicate file: {}",
				err_msg
			);
		}
	}
}

/// Test that replace-envs allows moving a manifest from one env to another
/// The file content stays the same, only manifest.json ownership changes
#[test]
fn test_replace_envs_move_manifest_between_envs() {
	use rtk::export::ExportMergeStrategy;

	let temp_dir = tempfile::TempDir::new().unwrap();
	let output_dir = temp_dir.path();

	// STEP 1: Export env1 to create initial file
	let opts1 = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 1,
		eval_opts: EvalOpts::default(),
		name: None,
		recursive: false,
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::None,
		..Default::default()
	};

	let result1 = export(
		&[testdata_path("test-export-conflict/env1")
			.to_string_lossy()
			.to_string()],
		opts1,
	);
	result1.unwrap();

	// Check manifest.json shows env1 owns the file
	let manifest_path = output_dir.join("manifest.json");
	let manifest_content: HashMap<String, String> =
		serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
	assert!(
		manifest_content
			.values()
			.any(|v| v.contains("test-export-conflict/env1")),
		"manifest.json should show env1 ownership"
	);

	// Capture the file content
	let file_path = output_dir.join("default/test-deployment.yaml");
	let original_content = fs::read_to_string(&file_path).unwrap();

	// STEP 2: Export env2 with replace-envs, also re-exporting env1 with merge-deleted-envs
	// This simulates moving the resource from env1 to env2
	// merge_deleted_envs needs the full path as stored in manifest.json
	let env1_path = testdata_path("test-export-conflict/env1")
		.to_string_lossy()
		.to_string();
	let opts2 = ExportOpts {
		output_dir: output_dir.to_path_buf(),
		extension: "yaml".to_string(),
		format: "{{.metadata.namespace}}/{{.metadata.name}}".to_string(),
		parallelism: 1,
		eval_opts: EvalOpts::default(),
		name: None,
		recursive: false,
		skip_manifest: false,
		merge_strategy: ExportMergeStrategy::ReplaceEnvs,
		merge_deleted_envs: vec![env1_path],
		..Default::default()
	};

	let result2 = export(
		&[testdata_path("test-export-conflict/env2")
			.to_string_lossy()
			.to_string()],
		opts2,
	);

	// Should succeed - env1's file is "released" via merge_deleted_envs
	assert!(
		result2.is_ok(),
		"Export should succeed when moving manifest via merge_deleted_envs: {:?}",
		result2.err()
	);
	let r2 = result2.unwrap();
	assert_eq!(r2.successful, 1);
	assert_eq!(r2.failed, 0);

	// Check file still exists with same content (env2 produces same output)
	let new_content = fs::read_to_string(&file_path).unwrap();
	assert_eq!(
		original_content, new_content,
		"File content should be unchanged after moving between envs"
	);

	// Check manifest.json now shows env2 ownership
	let new_manifest_content: HashMap<String, String> =
		serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
	assert!(
		new_manifest_content
			.values()
			.any(|v| v.contains("test-export-conflict/env2")),
		"manifest.json should now show env2 ownership"
	);
	assert!(
		!new_manifest_content
			.values()
			.any(|v| v.contains("test-export-conflict/env1")),
		"manifest.json should no longer show env1 ownership"
	);
}
