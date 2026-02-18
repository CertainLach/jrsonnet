//! Directory tree comparison with per-file textual diffs.

use std::{
	collections::{BTreeSet, HashMap},
	path::PathBuf,
};

use anyhow::Result;
use similar::{ChangeTag, TextDiff};

#[derive(Debug)]
pub struct DirectoryComparisonResult {
	pub matched: bool,
	pub differences: Vec<String>,
}

/// Compare two directories recursively.
///
/// The result contains user-facing difference strings, including inline `-`/`+`
/// line-level deltas for files that exist in both directories but differ.
pub fn compare_directories_detailed(dir1: &str, dir2: &str) -> Result<DirectoryComparisonResult> {
	let files1 = collect_files(dir1)?;
	let files2 = collect_files(dir2)?;

	let mut differences = Vec::new();
	let all_paths: BTreeSet<String> = files1
		.keys()
		.chain(files2.keys())
		.cloned()
		.collect::<BTreeSet<_>>();

	for path in all_paths {
		match (files1.get(&path), files2.get(&path)) {
			(Some(content1), Some(content2)) => {
				if content1 == content2 {
					continue;
				}

				let text1 = String::from_utf8_lossy(content1).to_string();
				let text2 = String::from_utf8_lossy(content2).to_string();
				let lines1: Vec<&str> = text1.lines().collect();
				let lines2: Vec<&str> = text2.lines().collect();
				let diff_lines = lines1.len().abs_diff(lines2.len()).max(
					lines1
						.iter()
						.zip(lines2.iter())
						.filter(|(a, b)| a != b)
						.count(),
				);

				differences.push(render_content_diff(&path, &text1, &text2, diff_lines));
			}
			(Some(_), None) => differences.push(format!("{}: only in first directory", path)),
			(None, Some(_)) => differences.push(format!("{}: only in second directory", path)),
			(None, None) => unreachable!(),
		}
	}

	Ok(DirectoryComparisonResult {
		matched: differences.is_empty(),
		differences,
	})
}

fn collect_files(dir: &str) -> Result<HashMap<String, Vec<u8>>> {
	use std::fs;

	let mut files = HashMap::new();
	let base_path = PathBuf::from(dir);

	if !base_path.exists() {
		return Ok(files);
	}

	fn visit_dirs(
		dir: &PathBuf,
		base: &PathBuf,
		files: &mut HashMap<String, Vec<u8>>,
	) -> Result<()> {
		if dir.is_dir() {
			for entry in fs::read_dir(dir)? {
				let entry = entry?;
				let path = entry.path();
				if path.is_dir() {
					visit_dirs(&path, base, files)?;
				} else {
					let relative_path = path
						.strip_prefix(base)
						.unwrap()
						.to_string_lossy()
						.to_string();
					let content = fs::read(&path)?;
					files.insert(relative_path, content);
				}
			}
		}
		Ok(())
	}

	visit_dirs(&base_path, &base_path, &mut files)?;
	Ok(files)
}

fn render_content_diff(path: &str, first: &str, second: &str, diff_lines: usize) -> String {
	let mut out = String::new();
	out.push_str(&format!(
		"{path}: content differs (~{diff_lines} line differences)\n"
	));
	out.push_str(&format!("--- first/{path}\n"));
	out.push_str(&format!("+++ second/{path}\n"));

	let mut wrote_changes = false;
	let diff = TextDiff::from_lines(first, second);
	for change in diff.iter_all_changes() {
		let sign = match change.tag() {
			ChangeTag::Delete => "- ",
			ChangeTag::Insert => "+ ",
			ChangeTag::Equal => continue,
		};
		wrote_changes = true;
		let line = change.to_string();
		out.push_str(sign);
		out.push_str(&line);
		if !line.ends_with('\n') {
			out.push('\n');
		}
	}

	if !wrote_changes {
		out.push_str("(no textual line-level diff available)\n");
	}

	out.trim_end_matches('\n').to_string()
}

#[cfg(test)]
mod tests {
	use std::fs;

	use rstest::rstest;
	use tempfile::tempdir;

	use super::*;

	#[rstest]
	#[case(&[("file.txt", "hello")], &[("file.txt", "hello")], true)]
	#[case(&[("file.txt", "hello")], &[("file.txt", "world")], false)]
	#[case(&[("file.txt", "line1\nline2\nline3")], &[("file.txt", "line1\ndifferent\nline3")], false)]
	#[case(&[("a.txt", "a")], &[("b.txt", "b")], false)]
	#[case(&[], &[], true)]
	fn test_compare_directories(
		#[case] files1: &[(&str, &str)],
		#[case] files2: &[(&str, &str)],
		#[case] expected_matched: bool,
	) {
		let dir = tempdir().unwrap();
		let dir1 = dir.path().join("a");
		let dir2 = dir.path().join("b");
		fs::create_dir_all(&dir1).unwrap();
		fs::create_dir_all(&dir2).unwrap();

		for (name, content) in files1 {
			fs::write(dir1.join(name), content).unwrap();
		}
		for (name, content) in files2 {
			fs::write(dir2.join(name), content).unwrap();
		}

		let result =
			compare_directories_detailed(dir1.to_str().unwrap(), dir2.to_str().unwrap()).unwrap();

		assert_eq!(result.matched, expected_matched);
	}
}
