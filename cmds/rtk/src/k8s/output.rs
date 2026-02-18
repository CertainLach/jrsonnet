//! Diff output adapter for `rtk` built on shared `rtk_diff::output`.

use std::io::Write;

use rtk_diff::output::{ColorMode as SharedColorMode, OutputError, SectionTone};

use super::diff::{DiffStatus, ResourceDiff};
use crate::{commands::diff::ColorMode, spec::DiffStrategy};

impl From<ColorMode> for SharedColorMode {
	fn from(value: ColorMode) -> Self {
		match value {
			ColorMode::Auto => SharedColorMode::Auto,
			ColorMode::Always => SharedColorMode::Always,
			ColorMode::Never => SharedColorMode::Never,
		}
	}
}

/// Handles diff output formatting with optional color.
pub struct DiffOutput<W: Write> {
	inner: rtk_diff::output::DiffOutput<W>,
	strategy: DiffStrategy,
}

impl<W: Write> DiffOutput<W> {
	/// Create a new diff output handler.
	pub fn new(
		writer: W,
		color_mode: ColorMode,
		strategy: DiffStrategy,
	) -> Result<Self, OutputError> {
		Ok(Self {
			inner: rtk_diff::output::DiffOutput::new(writer, color_mode.into())?,
			strategy,
		})
	}

	/// Write a single resource diff.
	pub fn write_diff(&mut self, diff: &ResourceDiff) -> Result<(), OutputError> {
		match diff.status {
			DiffStatus::SoonAdded => {
				self.inner
					.write_unified_diff("(namespace not yet created)\n")?;
				self.inner
					.write_unified_diff(&diff.unified_diff(self.strategy))?;
			}
			DiffStatus::Added | DiffStatus::Modified | DiffStatus::Deleted => {
				self.inner
					.write_unified_diff(&diff.unified_diff(self.strategy))?;
			}
			DiffStatus::Unchanged => {}
		}
		Ok(())
	}

	/// Write summary mode output (just resource names and statuses).
	pub fn write_summary(&mut self, diffs: &[ResourceDiff]) -> Result<(), OutputError> {
		let mut added = Vec::new();
		let mut modified = Vec::new();
		let mut deleted = Vec::new();
		let mut soon_added = Vec::new();

		for diff in diffs {
			let name = match self.strategy {
				DiffStrategy::Subset => diff.display_name_subset(),
				_ => diff.display_name(),
			};
			match diff.status {
				DiffStatus::Added => added.push(name),
				DiffStatus::Modified => modified.push(name),
				DiffStatus::Deleted => deleted.push(name),
				DiffStatus::SoonAdded => soon_added.push(name),
				DiffStatus::Unchanged => {}
			}
		}

		if !added.is_empty() {
			self.inner
				.write_section("Added", &added, SectionTone::Green)?;
		}
		if !modified.is_empty() {
			self.inner
				.write_section("Modified", &modified, SectionTone::Yellow)?;
		}
		if !deleted.is_empty() {
			self.inner
				.write_section("Deleted", &deleted, SectionTone::Red)?;
		}
		if !soon_added.is_empty() {
			self.inner.write_section(
				"Soon Added (namespace pending)",
				&soon_added,
				SectionTone::Cyan,
			)?;
		}

		self.inner
			.write_total_changes(added.len() + modified.len() + deleted.len() + soon_added.len())?;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use indoc::indoc;

	use super::*;

	fn make_test_diffs() -> Vec<ResourceDiff> {
		use kube::core::GroupVersionKind;

		vec![
			ResourceDiff {
				gvk: GroupVersionKind::gvk("", "v1", "ConfigMap"),
				namespace: Some("default".to_string()),
				name: "test-config".to_string(),
				status: DiffStatus::Added,
				current_yaml: String::new(),
				desired_yaml: "apiVersion: v1\nkind: ConfigMap".to_string(),
			},
			ResourceDiff {
				gvk: GroupVersionKind::gvk("apps", "v1", "Deployment"),
				namespace: Some("default".to_string()),
				name: "test-deploy".to_string(),
				status: DiffStatus::Modified,
				current_yaml: "old".to_string(),
				desired_yaml: "new".to_string(),
			},
		]
	}

	#[test]
	fn test_diff_output_summary_native() {
		let mut output = Vec::new();
		let mut diff_output =
			DiffOutput::new(&mut output, ColorMode::Never, DiffStrategy::Native).unwrap();

		diff_output.write_summary(&make_test_diffs()).unwrap();

		let output_str = String::from_utf8(output).unwrap();
		assert_eq!(
			output_str,
			indoc! {"

                Added:
                  v1.ConfigMap.default.test-config

                Modified:
                  apps.v1.Deployment.default.test-deploy

                Total: 2 resource(s) with changes
            "}
		);
	}

	#[test]
	fn test_diff_output_summary_subset() {
		let mut output = Vec::new();
		let mut diff_output =
			DiffOutput::new(&mut output, ColorMode::Never, DiffStrategy::Subset).unwrap();

		diff_output.write_summary(&make_test_diffs()).unwrap();

		let output_str = String::from_utf8(output).unwrap();
		assert_eq!(
			output_str,
			indoc! {"

                Added:
                  v1.ConfigMap.default.test-config

                Modified:
                  apps-v1.Deployment.default.test-deploy

                Total: 2 resource(s) with changes
            "}
		);
	}
}
