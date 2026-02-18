use std::{
	ffi::OsStr,
	path::{Path, PathBuf},
	process::Command as ProcessCommand,
	time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
use rtk_diff::directory::compare_directories_detailed;

use crate::{
	cli::{GlobalOptions, GoldenFixturesCli},
	common::run_process_output_with_timeout,
	config,
	constants::{COMMAND_TIMEOUT, TK_EXEC_NAME},
};

const JRSONNET_BINARY_REFERENCE: &str = "binary:/usr/local/bin/jrsonnet";

pub fn execute(cli: GoldenFixturesCli, global: &GlobalOptions) -> Result<()> {
	let fixtures_root = PathBuf::from(&cli.fixtures_dir);
	if !fixtures_root.exists() {
		bail!(
			"fixtures directory does not exist: {}",
			fixtures_root.display()
		);
	}
	if !fixtures_root.is_dir() {
		bail!(
			"fixtures path is not a directory: {}",
			fixtures_root.display()
		);
	}

	let fixture_dirs = discover_fixture_dirs(&fixtures_root)?;
	if fixture_dirs.is_empty() {
		bail!(
			"no fixture directories found under {}",
			fixtures_root.display()
		);
	}

	let tk_exec = global
		.tk
		.clone()
		.unwrap_or_else(|| TK_EXEC_NAME.to_string());
	let tk_exec_path = resolve_executable_path(&tk_exec)
		.with_context(|| format!("Executable '{}' not found in PATH", tk_exec))?;
	let jrsonnet_path = global
		.jrsonnet_path
		.as_deref()
		.map(resolve_executable_path)
		.transpose()
		.with_context(|| {
			format!(
				"Executable '{}' not found in PATH",
				global.jrsonnet_path.clone().unwrap_or_default()
			)
		})?
		.map(|path| path.to_string_lossy().to_string());

	if cli.dry_run {
		eprintln!(
			"Checking golden fixtures are up to date in {}...",
			fixtures_root.display()
		);
	} else {
		eprintln!("Updating golden fixtures in {}...", fixtures_root.display());
	}

	let mut failures = Vec::new();
	let suite_token_root = cli.fixtures_dir.trim_end_matches('/').to_string();
	for fixture_dir in fixture_dirs {
		let basename = fixture_dir
			.file_name()
			.and_then(OsStr::to_str)
			.unwrap_or_default()
			.to_string();
		let testcase = if suite_token_root.is_empty() {
			basename.clone()
		} else {
			format!("{}/{}", suite_token_root, basename)
		};

		let mut export_args = config::load_fixture_command_args(
			&fixtures_root,
			&fixture_dir,
			"export",
			&testcase,
			&basename,
		)
		.with_context(|| format!("failed to load fixture args for {}", fixture_dir.display()))?;
		export_args.extend(["--parallel".to_string(), "1".to_string()]);

		let staged = prepare_fixture_for_export(&fixture_dir, jrsonnet_path.as_deref())
			.with_context(|| format!("failed to stage fixture {}", fixture_dir.display()))?;
		let generated =
			TempDir::create("tk-compare-golden-generated").context("failed creating temp dir")?;
		let generated_dir = generated.path().to_path_buf();

		let export_result = run_tk_export(
			&tk_exec_path,
			staged.working_dir(),
			&generated_dir,
			&export_args,
		);

		match export_result {
			Ok(()) => {
				let golden_dir = fixture_dir.join("golden");
				if cli.dry_run {
					let differences = differences_without_manifest(&golden_dir, &generated_dir)
						.with_context(|| {
							format!("failed comparing fixture {}", fixture_dir.display())
						})?;
					if !differences.is_empty() {
						failures.push(FixtureFailure {
							fixture: basename,
							reason: FailureReason::Differences(differences),
						});
					}
				} else {
					replace_directory_contents(&golden_dir, &generated_dir).with_context(|| {
						format!("failed writing golden dir for {}", fixture_dir.display())
					})?;
				}
			}
			Err(err) => {
				failures.push(FixtureFailure {
					fixture: basename,
					reason: FailureReason::Export(err.to_string()),
				});
			}
		}
	}

	if !failures.is_empty() {
		let mut msg = format!("{} fixture(s) failed:\n", failures.len());
		for failure in failures {
			msg.push_str(&format!("\n=== {} ===\n", failure.fixture));
			match failure.reason {
				FailureReason::Export(err) => {
					msg.push_str(&err);
					msg.push('\n');
				}
				FailureReason::Differences(diffs) => {
					if diffs.is_empty() {
						msg.push_str("golden output differs\n");
					} else {
						for diff in diffs {
							msg.push_str(&diff);
							msg.push('\n');
						}
					}
				}
			}
		}
		bail!("{msg}");
	}

	if cli.dry_run {
		eprintln!("Golden fixtures are up to date.");
	} else {
		eprintln!("Golden fixtures updated.");
	}

	Ok(())
}

#[derive(Debug)]
struct FixtureFailure {
	fixture: String,
	reason: FailureReason,
}

#[derive(Debug)]
enum FailureReason {
	Export(String),
	Differences(Vec<String>),
}

#[derive(Debug)]
struct StagedFixture {
	working_dir: PathBuf,
	_temp: Option<TempDir>,
}

impl StagedFixture {
	fn working_dir(&self) -> &Path {
		&self.working_dir
	}
}

fn prepare_fixture_for_export(
	fixture_dir: &Path,
	jrsonnet_path: Option<&str>,
) -> Result<StagedFixture> {
	if jrsonnet_path.is_none() {
		return Ok(StagedFixture {
			working_dir: fixture_dir.to_path_buf(),
			_temp: None,
		});
	}

	let temp = TempDir::create("tk-compare-golden-stage")?;
	let staged = temp.path().join("fixture");
	copy_dir_recursive(fixture_dir, &staged)?;
	if let Some(path) = jrsonnet_path {
		rewrite_jrsonnet_binary_path(&staged, path)?;
	}
	Ok(StagedFixture {
		working_dir: staged,
		_temp: Some(temp),
	})
}

fn run_tk_export(
	tk_exec: &Path,
	working_dir: &Path,
	destination: &Path,
	export_args: &[String],
) -> Result<()> {
	let mut argv = vec![
		"export".to_string(),
		destination.to_string_lossy().to_string(),
		".".to_string(),
	];
	argv.extend(export_args.iter().cloned());

	let mut cmd = ProcessCommand::new(tk_exec);
	cmd.current_dir(working_dir).args(&argv);

	let output = run_process_output_with_timeout(&mut cmd, COMMAND_TIMEOUT).with_context(|| {
		format!(
			"failed to execute {} {:?} (cwd: {})",
			tk_exec.display(),
			argv,
			working_dir.display()
		)
	})?;

	if output.status.success() {
		return Ok(());
	}

	let stdout = String::from_utf8_lossy(&output.stdout);
	let stderr = String::from_utf8_lossy(&output.stderr);
	bail!(
		"tk export failed (cwd: {})\ncommand: {} {:?}\nstdout:\n{}\nstderr:\n{}",
		working_dir.display(),
		tk_exec.display(),
		argv,
		stdout.trim_end(),
		stderr.trim_end()
	);
}

fn differences_without_manifest(expected: &Path, actual: &Path) -> Result<Vec<String>> {
	if !expected.exists() {
		return Ok(vec![format!(
			"golden directory missing: {}",
			expected.display()
		)]);
	}
	if !expected.is_dir() {
		return Ok(vec![format!(
			"golden path is not a directory: {}",
			expected.display()
		)]);
	}

	let expected_filtered =
		TempDir::create("tk-compare-golden-expected").context("failed creating temp dir")?;
	let actual_filtered =
		TempDir::create("tk-compare-golden-actual").context("failed creating temp dir")?;
	copy_without_manifest(expected, expected_filtered.path())?;
	copy_without_manifest(actual, actual_filtered.path())?;

	let cmp = compare_directories_detailed(
		expected_filtered.path().to_string_lossy().as_ref(),
		actual_filtered.path().to_string_lossy().as_ref(),
	)?;
	if cmp.matched {
		return Ok(Vec::new());
	}
	Ok(cmp.differences)
}

fn discover_fixture_dirs(fixtures_root: &Path) -> Result<Vec<PathBuf>> {
	let mut dirs: Vec<_> = std::fs::read_dir(fixtures_root)
		.with_context(|| format!("failed to read {}", fixtures_root.display()))?
		.filter_map(|entry| entry.ok())
		.map(|entry| entry.path())
		.filter(|path| path.is_dir())
		.collect();
	dirs.sort();
	Ok(dirs)
}

fn resolve_executable_path(executable: &str) -> Result<PathBuf> {
	which::which(executable).map_err(Into::into)
}

fn replace_directory_contents(target_dir: &Path, source_dir: &Path) -> Result<()> {
	if target_dir.exists() {
		std::fs::remove_dir_all(target_dir)
			.with_context(|| format!("failed to remove {}", target_dir.display()))?;
	}
	copy_dir_recursive(source_dir, target_dir)
}

fn copy_without_manifest(src: &Path, dst: &Path) -> Result<()> {
	std::fs::create_dir_all(dst)?;
	for entry in std::fs::read_dir(src)? {
		let entry = entry?;
		let src_path = entry.path();
		let dst_path = dst.join(entry.file_name());
		if src_path.is_dir() {
			copy_without_manifest(&src_path, &dst_path)?;
			continue;
		}
		if dst_path == dst.join("manifest.json") {
			continue;
		}
		std::fs::copy(&src_path, &dst_path).with_context(|| {
			format!(
				"failed copying {} -> {}",
				src_path.display(),
				dst_path.display()
			)
		})?;
	}
	Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
	if !src.is_dir() {
		return Ok(());
	}
	std::fs::create_dir_all(dst)?;
	for entry in std::fs::read_dir(src)? {
		let entry = entry?;
		let src_path = entry.path();
		let dst_path = dst.join(entry.file_name());
		if src_path.is_dir() {
			copy_dir_recursive(&src_path, &dst_path)?;
			continue;
		}
		std::fs::copy(&src_path, &dst_path).with_context(|| {
			format!(
				"failed copying {} -> {}",
				src_path.display(),
				dst_path.display()
			)
		})?;
	}
	Ok(())
}

fn rewrite_jrsonnet_binary_path(root: &Path, jrsonnet_path: &str) -> Result<()> {
	let mut stack = vec![root.to_path_buf()];
	let replacement = format!("binary:{jrsonnet_path}");
	while let Some(dir) = stack.pop() {
		for entry in std::fs::read_dir(&dir)? {
			let entry = entry?;
			let path = entry.path();
			if path.is_dir() {
				stack.push(path);
				continue;
			}
			let is_candidate = path
				.extension()
				.and_then(|ext| ext.to_str())
				.map(|ext| matches!(ext, "jsonnet" | "json"))
				.unwrap_or(false);
			if !is_candidate {
				continue;
			}
			let Ok(contents) = std::fs::read_to_string(&path) else {
				continue;
			};
			if !contents.contains(JRSONNET_BINARY_REFERENCE) {
				continue;
			}
			let updated = contents.replace(JRSONNET_BINARY_REFERENCE, &replacement);
			std::fs::write(&path, updated)
				.with_context(|| format!("failed updating {}", path.display()))?;
		}
	}
	Ok(())
}

#[derive(Debug)]
struct TempDir {
	path: PathBuf,
}

impl TempDir {
	fn create(prefix: &str) -> Result<Self> {
		let ts = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.unwrap_or_default()
			.as_nanos();
		let path = std::env::temp_dir().join(format!("{prefix}-{}-{}", std::process::id(), ts));
		std::fs::create_dir_all(&path)
			.with_context(|| format!("failed to create {}", path.display()))?;
		Ok(Self { path })
	}

	fn path(&self) -> &Path {
		&self.path
	}
}

impl Drop for TempDir {
	fn drop(&mut self) {
		let _ = std::fs::remove_dir_all(&self.path);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::fs;

	#[test]
	fn test_discover_fixture_dirs_sorted() {
		let root = TempDir::create("tk-compare-golden-discover-test").unwrap();
		fs::create_dir_all(root.path().join("b")).unwrap();
		fs::create_dir_all(root.path().join("a")).unwrap();
		fs::write(root.path().join("not-a-dir"), "").unwrap();

		let discovered = discover_fixture_dirs(root.path()).unwrap();
		let names: Vec<_> = discovered
			.iter()
			.map(|p| p.file_name().unwrap().to_string_lossy().to_string())
			.collect();
		assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
	}

	#[test]
	fn test_rewrite_jrsonnet_binary_path() {
		let root = TempDir::create("tk-compare-golden-rewrite-test").unwrap();
		let jsonnet = root.path().join("main.jsonnet");
		fs::write(
			&jsonnet,
			"{ spec: { exportJsonnetImplementation: \"binary:/usr/local/bin/jrsonnet\" } }",
		)
		.unwrap();

		rewrite_jrsonnet_binary_path(root.path(), "/tmp/custom-jrsonnet").unwrap();
		let updated = fs::read_to_string(&jsonnet).unwrap();
		assert!(updated.contains("binary:/tmp/custom-jrsonnet"));
	}

	#[test]
	fn test_load_fixture_command_args_uses_suite_and_fixture_layers() {
		let root = TempDir::create("tk-compare-golden-args-test").unwrap();
		let suite = root.path().join("suite");
		let fixture = suite.join("case-a");
		fs::create_dir_all(&fixture).unwrap();
		fs::write(
			suite.join("tk-compare.toml"),
			"[args]\nexport=[\"--extension\",\"golden\"]\n",
		)
		.unwrap();
		fs::write(
			fixture.join("tk-compare.toml"),
			"extra_args=[\"--name={{basename}}\"]\n",
		)
		.unwrap();

		let args = config::load_fixture_command_args(
			&suite,
			&fixture,
			"export",
			"fixtures/case-a",
			"case-a",
		)
		.unwrap();
		assert_eq!(
			args,
			vec![
				"--name=case-a".to_string(),
				"--extension".to_string(),
				"golden".to_string(),
			]
		);
	}
}
