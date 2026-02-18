//! Lint configuration: enable/disable individual checks.

use crate::checks;

/// Configuration for which lint checks are run.
#[derive(Clone, Debug)]
pub struct LintConfig {
	/// Report local bindings (local x = ..., object locals, for bindings, function params) that are never used.
	pub unused_locals: bool,
}

impl Default for LintConfig {
	fn default() -> Self {
		Self {
			unused_locals: true,
		}
	}
}

impl LintConfig {
	/// All checks enabled (default).
	pub fn all() -> Self {
		Self::default()
	}

	/// Disable the unused locals check.
	#[must_use]
	pub fn with_unused_locals(mut self, enable: bool) -> Self {
		self.unused_locals = enable;
		self
	}

	/// Build config with the given checks disabled. Returns an error if any check name is invalid.
	pub fn with_disabled_checks(mut self, disabled: &[String]) -> Result<Self, String> {
		for name in disabled {
			let name = name.trim();
			if name.is_empty() {
				continue;
			}
			if !checks::ALL_CHECKS.contains(&name) {
				return Err(format!(
					"unknown check '{}'; valid checks: {}",
					name,
					checks::ALL_CHECKS.join(", ")
				));
			}
			if name == checks::UNUSED_LOCALS {
				self.unused_locals = false;
			}
		}
		Ok(self)
	}
}
