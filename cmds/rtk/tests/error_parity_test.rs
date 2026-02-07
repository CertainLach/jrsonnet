//! Error parity tests
//!
//! These tests verify that rtk fails on the same inputs as tk, with similar error messages.
//! Both tk and rtk are run on each test case to verify:
//! 1. Both tools fail on the same input
//! 2. Both error messages contain the expected pattern
//!
//! Structure:
//! - test_fixtures/error_parity/<test_name>/
//!   - main.jsonnet (and other source files)
//!   - jsonnetfile.json (for env discovery)
//!   - expected_error.txt (the error pattern that should appear in both tk and rtk output)

use std::{
	fs,
	path::PathBuf,
	process::{Command, Stdio},
};

use rtk::eval::{eval, EvalOpts};

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

/// Discover all error parity test environments
/// Returns a list of (env_name, env_path, expected_error) tuples
fn discover_error_tests() -> Vec<(String, PathBuf, String)> {
	let error_tests_dir = fixtures_path("error_parity");
	let mut tests = Vec::new();

	if !error_tests_dir.exists() {
		return tests;
	}

	for entry in fs::read_dir(&error_tests_dir).unwrap() {
		let entry = entry.unwrap();
		let path = entry.path();
		if path.is_dir() {
			let expected_error_file = path.join("expected_error.txt");
			if expected_error_file.exists() {
				let name = path.file_name().unwrap().to_string_lossy().to_string();
				let expected_error = fs::read_to_string(&expected_error_file)
					.unwrap()
					.trim()
					.to_string();
				tests.push((name, path, expected_error));
			}
		}
	}

	// Sort by name for consistent test ordering
	tests.sort_by(|a, b| a.0.cmp(&b.0));
	tests
}

/// Run tk eval on the given path and return the combined stdout/stderr output
fn run_tk_eval(env_path: &PathBuf) -> Result<String, String> {
	let output = Command::new("tk")
		.args(["eval", "."])
		.current_dir(env_path)
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.output();

	match output {
		Ok(output) => {
			let stdout = String::from_utf8_lossy(&output.stdout);
			let stderr = String::from_utf8_lossy(&output.stderr);
			let combined = format!("{}{}", stdout, stderr);

			if output.status.success() {
				// tk succeeded (exit code 0) - this means it didn't fail
				Ok(combined)
			} else {
				// tk failed - return the error output
				Err(combined)
			}
		}
		Err(e) => {
			panic!("Failed to run tk: {}. Is tk installed?", e);
		}
	}
}

/// Run an error parity test - verifies both tk and rtk fail with expected error
fn run_error_test(env_path: &PathBuf, expected_error: &str) {
	// First, verify tk fails with the expected error
	match run_tk_eval(env_path) {
		Ok(_) => {
			panic!(
				"[tk] Expected evaluation to fail but it succeeded.\n\
				 Expected error containing: {}",
				expected_error
			);
		}
		Err(tk_error) => {
			if !tk_error.contains(expected_error) {
				panic!(
					"[tk] Error message does not contain expected pattern.\n\
					 Expected to contain: {}\n\
					 Actual error: {}",
					expected_error, tk_error
				);
			}
			println!("  tk: fails with expected error ✓");
		}
	}

	// Then, verify rtk also fails with the expected error
	let result = eval(&env_path.to_string_lossy(), EvalOpts::default());

	match result {
		Ok(_) => {
			panic!(
				"[rtk] Expected evaluation to fail but it succeeded.\n\
				 Expected error containing: {}",
				expected_error
			);
		}
		Err(e) => {
			let error_msg = e.to_string();
			if !error_msg.contains(expected_error) {
				panic!(
					"[rtk] Error message does not contain expected pattern.\n\
					 Expected to contain: {}\n\
					 Actual error: {}",
					expected_error, error_msg
				);
			}
			println!("  rtk: fails with expected error ✓");
		}
	}
}

/// Main test that discovers and runs all error parity tests
#[test]
fn test_error_parity() {
	let tests = discover_error_tests();

	if tests.is_empty() {
		println!("No error parity tests found in test_fixtures/error_parity/");
		return;
	}

	println!("Discovered {} error parity tests:", tests.len());
	for (name, _, _) in &tests {
		println!("  - {}", name);
	}

	let mut failures = Vec::new();

	for (name, path, expected_error) in &tests {
		println!("\n=== Testing {} ===", name);
		let result = std::panic::catch_unwind(|| {
			run_error_test(path, expected_error);
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
		let mut error_msg = format!("\n{} error parity test(s) failed:\n", failures.len());
		for (name, msg) in &failures {
			error_msg.push_str(&format!("\n=== {} ===\n{}\n", name, msg));
		}
		panic!("{}", error_msg);
	}
}
