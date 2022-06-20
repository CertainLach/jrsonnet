use std::{fs, path::Path};

use anyhow::Result;
use xshell::{cmd, Shell};

/// Checks that the `file` has the specified `contents`. If that is not the
/// case, updates the file and then fails the test.
pub fn ensure_file_contents(file: &Path, contents: &str) -> Result<()> {
	if let Ok(old_contents) = fs::read_to_string(file) {
		if normalize_newlines(&old_contents) == normalize_newlines(contents) {
			// File is already up to date.
			return Ok(());
		}
	}

	eprintln!(" {} was not up-to-date, updating\n", file.display());
	if std::env::var("CI").is_ok() {
		eprintln!("NOTE: run `cargo xtask` locally and commit the updated files\n");
	}
	if let Some(parent) = file.parent() {
		let _ = fs::create_dir_all(parent);
	}
	fs::write(file, contents).unwrap();
	Ok(())
}

// Eww, someone configured git to use crlf?
fn normalize_newlines(s: &str) -> String {
	s.replace("\r\n", "\n")
}

pub(crate) fn pluralize(s: &str) -> String {
	format!("{}s", s)
}

pub fn to_upper_snake_case(s: &str) -> String {
	let mut buf = String::with_capacity(s.len());
	let mut prev = false;
	for c in s.chars() {
		if c.is_ascii_uppercase() && prev {
			buf.push('_')
		}
		prev = true;

		buf.push(c.to_ascii_uppercase());
	}
	buf
}
pub fn to_lower_snake_case(s: &str) -> String {
	let mut buf = String::with_capacity(s.len());
	let mut prev = false;
	for c in s.chars() {
		if c.is_ascii_uppercase() && prev {
			buf.push('_')
		}
		prev = true;

		buf.push(c.to_ascii_lowercase());
	}
	buf
}

pub fn to_pascal_case(s: &str) -> String {
	let mut buf = String::with_capacity(s.len());
	let mut prev_is_underscore = true;
	for c in s.chars() {
		if c == '_' {
			prev_is_underscore = true;
		} else if prev_is_underscore {
			buf.push(c.to_ascii_uppercase());
			prev_is_underscore = false;
		} else {
			buf.push(c.to_ascii_lowercase());
		}
	}
	buf
}

pub fn reformat(text: &str) -> Result<String> {
	// let _e = pushenv("RUSTUP_TOOLCHAIN", "stable");
	// rustfmt()?;
	let sh = Shell::new()?;
	let stdout = cmd!(sh, "rustfmt").stdin(text).read()?;
	Ok(format!(
		"{}\n\n{}\n",
		"//! This is a generated file, please do not edit manually. Changes can be
//! made in codegeneration that lives in `xtask` top-level dir.",
		stdout
	))
}
