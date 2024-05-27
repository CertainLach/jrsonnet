use std::{
	any::Any,
	cell::RefCell,
	path::{Path, PathBuf},
};

use jrsonnet_gcmodule::Trace;
use jrsonnet_parser::{CodeLocation, Source, Span};

use crate::{error::ErrorKind, Error};

/// The way paths should be displayed
#[derive(Clone, Trace)]
pub enum PathResolver {
	/// Only filename
	FileName,
	/// Absolute path
	Absolute,
	/// Path relative to base directory
	Relative(PathBuf),
}

impl PathResolver {
	/// Will return `Self::Relative(cwd)`, or `Self::Absolute` on cwd failure
	pub fn new_cwd_fallback() -> Self {
		std::env::current_dir().map_or(Self::Absolute, Self::Relative)
	}
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
pub trait TraceFormat: Trace {
	fn write_trace(
		&self,
		out: &mut dyn std::fmt::Write,
		error: &Error,
	) -> Result<(), std::fmt::Error>;
	fn format(&self, error: &Error) -> Result<String, std::fmt::Error> {
		let mut out = String::new();
		self.write_trace(&mut out, error)?;
		Ok(out)
	}
	fn as_any(&self) -> &dyn Any;
	fn as_any_mut(&mut self) -> &mut dyn Any;
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
#[derive(Trace)]
pub struct CompactFormat {
	pub resolver: PathResolver,
	pub max_trace: usize,
	pub padding: usize,
}
impl Default for CompactFormat {
	fn default() -> Self {
		Self {
			resolver: PathResolver::Absolute,
			max_trace: 20,
			padding: 4,
		}
	}
}

impl TraceFormat for CompactFormat {
	fn write_trace(
		&self,
		out: &mut dyn std::fmt::Write,
		error: &Error,
	) -> Result<(), std::fmt::Error> {
		write!(out, "{}", error.error())?;
		if let ErrorKind::ImportSyntaxError { path, error } = error.error() {
			use std::fmt::Write;

			writeln!(out)?;
			let mut n = path.source_path().path().map_or_else(
				|| path.source_path().to_string(),
				|r| self.resolver.resolve(r),
			);
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
			write!(out, "{:<p$}{n}", "", p = self.padding)?;
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
					let mut resolved_path = match location.0.source_path().path() {
						Some(r) => self.resolver.resolve(r),
						None => location.0.source_path().to_string(),
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

	fn as_any(&self) -> &dyn Any {
		self
	}

	fn as_any_mut(&mut self) -> &mut dyn Any {
		self
	}
}

#[derive(Trace)]
pub struct JsFormat {
	pub max_trace: usize,
}
impl TraceFormat for JsFormat {
	fn write_trace(
		&self,
		out: &mut dyn std::fmt::Write,
		error: &Error,
	) -> Result<(), std::fmt::Error> {
		write!(out, "{}", error.error())?;
		for item in &error.trace().0 {
			writeln!(out)?;
			let desc = &item.desc;
			if let Some(source) = &item.location {
				let start_end = source.0.map_source_locations(&[source.1, source.2]);
				let resolved_path = source.0.source_path().path().map_or_else(
					|| source.0.source_path().to_string(),
					|r| r.display().to_string(),
				);

				write!(
					out,
					"    at {} ({}:{}:{})",
					desc, resolved_path, start_end[0].line, start_end[0].column,
				)?;
			} else {
				write!(out, "    during {desc}")?;
			}
		}
		Ok(())
	}

	fn as_any(&self) -> &dyn Any {
		self
	}

	fn as_any_mut(&mut self) -> &mut dyn Any {
		self
	}
}

/// rustc-like trace displaying
#[cfg(feature = "explaining-traces")]
#[derive(Trace)]
pub struct ExplainingFormat {
	pub resolver: PathResolver,
	pub max_trace: usize,
}
#[cfg(feature = "explaining-traces")]
impl TraceFormat for ExplainingFormat {
	fn write_trace(
		&self,
		out: &mut dyn std::fmt::Write,
		error: &Error,
	) -> Result<(), std::fmt::Error> {
		write!(out, "{}", error.error())?;
		if let ErrorKind::ImportSyntaxError { path, error } = error.error() {
			writeln!(out)?;
			let offset = error.location.offset;
			let location = path
				.map_source_locations(&[offset as u32])
				.into_iter()
				.next()
				.unwrap();
			let mut end_location = location;
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
				write!(out, "{desc}")?;
			}
		}
		Ok(())
	}

	fn as_any(&self) -> &dyn Any {
		self
	}

	fn as_any_mut(&mut self) -> &mut dyn Any {
		self
	}
}

#[cfg(feature = "explaining-traces")]
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
			// DisplayList, FormatOptions,
			AnnotationType,
			Renderer,
			Slice,
			Snippet,
			SourceAnnotation,
		};

		let source_fragment: String = source
			.chars()
			.skip(start.line_start_offset)
			.take(end.line_end_offset - end.line_start_offset)
			.collect();

		let origin = origin.source_path().path().map_or_else(
			|| origin.source_path().to_string(),
			|r| self.resolver.resolve(r),
		);
		let snippet = Snippet {
			// opt: FormatOptions {
			// 	color: true,
			// 	..FormatOptions::default()
			// },
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
						(end.offset.saturating_sub(start.line_start_offset))
							.min(source_fragment.len()),
					),
				}],
			}],
		};

		let renderer = Renderer::styled();
		let dl = renderer.render(snippet);
		write!(out, "{dl}")?;

		Ok(())
	}
}

#[cfg(feature = "explaining-traces")]
#[derive(Trace)]
pub struct AssStrokeFormat {
	pub resolver: PathResolver,
	pub max_trace: usize,
}
#[cfg(feature = "explaining-traces")]
impl TraceFormat for AssStrokeFormat {
	fn write_trace(
		&self,
		out: &mut dyn std::fmt::Write,
		error: &Error,
	) -> Result<(), std::fmt::Error> {
		struct ResetData {
			loc: Span,
		}
		use hi_doc::{source_to_ansi, Formatting, SnippetBuilder, Text};

		write!(out, "{}", error.error())?;
		if let ErrorKind::ImportSyntaxError { path, error } = error.error() {
			writeln!(out)?;
			let offset = error.location.offset;
			let mut builder = SnippetBuilder::new(path.code());
			builder
				.error(Text::single("syntax error".chars(), Formatting::default()))
				.range(offset..=offset)
				.build();
			let source = builder.build();
			let ansi = source_to_ansi(&source);
			write!(out, "{ansi}")?;
		}
		let trace = &error.trace();
		let snippet_builder: RefCell<Option<SnippetBuilder>> = RefCell::new(None);
		let mut last_location: Option<Span> = None;
		let mut flush_builder = |data: Option<ResetData>| {
			use std::fmt::Write;
			let mut out = String::new();
			let location_changed = if let Some(ResetData { loc }) = &data {
				if last_location.as_ref().map(|l| l.0.code()) != Some(loc.0.code()) {
					true
				} else if let (Some(last), new) = (&last_location, loc) {
					// Reverse condition if traceback
					last.1 > new.1 || last.2 > new.2
				} else {
					false
				}
			} else {
				true
			};
			if location_changed {
				if let Some(builder) = snippet_builder.borrow_mut().take() {
					let rendered = builder.build();
					let ansi = source_to_ansi(&rendered);
					if let Some(loc) = &last_location {
						let _ = writeln!(out, "...because of {}", loc.0.source_path());
					}
					let _ = write!(out, "{}", ansi.trim_end());
				}
				last_location = None;

				if let Some(ResetData { loc }) = data {
					*snippet_builder.borrow_mut() = Some(SnippetBuilder::new(loc.0.code()));
					last_location = Some(loc);
				}
			}
			if out.is_empty() {
				return None;
			}
			Some(out)
		};
		for item in &trace.0 {
			let desc = &item.desc;
			if let Some(source) = &item.location {
				if let Some(flushed) = flush_builder(Some(ResetData {
					loc: source.clone(),
				})) {
					writeln!(out)?;
					write!(out, "{flushed}")?;
				}
				let mut builder = snippet_builder.borrow_mut();
				let builder = builder.as_mut().unwrap();
				builder
					.note(Text::single(desc.chars(), Formatting::default()))
					.range(source.1 as usize..=(source.2 as usize - 1).max(source.1 as usize))
					.build();
			} else {
				if let Some(flushed) = flush_builder(None) {
					writeln!(out)?;
					write!(out, "{flushed}")?;
				}
				write!(out, "{desc}")?;
			}
		}

		if let Some(flushed) = flush_builder(None) {
			writeln!(out)?;
			write!(out, "{flushed}")?;
		}
		Ok(())
	}

	fn as_any(&self) -> &dyn Any {
		self
	}

	fn as_any_mut(&mut self) -> &mut dyn Any {
		self
	}
}
