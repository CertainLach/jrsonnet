//! Integration tests for the show command.
//!
//! These tests call the actual `show_environment` entrypoint, testing the full
//! show flow from Jsonnet evaluation through to YAML output.

use rtk::{
	commands::show::{show_environment, ShowOpts},
	eval::EvalOpts,
};

/// Run a show test with custom options.
///
/// Test structure:
/// - `environment/` - Tanka environment directory
///   - `main.jsonnet` - environment producing manifests
///   - `jsonnetfile.json` - marks the project root
/// - `expected.yaml` - expected YAML output
fn run_show_test_with_opts(test_dir: &std::path::Path, opts: ShowOpts) {
	let env_dir = test_dir.join("environment");

	// Capture show output to a string
	let result = show_environment(env_dir.to_str().unwrap(), EvalOpts::default(), opts);

	let actual = result.expect("show failed");

	// Compare output against expected file
	let expected_path = test_dir.join("expected.yaml");
	let expected = std::fs::read_to_string(&expected_path)
		.unwrap_or_else(|_| panic!("failed to read {}", expected_path.display()));

	assert_eq!(actual, expected, "show output mismatch");
}

/// Run a show test that should fail with a specific error message.
fn run_show_test_expect_error(test_dir: &std::path::Path, opts: ShowOpts, expected_error: &str) {
	let env_dir = test_dir.join("environment");

	let result = show_environment(env_dir.to_str().unwrap(), EvalOpts::default(), opts);

	let err = result.expect_err("show should have failed");
	let err_msg = err.to_string();
	assert!(
		err_msg.contains(expected_error),
		"expected error to contain '{}', but got: {}",
		expected_error,
		err_msg
	);
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Generate show tests.
	///
	/// Usage:
	/// - `show_test!(test_name)` - run with default ShowOpts
	/// - `show_test!(test_name, { opts })` - run with custom ShowOpts expression
	macro_rules! show_test {
		($name:ident) => {
			show_test!($name, { ShowOpts::default() });
		};
		($name:ident, { $opts:expr }) => {
			#[test]
			fn $name() {
				let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
					.join("tests/testdata/show")
					.join(stringify!($name));
				run_show_test_with_opts(&test_dir, $opts);
			}
		};
	}

	// Basic tests
	show_test!(single_configmap);
	show_test!(multiple_manifests);
	show_test!(nested_manifests);
	show_test!(manifest_array);
	show_test!(manifest_list);

	// Environment types
	show_test!(inline_environment);
	show_test!(static_environment);

	// Filtering tests
	show_test!(target_filter, {
		ShowOpts {
			target: vec!["ConfigMap/.*".to_string()],
			..Default::default()
		}
	});

	show_test!(multi_inline_env, {
		ShowOpts {
			name: Some("env-a".to_string()),
			..Default::default()
		}
	});

	// Special cases
	show_test!(inject_labels);
	show_test!(empty_labels_stripped);
}

#[cfg(test)]
mod error_tests {
	use super::*;

	#[test]
	fn test_multiple_inline_envs_requires_name() {
		let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("tests/testdata/show")
			.join("multi_inline_env");

		run_show_test_expect_error(
			&test_dir,
			ShowOpts::default(),
			"multiple inline environments found",
		);
	}

	#[test]
	fn test_name_filter_no_match() {
		let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("tests/testdata/show")
			.join("multi_inline_env");

		run_show_test_expect_error(
			&test_dir,
			ShowOpts {
				name: Some("nonexistent".to_string()),
				..Default::default()
			},
			"no environment found matching name",
		);
	}
}
