//! Unified diff parsing and semantic comparison.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use similar::TextDiff;

use crate::SimilarityScore;

/// Parsed unified diff grouped by resource path.
#[derive(Debug)]
pub struct ParsedUnifiedDiff {
	resources: BTreeMap<String, Vec<DiffChange>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum DiffChange {
	Add(String),
	Remove(String),
}

impl ParsedUnifiedDiff {
	/// Parse unified diff text into a semantic structure.
	///
	/// Returns `None` if parsing fails.
	pub fn parse(diff_output: &str) -> Option<Self> {
		use patch::Patch;

		let patches = Patch::from_multiple(diff_output).ok()?;

		let mut resources = BTreeMap::new();
		for patch in patches {
			let name = extract_resource_name(&patch.new.path);
			let mut changes = Vec::new();
			for hunk in &patch.hunks {
				for line in &hunk.lines {
					match line {
						patch::Line::Add(s) => changes.push(DiffChange::Add(s.to_string())),
						patch::Line::Remove(s) => changes.push(DiffChange::Remove(s.to_string())),
						patch::Line::Context(_) => {}
					}
				}
			}
			resources.insert(name, changes);
		}

		Some(Self { resources })
	}

	#[cfg(test)]
	pub fn resource_names(&self) -> impl Iterator<Item = &str> {
		self.resources.keys().map(|s| s.as_str())
	}

	/// Similarity score against another parsed diff.
	pub fn similarity_score(&self, other: &Self) -> SimilarityScore {
		let (matched, total) = self.calculate_similarity(other);
		SimilarityScore::new(matched, total)
	}

	fn calculate_similarity(&self, other: &Self) -> (usize, usize) {
		let all_resources: BTreeSet<_> = self
			.resources
			.keys()
			.chain(other.resources.keys())
			.collect();
		let mut matched = 0;
		let mut total = 0;

		for name in all_resources {
			let changes1 = self.resources.get(name);
			let changes2 = other.resources.get(name);
			match (changes1, changes2) {
				(Some(c1), Some(c2)) => {
					let set1: HashSet<_> = c1.iter().collect();
					let set2: HashSet<_> = c2.iter().collect();
					total += set1.union(&set2).count();
					matched += set1.intersection(&set2).count();
				}
				(Some(c), None) | (None, Some(c)) => total += c.len(),
				(None, None) => unreachable!(),
			}
		}

		(matched, total)
	}
}

impl PartialEq for ParsedUnifiedDiff {
	fn eq(&self, other: &Self) -> bool {
		self.resources == other.resources
	}
}

impl Eq for ParsedUnifiedDiff {}

fn extract_resource_name(path: &str) -> String {
	if path == "/dev/null" {
		return path.to_string();
	}
	if let Some(pos) = path.rfind('/') {
		let after_slash = &path[pos + 1..];
		if after_slash.contains('.') {
			return after_slash.to_string();
		}
	}
	if path.starts_with("a/") || path.starts_with("b/") {
		return path[2..].to_string();
	}
	path.to_string()
}

/// Render a unified diff from two YAML/text blobs with explicit headers.
pub fn render_unified_diff(
	current: &str,
	desired: &str,
	old_header: &str,
	new_header: &str,
) -> String {
	TextDiff::from_lines(current, desired)
		.unified_diff()
		.context_radius(3)
		.header(old_header, new_header)
		.to_string()
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;

	#[rstest]
	#[case(
		"/tmp/LIVE-123456/v1.Namespace..my-namespace",
		"v1.Namespace..my-namespace"
	)]
	#[case(
		"/tmp/MERGED-789012/v1.Namespace..my-namespace",
		"v1.Namespace..my-namespace"
	)]
	#[case("a/v1.Namespace..my-namespace", "v1.Namespace..my-namespace")]
	#[case("b/v1.Namespace..my-namespace", "v1.Namespace..my-namespace")]
	#[case("/dev/null", "/dev/null")]
	fn test_extract_resource_name(#[case] path: &str, #[case] expected: &str) {
		assert_eq!(extract_resource_name(path), expected);
	}

	#[test]
	fn test_parsed_unified_diff_same_content_different_format() {
		let tk_diff = r#"diff -u -N /tmp/LIVE-123/v1.Namespace..my-namespace /tmp/MERGED-456/v1.Namespace..my-namespace
--- /tmp/LIVE-123/v1.Namespace..my-namespace	2024-01-01 00:00:00.000000000 +0000
+++ /tmp/MERGED-456/v1.Namespace..my-namespace	2024-01-01 00:00:00.000000000 +0000
@@ -1,5 +1,5 @@
 apiVersion: v1
 kind: Namespace
 metadata:
-  team: backend
+  team: platform
"#;

		let rtk_diff = r#"--- a/v1.Namespace..my-namespace
+++ b/v1.Namespace..my-namespace
@@ -2,4 +2,4 @@
 kind: Namespace
 metadata:
-  team: backend
+  team: platform
"#;

		let parsed1 = ParsedUnifiedDiff::parse(tk_diff).unwrap();
		let parsed2 = ParsedUnifiedDiff::parse(rtk_diff).unwrap();
		assert_eq!(parsed1, parsed2);
	}
}
