//! Environment variable configuration for tk-compare.

use regex::Regex;

/// Environment-based configuration for the comparison tool.
pub struct EnvConfig {
	/// Whether debug mode is enabled (DEBUG=true).
	pub debug: bool,
	/// Maximum lines to show in debug output (DEBUG_MAX_LINES).
	pub debug_max_lines: usize,
	/// Optional command filter regex (COMPARE_REGEXP).
	pub filter_regex: Option<Regex>,
}

impl EnvConfig {
	/// Parse configuration from environment variables.
	pub fn from_env() -> Self {
		let debug = std::env::var("DEBUG").unwrap_or_default() == "true";

		let debug_max_lines = std::env::var("DEBUG_MAX_LINES")
			.ok()
			.and_then(|v| v.parse().ok())
			.unwrap_or(100);

		let filter_regex =
			std::env::var("COMPARE_REGEXP")
				.ok()
				.and_then(|pattern| match Regex::new(&pattern) {
					Ok(re) => Some(re),
					Err(e) => {
						eprintln!(
							"Warning: Invalid COMPARE_REGEXP pattern '{}': {}",
							pattern, e
						);
						None
					}
				});

		Self {
			debug,
			debug_max_lines,
			filter_regex,
		}
	}

	/// Print debug mode status if enabled.
	pub fn print_debug_status(&self) {
		if !self.debug {
			return;
		}

		eprintln!(
			"DEBUG mode enabled (max {} diff lines)",
			self.debug_max_lines
		);

		if let Ok(config) = std::env::var("PRINT_FULL_OBJECTS") {
			if config == "true" {
				eprintln!("PRINT_FULL_OBJECTS: enabled (0 levels - current object)\n");
			} else if let Ok(levels) = config.parse::<usize>() {
				eprintln!("PRINT_FULL_OBJECTS: enabled ({} levels up)\n", levels);
			}
		} else {
			eprintln!();
		}
	}

	/// Print filter status if a filter is configured.
	pub fn print_filter_status(&self) {
		if let Ok(pattern) = std::env::var("COMPARE_REGEXP") {
			if self.filter_regex.is_some() {
				eprintln!("Filtering commands with pattern: {}\n", pattern);
			} else {
				eprintln!("Running all commands\n");
			}
		}
	}
}
