use std::path::{Path, PathBuf};

use jrsonnet_parser::{CodeLocation, Source};

use crate::{error::Error, LocError, State};

/// The way paths should be displayed
pub enum PathResolver {
	/// Only filename
	FileName,
	/// Absolute path
	Absolute,
	/// Path relative to base directory
	Relative(PathBuf),
}

impl PathResolver {
	pub fn resolve(&self, from: &Path) -> String {
		match self {
			Self::FileName => from
				.file_name()
				.expect("file name exists")
				.to_string_lossy()
				.into_owned(),
			Self::Absolute => from.to_string_lossy().into_owned(),
			Self::Relative(base) => {
				if from.is_relative() {
					return from.to_string_lossy().into_owned();
				}
				pathdiff::diff_paths(from, base)
					.expect("base is absolute")
					.to_string_lossy()
					.into_owned()
			}
		}
	}
}

/// Implements pretty-printing of traces
#[allow(clippy::module_name_repetitions)]
pub trait TraceFormat {
	fn write_trace(
		&self,
		out: &mut dyn std::fmt::Write,
		s: &State,
		error: &LocError,
	) -> Result<(), std::fmt::Error>;
}

fn print_code_location(
	out: &mut impl std::fmt::Write,
	start: &CodeLocation,
	end: &CodeLocation,
) -> Result<(), std::fmt::Error> {
	if start.line == end.line {
		if start.column == end.column {
			write!(out, "{}:{}", start.line, end.column.saturating_sub(1))?;
		} else {
			write!(out, "{}:{}-{}", start.line, start.column - 1, end.column)?;
		}
	} else {
		write!(
			out,
			"{}:{}-{}:{}",
			start.line,
			end.column.saturating_sub(1),
			start.line,
			end.column
		)?;
	}
	Ok(())
}

/// vanilla-like jsonnet formatting
pub struct CompactFormat {
	pub resolver: PathResolver,
	pub padding: usize,
}

impl TraceFormat for CompactFormat {
	fn write_trace(
		&self,
		out: &mut dyn std::fmt::Write,
		_s: &State,
		error: &LocError,
	) -> Result<(), std::fmt::Error> {
		write!(out, "{}", error.error())?;
		if let Error::ImportSyntaxError { path, error } = error.error() {
			use std::fmt::Write;

			writeln!(out)?;
			let mut n = match path.path() {
				Some(r) => self.resolver.resolve(r),
				None => path.short_display().to_string(),
			};
			let mut offset = error.location.offset;
			let is_eof = if offset >= path.code().len() {
				offset = path.code().len().saturating_sub(1);
				true
			} else {
				false
			};
			let mut location = path
				.map_source_locations(&[offset as u32])
				.into_iter()
				.next()
				.unwrap();
			if is_eof {
				location.column += 1;
			}

			write!(n, ":").unwrap();
			print_code_location(&mut n, &location, &location).unwrap();
			write!(out, "{:<p$}{}", "", n, p = self.padding,)?;
		}
		let file_names = error
			.trace()
			.0
			.iter()
			.map(|el| &el.location)
			.map(|location| {
				use std::fmt::Write;
				#[allow(clippy::option_if_let_else)]
				if let Some(location) = location {
					let mut resolved_path = match location.0.path() {
						Some(r) => self.resolver.resolve(r),
						None => location.0.short_display().to_string(),
					};
					// TODO: Process all trace elements first
					let location = location.0.map_source_locations(&[location.1, location.2]);
					write!(resolved_path, ":").unwrap();
					print_code_location(&mut resolved_path, &location[0], &location[1]).unwrap();
					write!(resolved_path, ":").unwrap();
					Some(resolved_path)
				} else {
					None
				}
			})
			.collect::<Vec<_>>();
		let align = file_names
			.iter()
			.flatten()
			.map(String::len)
			.max()
			.unwrap_or(0);
		for (el, file) in error.trace().0.iter().zip(file_names) {
			writeln!(out)?;
			if let Some(file) = file {
				write!(
					out,
					"{:<p$}{:<w$} {}",
					"",
					file,
					el.desc,
					p = self.padding,
					w = align
				)?;
			} else {
				write!(out, "{:<p$}{}", "", el.desc, p = self.padding,)?;
			}
		}
		Ok(())
	}
}

pub struct JsFormat;
impl TraceFormat for JsFormat {
	fn write_trace(
		&self,
		out: &mut dyn std::fmt::Write,
		_s: &State,
		error: &LocError,
	) -> Result<(), std::fmt::Error> {
		write!(out, "{}", error.error())?;
		for item in &error.trace().0 {
			writeln!(out)?;
			let desc = &item.desc;
			if let Some(source) = &item.location {
				let start_end = source.0.map_source_locations(&[source.1, source.2]);
				let resolved_path = match source.0.path() {
					Some(r) => r.display().to_string(),
					None => source.0.short_display().to_string(),
				};

				write!(
					out,
					"    at {} ({}:{}:{})",
					desc, resolved_path, start_end[0].line, start_end[0].column,
				)?;
			} else {
				write!(out, "    during {}", desc)?;
			}
		}
		Ok(())
	}
}

/// rustc-like trace displaying
#[cfg(feature = "explaining-traces")]
pub struct ExplainingFormat {
	pub resolver: PathResolver,
}
#[cfg(feature = "explaining-traces")]
impl TraceFormat for ExplainingFormat {
	fn write_trace(
		&self,
		out: &mut dyn std::fmt::Write,
		_s: &State,
		error: &LocError,
	) -> Result<(), std::fmt::Error> {
		write!(out, "{}", error.error())?;
		if let Error::ImportSyntaxError { path, error } = error.error() {
			writeln!(out)?;
			let offset = error.location.offset;
			let location = path
				.map_source_locations(&[offset as u32])
				.into_iter()
				.next()
				.unwrap();
			let mut end_location = location.clone();
			end_location.offset += 1;

			self.print_snippet(
				out,
				path.code(),
				path,
				&location,
				&end_location,
				"syntax error",
			)?;
		}
		let trace = &error.trace();
		for item in &trace.0 {
			writeln!(out)?;
			let desc = &item.desc;
			if let Some(source) = &item.location {
				let start_end = source.0.map_source_locations(&[source.1, source.2]);
				self.print_snippet(
					out,
					source.0.code(),
					&source.0,
					&start_end[0],
					&start_end[1],
					desc,
				)?;
			} else {
				write!(out, "{}", desc)?;
			}
		}
		Ok(())
	}
}

impl ExplainingFormat {
	fn print_snippet(
		&self,
		out: &mut dyn std::fmt::Write,
		source: &str,
		origin: &Source,
		start: &CodeLocation,
		end: &CodeLocation,
		desc: &str,
	) -> Result<(), std::fmt::Error> {
		use annotate_snippets::{
			display_list::{DisplayList, FormatOptions},
			snippet::{AnnotationType, Slice, Snippet, SourceAnnotation},
		};

		let source_fragment: String = source
			.chars()
			.skip(start.line_start_offset)
			.take(end.line_end_offset - end.line_start_offset)
			.collect();

		let origin = match origin.path() {
			Some(r) => self.resolver.resolve(r),
			None => origin.short_display().to_string(),
		};
		let snippet = Snippet {
			opt: FormatOptions {
				color: true,
				..FormatOptions::default()
			},
			title: None,
			footer: vec![],
			slices: vec![Slice {
				source: &source_fragment,
				line_start: start.line,
				origin: Some(&origin),
				fold: false,
				annotations: vec![SourceAnnotation {
					label: desc,
					annotation_type: AnnotationType::Error,
					range: (
						start.offset - start.line_start_offset,
						(end.offset - start.line_start_offset).min(source_fragment.len()),
					),
				}],
			}],
		};

		let dl = DisplayList::from(snippet);
		write!(out, "{}", dl)?;

		Ok(())
	}
}
