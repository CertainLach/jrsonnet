//! Workspace management for isolated command execution.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::types::Pair;

const BASE_PATH: &str = ".tk-compare-workspace";

/// Manages workspace directories for isolated command execution.
#[derive(Debug)]
pub struct Workspace {
	paths: Pair<String>,
}

impl Workspace {
	/// Create a new workspace with directories for each executor.
	pub fn new(exec1_name: &str, exec2_name: &str) -> Self {
		Self {
			paths: Pair::new(
				format!("{}/{}", BASE_PATH, exec1_name),
				format!("{}/{}", BASE_PATH, exec2_name),
			),
		}
	}

	/// Get the workspace path for the first executor.
	pub fn first(&self) -> &str {
		&self.paths.first
	}

	/// Get the workspace path for the second executor.
	pub fn second(&self) -> &str {
		&self.paths.second
	}

	/// Clean up workspace directories (remove and recreate empty).
	pub fn clean(&self) -> Result<()> {
		for path in [&self.paths.first, &self.paths.second] {
			let p = Path::new(path);
			if p.exists() {
				std::fs::remove_dir_all(p)?;
			}
		}
		Ok(())
	}
}

/// Clean up the entire workspace base directory.
pub fn cleanup_all() -> Result<()> {
	let p = Path::new(BASE_PATH);
	if p.exists() {
		std::fs::remove_dir_all(p)?;
	}
	Ok(())
}

/// Check if the workspace should be kept based on CLI flag or environment variable.
pub fn should_keep(cli_flag: bool) -> bool {
	cli_flag || std::env::var("KEEP_WORKSPACE").unwrap_or_default() == "true"
}

/// Print message about preserved workspace location.
pub fn print_preserved_message() {
	eprintln!("\nWorkspace preserved at: {}/", BASE_PATH);
}

pub fn stage_working_dir(
	workspace_root: &str,
	working_dir: &str,
	jrsonnet_path: Option<&str>,
) -> Result<()> {
	let src = Path::new(working_dir);
	let dst = Path::new(workspace_root).join(working_dir);
	copy_dir_recursive(src, &dst)?;
	if let Some(path) = jrsonnet_path {
		rewrite_jrsonnet_path(&dst, path)?;
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

fn rewrite_jrsonnet_path(root: &Path, jrsonnet_path: &str) -> Result<()> {
	let mut stack = vec![PathBuf::from(root)];
	let old = "binary:/usr/local/bin/jrsonnet";
	let new = format!("binary:{jrsonnet_path}");
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
			let contents = std::fs::read_to_string(&path).unwrap_or_default();
			if !contents.contains(old) {
				continue;
			}
			let updated = contents.replace(old, &new);
			std::fs::write(&path, updated)
				.with_context(|| format!("failed updating {}", path.display()))?;
		}
	}
	Ok(())
}
