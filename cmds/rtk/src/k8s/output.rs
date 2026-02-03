//! Diff output formatting with optional syntax highlighting.
//!
//! This module handles formatting diff output with ANSI colors,
//! using syntect for YAML syntax highlighting combined with
//! diff-specific coloring (red for deletions, green for additions).
//!
//! The approach is modeled on bat's terminal.rs.

use std::{io::Write, sync::OnceLock};

use nu_ansi_term::{Color, Style};
use thiserror::Error;

/// Errors that can occur during diff output.
#[derive(Debug, Error)]
pub enum OutputError {
	#[error("writing diff output")]
	Write(#[from] std::io::Error),

	#[error("syntax highlighting not available: {0}")]
	SyntaxNotFound(String),
}
use syntect::{
	easy::HighlightLines,
	highlighting::{self, FontStyle, Theme, ThemeSet},
	parsing::{SyntaxReference, SyntaxSet},
};
use tracing::instrument;

use super::diff::{DiffStatus, ResourceDiff};
use crate::{commands::diff::ColorMode, spec::DiffStrategy};

/// Lazy-loaded syntect syntax set.
static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();

/// Lazy-loaded syntect theme set.
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

fn syntax_set() -> &'static SyntaxSet {
	SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme_set() -> &'static ThemeSet {
	THEME_SET.get_or_init(ThemeSet::load_defaults)
}

/// Convert a syntect color to a nu_ansi_term color.
///
/// Based on bat's terminal.rs `to_ansi_color()`.
fn to_ansi_color(color: highlighting::Color, true_color: bool) -> Option<Color> {
	if color.a == 0 {
		// Themes can specify terminal colors by encoding them with alpha=0
		// and the palette number in the red channel.
		Some(match color.r {
			0x00 => Color::Black,
			0x01 => Color::Red,
			0x02 => Color::Green,
			0x03 => Color::Yellow,
			0x04 => Color::Blue,
			0x05 => Color::Purple,
			0x06 => Color::Cyan,
			0x07 => Color::White,
			n => Color::Fixed(n),
		})
	} else if color.a == 1 {
		// Alpha=1 means use terminal's default color (no escape sequence)
		None
	} else if true_color {
		Some(Color::Rgb(color.r, color.g, color.b))
	} else {
		// Fall back to 256-color palette
		// For simplicity, we just use true color here since most modern terminals support it
		Some(Color::Rgb(color.r, color.g, color.b))
	}
}

/// Convert a syntect style to terminal-escaped text.
///
/// Based on bat's terminal.rs `as_terminal_escaped()`.
fn as_terminal_escaped(
	style: highlighting::Style,
	text: &str,
	true_color: bool,
	colored: bool,
) -> String {
	if text.is_empty() {
		return text.to_string();
	}

	if !colored {
		return text.to_string();
	}

	let mut ansi_style = Style {
		foreground: to_ansi_color(style.foreground, true_color),
		..Style::default()
	};

	if style.font_style.contains(FontStyle::BOLD) {
		ansi_style = ansi_style.bold();
	}
	if style.font_style.contains(FontStyle::UNDERLINE) {
		ansi_style = ansi_style.underline();
	}
	if style.font_style.contains(FontStyle::ITALIC) {
		ansi_style = ansi_style.italic();
	}

	ansi_style.paint(text).to_string()
}

/// Handles diff output formatting with optional color.
pub struct DiffOutput<W: Write> {
	writer: W,
	use_color: bool,
	diff_syntax: &'static SyntaxReference,
	theme: Theme,
	strategy: DiffStrategy,
}

impl<W: Write> DiffOutput<W> {
	/// Create a new diff output handler.
	pub fn new(
		writer: W,
		color_mode: ColorMode,
		strategy: DiffStrategy,
	) -> Result<Self, OutputError> {
		let ss = syntax_set();
		let diff_syntax = ss
			.find_syntax_by_extension("diff")
			.ok_or_else(|| OutputError::SyntaxNotFound("diff".to_string()))?;

		let ts = theme_set();
		let theme = ts.themes["base16-ocean.dark"].clone();

		let use_color = color_mode.should_colorize();

		Ok(Self {
			writer,
			use_color,
			diff_syntax,
			theme,
			strategy,
		})
	}

	/// Write a single resource diff.
	#[instrument(skip_all, fields(resource = %diff.display_name()))]
	pub fn write_diff(&mut self, diff: &ResourceDiff) -> Result<(), OutputError> {
		match diff.status {
			DiffStatus::SoonAdded => {
				writeln!(self.writer, "(namespace not yet created)")?;
				self.write_unified_diff(&diff.unified_diff(self.strategy))?;
			}
			DiffStatus::Added | DiffStatus::Modified | DiffStatus::Deleted => {
				self.write_unified_diff(&diff.unified_diff(self.strategy))?;
			}
			DiffStatus::Unchanged => {
				// Nothing to display
			}
		}
		Ok(())
	}

	/// Write summary mode output (just resource names and statuses).
	#[instrument(skip_all, fields(diff_count = diffs.len()))]
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
			self.write_section("Added", &added, Color::Green)?;
		}

		if !modified.is_empty() {
			self.write_section("Modified", &modified, Color::Yellow)?;
		}

		if !deleted.is_empty() {
			self.write_section("Deleted", &deleted, Color::Red)?;
		}

		if !soon_added.is_empty() {
			self.write_section("Soon Added (namespace pending)", &soon_added, Color::Cyan)?;
		}

		// Write totals
		let total_changes = added.len() + modified.len() + deleted.len() + soon_added.len();
		writeln!(self.writer)?;

		if self.use_color {
			writeln!(
				self.writer,
				"{}",
				Style::new()
					.bold()
					.paint(format!("Total: {} resource(s) with changes", total_changes))
			)?;

			return Ok(());
		}

		writeln!(
			self.writer,
			"Total: {} resource(s) with changes",
			total_changes
		)?;

		Ok(())
	}

	/// Write a unified diff with syntax highlighting.
	fn write_unified_diff(&mut self, diff: &str) -> Result<(), OutputError> {
		self.write_highlighted(diff)
	}

	/// Write content with syntax highlighting using the diff syntax.
	#[instrument(skip_all)]
	fn write_highlighted(&mut self, content: &str) -> Result<(), OutputError> {
		if !self.use_color {
			write!(self.writer, "{}", content)?;
			return Ok(());
		}

		let ss = syntax_set();
		let mut highlighter = HighlightLines::new(self.diff_syntax, &self.theme);

		for line in content.lines() {
			match highlighter.highlight_line(line, ss) {
				Ok(regions) => {
					for (style, text) in regions {
						write!(
							self.writer,
							"{}",
							as_terminal_escaped(style, text, true, true)
						)?;
					}
					writeln!(self.writer)?;
				}
				Err(_) => {
					writeln!(self.writer, "{}", line)?;
				}
			}
		}

		Ok(())
	}

	/// Write a section in summary mode.
	fn write_section(
		&mut self,
		title: &str,
		items: &[String],
		color: Color,
	) -> Result<(), OutputError> {
		if self.use_color {
			writeln!(
				self.writer,
				"\n{}",
				Style::new().bold().fg(color).paint(format!("{}:", title))
			)?;
		} else {
			writeln!(self.writer, "\n{}:", title)?;
		}

		for item in items {
			if self.use_color {
				writeln!(self.writer, "  {}", Style::new().fg(color).paint(item))?;
			} else {
				writeln!(self.writer, "  {}", item)?;
			}
		}

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use indoc::indoc;

	use super::*;

	#[test]
	fn test_color_mode_auto() {
		// In tests, stdout is not a terminal
		let mode = ColorMode::Auto;
		// This would be false in test environment
		let _ = mode.should_colorize();
	}

	#[test]
	fn test_color_mode_always() {
		let mode = ColorMode::Always;
		assert!(mode.should_colorize());
	}

	#[test]
	fn test_color_mode_never() {
		let mode = ColorMode::Never;
		assert!(!mode.should_colorize());
	}

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

	#[test]
	fn test_to_ansi_color_palette() {
		// Test terminal palette colors (alpha=0)
		let color = highlighting::Color {
			r: 0x01,
			g: 0,
			b: 0,
			a: 0,
		};
		assert_eq!(to_ansi_color(color, true), Some(Color::Red));

		let color = highlighting::Color {
			r: 0x02,
			g: 0,
			b: 0,
			a: 0,
		};
		assert_eq!(to_ansi_color(color, true), Some(Color::Green));
	}

	#[test]
	fn test_to_ansi_color_default() {
		// Test default color (alpha=1 means no escape sequence)
		let color = highlighting::Color {
			r: 255,
			g: 255,
			b: 255,
			a: 1,
		};
		assert_eq!(to_ansi_color(color, true), None);
	}

	#[test]
	fn test_to_ansi_color_rgb() {
		// Test true color RGB
		let color = highlighting::Color {
			r: 100,
			g: 150,
			b: 200,
			a: 255,
		};
		assert_eq!(to_ansi_color(color, true), Some(Color::Rgb(100, 150, 200)));
	}
}
