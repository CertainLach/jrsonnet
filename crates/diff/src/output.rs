//! Diff output formatting with optional syntax highlighting.

use std::{io::Write, sync::OnceLock};

use nu_ansi_term::{Color, Style};
use syntect::{
	easy::HighlightLines,
	highlighting::{self, FontStyle, Theme, ThemeSet},
	parsing::{SyntaxReference, SyntaxSet},
};
use thiserror::Error;

/// Color output mode for diff display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
	/// Color if stdout is a TTY.
	#[default]
	Auto,
	/// Always emit ANSI color codes.
	Always,
	/// No colors (plain text).
	Never,
}

impl ColorMode {
	/// Determine if colors should be used based on mode and terminal detection.
	pub fn should_colorize(&self) -> bool {
		match self {
			ColorMode::Auto => std::io::IsTerminal::is_terminal(&std::io::stdout()),
			ColorMode::Always => true,
			ColorMode::Never => false,
		}
	}
}

/// Visual style for summary sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionTone {
	Green,
	Yellow,
	Red,
	Cyan,
}

/// Errors that can occur during diff output.
#[derive(Debug, Error)]
pub enum OutputError {
	#[error("writing diff output")]
	Write(#[from] std::io::Error),

	#[error("syntax highlighting not available: {0}")]
	SyntaxNotFound(String),
}

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

fn to_ansi_color(color: highlighting::Color, true_color: bool) -> Option<Color> {
	if color.a == 0 {
		return Some(match color.r {
			0x00 => Color::Black,
			0x01 => Color::Red,
			0x02 => Color::Green,
			0x03 => Color::Yellow,
			0x04 => Color::Blue,
			0x05 => Color::Purple,
			0x06 => Color::Cyan,
			0x07 => Color::White,
			n => Color::Fixed(n),
		});
	}
	if color.a == 1 {
		return None;
	}
	if true_color {
		return Some(Color::Rgb(color.r, color.g, color.b));
	}
	Some(Color::Rgb(color.r, color.g, color.b))
}

fn as_terminal_escaped(
	style: highlighting::Style,
	text: &str,
	true_color: bool,
	colored: bool,
) -> String {
	if text.is_empty() || !colored {
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

/// Shared diff output formatter with optional syntax highlighting.
pub struct DiffOutput<W: Write> {
	writer: W,
	use_color: bool,
	diff_syntax: &'static SyntaxReference,
	theme: Theme,
}

impl<W: Write> DiffOutput<W> {
	/// Create a new diff output formatter.
	pub fn new(writer: W, color_mode: ColorMode) -> Result<Self, OutputError> {
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
		})
	}

	/// Write a unified diff.
	pub fn write_unified_diff(&mut self, diff: &str) -> Result<(), OutputError> {
		self.write_highlighted(diff)
	}

	/// Write a summary section (e.g. Added/Modified/Deleted).
	pub fn write_section(
		&mut self,
		title: &str,
		items: &[String],
		tone: SectionTone,
	) -> Result<(), OutputError> {
		let color = match tone {
			SectionTone::Green => Color::Green,
			SectionTone::Yellow => Color::Yellow,
			SectionTone::Red => Color::Red,
			SectionTone::Cyan => Color::Cyan,
		};

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

	/// Write the trailing total line for summary mode.
	pub fn write_total_changes(&mut self, total_changes: usize) -> Result<(), OutputError> {
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
				Err(_) => writeln!(self.writer, "{}", line)?,
			}
		}

		Ok(())
	}
}
