use std::path::PathBuf;

fn abs_path(path: &str) -> String {
	let test_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.join(path);
	test_root
		.canonicalize()
		.unwrap()
		.to_string_lossy()
		.to_string()
}

#[test]
fn test_no_files() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result = rtk::importers::find_importers(&root, vec![]).unwrap();
	assert_eq!(result, Vec::<String>::new());
}

#[test]
fn test_invalid_file() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let invalid_file = PathBuf::from(&root)
		.join("does-not-exist.jsonnet")
		.to_string_lossy()
		.to_string();
	let result = rtk::importers::find_importers(&root, vec![invalid_file]);
	assert!(result.is_err());
	assert!(result.unwrap_err().to_string().contains("does not exist"));
}

#[test]
fn test_project_with_no_imports() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let file = abs_path("environments/no-imports/main.jsonnet");
	let result = rtk::importers::find_importers(&root, vec![file.clone()]).unwrap();
	assert_eq!(result, vec![file]); // itself only
}

#[test]
fn test_local_import() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result = rtk::importers::find_importers(
		&root,
		vec![abs_path(
			"environments/imports-locals-and-vendored/local-file1.libsonnet",
		)],
	)
	.unwrap();
	assert_eq!(
		result,
		vec![abs_path(
			"environments/imports-locals-and-vendored/main.jsonnet"
		)]
	);
}

#[test]
fn test_local_import_with_relative_path() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result = rtk::importers::find_importers(
		&root,
		vec![abs_path(
			"environments/imports-locals-and-vendored/local-file2.libsonnet",
		)],
	)
	.unwrap();
	assert_eq!(
		result,
		vec![abs_path(
			"environments/imports-locals-and-vendored/main.jsonnet"
		)]
	);
}

#[test]
fn test_lib_imported_through_chain() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result =
		rtk::importers::find_importers(&root, vec![abs_path("lib/lib1/main.libsonnet")]).unwrap();
	assert_eq!(
		result,
		vec![abs_path(
			"environments/imports-lib-and-vendored-through-chain/main.jsonnet"
		)]
	);
}

#[test]
fn test_vendored_lib_imported_through_chain_and_directly() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result =
		rtk::importers::find_importers(&root, vec![abs_path("vendor/vendored/main.libsonnet")])
			.unwrap();
	let mut expected = vec![
		abs_path("environments/imports-lib-and-vendored-through-chain/main.jsonnet"),
		abs_path("environments/imports-locals-and-vendored/main.jsonnet"),
		abs_path("environments/imports-symlinked-vendor/main.jsonnet"),
	];
	expected.sort();
	assert_eq!(result, expected);
}

#[test]
fn test_vendored_lib_found_through_symlink() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result = rtk::importers::find_importers(
		&root,
		vec![abs_path("vendor/vendor-symlinked/main.libsonnet")],
	)
	.unwrap();
	let mut expected = vec![
		abs_path("environments/imports-lib-and-vendored-through-chain/main.jsonnet"),
		abs_path("environments/imports-locals-and-vendored/main.jsonnet"),
		abs_path("environments/imports-symlinked-vendor/main.jsonnet"),
	];
	expected.sort();
	assert_eq!(result, expected);
}

#[test]
fn test_text_file() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result =
		rtk::importers::find_importers(&root, vec![abs_path("vendor/vendored/text-file.txt")])
			.unwrap();
	let mut expected = vec![
		abs_path("environments/imports-lib-and-vendored-through-chain/main.jsonnet"),
		abs_path("environments/imports-locals-and-vendored/main.jsonnet"),
		abs_path("environments/imports-symlinked-vendor/main.jsonnet"),
	];
	expected.sort();
	assert_eq!(result, expected);
}

#[test]
fn test_relative_imported_environment() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result = rtk::importers::find_importers(
		&root,
		vec![abs_path("environments/relative-imported/main.jsonnet")],
	)
	.unwrap();
	let mut expected = vec![
		abs_path("environments/relative-import/main.jsonnet"),
		abs_path("environments/relative-imported/main.jsonnet"), // itself, it's a main file
	];
	expected.sort();
	assert_eq!(result, expected);
}

#[test]
fn test_relative_imported_environment_with_doubled_dotdot() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result = rtk::importers::find_importers(
		&root,
		vec![abs_path("environments/relative-imported2/main.jsonnet")],
	)
	.unwrap();
	let mut expected = vec![
		abs_path("environments/relative-import/main.jsonnet"),
		abs_path("environments/relative-imported2/main.jsonnet"), // itself, it's a main file
	];
	expected.sort();
	assert_eq!(result, expected);
}

#[test]
fn test_relative_imported_text_file() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result =
		rtk::importers::find_importers(&root, vec![abs_path("other-files/test.txt")]).unwrap();
	assert_eq!(
		result,
		vec![abs_path("environments/relative-import/main.jsonnet")]
	);
}

#[test]
fn test_relative_imported_text_file_with_doubled_dotdot() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result =
		rtk::importers::find_importers(&root, vec![abs_path("other-files/test2.txt")]).unwrap();
	assert_eq!(
		result,
		vec![abs_path("environments/relative-import/main.jsonnet")]
	);
}

#[test]
fn test_vendor_override_in_env_override_vendor_used() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result = rtk::importers::find_importers(
		&root,
		vec![abs_path(
			"environments/vendor-override-in-env/vendor/vendor-override-in-env/main.libsonnet",
		)],
	)
	.unwrap();
	assert_eq!(
		result,
		vec![abs_path("environments/vendor-override-in-env/main.jsonnet")]
	);
}

#[test]
fn test_vendor_override_in_env_global_vendor_unused() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result = rtk::importers::find_importers(
		&root,
		vec![abs_path("vendor/vendor-override-in-env/main.libsonnet")],
	)
	.unwrap();
	assert_eq!(result, Vec::<String>::new());
}

#[test]
fn test_imported_file_in_lib_relative_to_env() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result = rtk::importers::find_importers(
		&root,
		vec![abs_path(
			"environments/lib-import-relative-to-env/file-to-import.libsonnet",
		)],
	)
	.unwrap();
	assert_eq!(
		result,
		vec![abs_path(
			"environments/lib-import-relative-to-env/folder1/folder2/main.jsonnet"
		)]
	);
}

#[test]
fn test_unused_deleted_file() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result = rtk::importers::find_importers(
		&root,
		vec!["deleted:vendor/deleted-vendor/main.libsonnet".to_string()],
	)
	.unwrap();
	assert_eq!(result, Vec::<String>::new());
}

#[test]
fn test_deleted_local_path_that_is_still_potentially_imported() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result = rtk::importers::find_importers(
		&root,
		vec!["deleted:environments/using-deleted-stuff/my-import-dir/main.libsonnet".to_string()],
	)
	.unwrap();
	assert_eq!(
		result,
		vec![abs_path("environments/using-deleted-stuff/main.jsonnet")]
	);
}

#[test]
fn test_deleted_lib_that_is_still_potentially_imported() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result = rtk::importers::find_importers(
		&root,
		vec!["deleted:lib/my-import-dir/main.libsonnet".to_string()],
	)
	.unwrap();
	assert_eq!(
		result,
		vec![abs_path("environments/using-deleted-stuff/main.jsonnet")]
	);
}

#[test]
fn test_deleted_vendor_that_is_still_potentially_imported() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result = rtk::importers::find_importers(
		&root,
		vec!["deleted:vendor/my-import-dir/main.libsonnet".to_string()],
	)
	.unwrap();
	assert_eq!(
		result,
		vec![abs_path("environments/using-deleted-stuff/main.jsonnet")]
	);
}

#[test]
fn test_deleted_lib_that_is_still_potentially_imported_relative_path_from_root() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result = rtk::importers::find_importers(
		&root,
		vec!["deleted:lib/my-import-dir/main.libsonnet".to_string()],
	)
	.unwrap();
	assert_eq!(
		result,
		vec![abs_path("environments/using-deleted-stuff/main.jsonnet")]
	);
}

#[test]
fn test_deleted_dir_in_environment() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result = rtk::importers::find_importers(
		&root,
		vec!["deleted:environments/no-imports/deleted-dir/deleted-file.libsonnet".to_string()],
	)
	.unwrap();
	assert_eq!(
		result,
		vec![abs_path("environments/no-imports/main.jsonnet")]
	);
}

#[test]
fn test_imports_through_a_main_file_are_followed() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result = rtk::importers::find_importers(
		&root,
		vec![abs_path(
			"environments/import-other-main-file/env2/file.libsonnet",
		)],
	)
	.unwrap();
	let mut expected = vec![
		abs_path("environments/import-other-main-file/env1/main.jsonnet"),
		abs_path("environments/import-other-main-file/env2/main.jsonnet"),
	];
	expected.sort();
	assert_eq!(result, expected);
}

#[test]
fn test_lib_file_imports_environment_file() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result = rtk::importers::find_importers(
		&root,
		vec![abs_path(
			"environments/lib-imports-environment/config.jsonnet",
		)],
	)
	.unwrap();
	let mut expected = vec![
		abs_path("environments/lib-imports-environment/main.jsonnet"),
		abs_path("environments/uses-lib-that-imports-env/main.jsonnet"),
	];
	expected.sort();
	assert_eq!(result, expected);
}

#[test]
fn test_complex_transitive_chain_env1_lib1_env2_lib3_env3() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	let result = rtk::importers::find_importers(
		&root,
		vec![abs_path("environments/chain-env1/config.jsonnet")],
	)
	.unwrap();
	let mut expected = vec![
		abs_path("environments/chain-env1/main.jsonnet"), // direct env importer
		abs_path("environments/chain-env2/main.jsonnet"), // via lib1
		abs_path("environments/chain-env3/main.jsonnet"), // via lib1->env2->lib3
	];
	expected.sort();
	assert_eq!(result, expected);
}

#[test]
fn test_relative_import_from_lib_to_env_should_not_match_as_lib_vendor() {
	let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("testdata/findImporters")
		.to_string_lossy()
		.to_string();
	// Search for an environment file that is imported by a lib file using a relative path starting with ../
	// The lib file should NOT be found as an importer because relative imports starting with ../
	// should only match via the relative import check, not the lib/vendor check
	let result = rtk::importers::find_importers(
		&root,
		vec![abs_path("environments/relative-import-target/main.jsonnet")],
	)
	.unwrap();
	// The lib file imports it with ../environments/relative-import-target/main.jsonnet
	// Without the fix, this incorrectly matches via lib/vendor path check because:
	// - The relative import check resolves it to lib/environments/relative-import-target/main.jsonnet (doesn't match)
	// - The lib/vendor check then does lib.join("../environments/...") = environments/... (incorrectly matches!)
	// With the fix, the lib/vendor check should skip paths starting with ../, so it shouldn't match
	let mut expected = vec![
		abs_path("environments/relative-import-target/main.jsonnet"), // itself, it's a main file
	];
	expected.sort();
	// Without the fix, lib/internal-alerting/main.libsonnet is incorrectly matched as an importer
	// via lib/vendor path check. Even though it gets filtered out (non-main files are filtered),
	// this causes incorrect transitive matching: if another env imports lib/internal-alerting/main.libsonnet,
	// that env would incorrectly be included in the result.
	//
	// Create an environment that imports the lib file to test transitive matching
	// Without the fix: test-env-imports-lib/main.jsonnet would be incorrectly included
	// With the fix: test-env-imports-lib/main.jsonnet should NOT be included
	let incorrectly_included =
		result.contains(&abs_path("environments/test-env-imports-lib/main.jsonnet"));
	assert!(!incorrectly_included,
		"Environment importing lib file with relative import to ../environments/ should NOT be included. Result: {:?}", result);
	assert_eq!(
		result, expected,
		"lib file with relative import starting with ../ should not match via lib/vendor check"
	);
}
