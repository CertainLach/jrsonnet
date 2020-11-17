mod location;

use crate::{error::Error, EvaluationState, LocError};
pub use location::*;
use std::path::PathBuf;

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
	pub fn resolve(&self, from: &PathBuf) -> String {
		match self {
			Self::FileName => from.file_name().unwrap().to_string_lossy().into_owned(),
			Self::Absolute => from.to_string_lossy().into_owned(),
			Self::Relative(base) => {
				if from.is_relative() {
					return from.to_string_lossy().into_owned();
				}
				pathdiff::diff_paths(from, base)
					.unwrap()
					.to_string_lossy()
					.into_owned()
			}
		}
	}
}

/// Implements pretty-printing of traces
pub trait TraceFormat {
	fn write_trace(
		&self,
		out: &mut dyn std::fmt::Write,
		evaluation_state: &EvaluationState,
		error: &LocError,
	) -> Result<(), std::fmt::Error>;
	// fn print_trace(
	// 	&self,
	// 	evaluation_state: &EvaluationState,
	// 	error: &LocError,
	// ) -> Result<(), std::fmt::Error> {
	// 	self.write_trace(&mut std::fmt::stdout(), evaluation_state, error)
	// }
}

fn print_code_location(
	out: &mut impl std::fmt::Write,
	start: &CodeLocation,
	end: &CodeLocation,
) -> Result<(), std::fmt::Error> {
	if start.line == end.line {
		if start.column == end.column {
			write!(out, "{}:{}", start.line, end.column - 1)?;
		} else {
			write!(out, "{}:{}-{}", start.line, start.column - 1, end.column)?;
		}
	} else {
		write!(
			out,
			"{}:{}-{}:{}",
			start.line,
			end.column - 1,
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
		evaluation_state: &EvaluationState,
		error: &LocError,
	) -> Result<(), std::fmt::Error> {
		writeln!(out, "{}", error.error())?;
		if let Error::ImportSyntaxError {
			path,
			source_code,
			error,
		} = error.error()
		{
			use std::fmt::Write;
			let mut n = self.resolver.resolve(path);
			let mut offset = error.location.offset;
			let is_eof = if offset >= source_code.len() {
				offset = source_code.len() - 1;
				true
			} else {
				false
			};
			let mut location = offset_to_location(source_code, &[offset])
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
			.map(|el| {
				let resolved_path = self.resolver.resolve(&el.location.0);
				// TODO: Process all trace elements first
				let location = evaluation_state
					.map_source_locations(&el.location.0, &[el.location.1, el.location.2]);
				(resolved_path, location)
			})
			.map(|(mut n, location)| {
				use std::fmt::Write;
				write!(n, ":").unwrap();
				print_code_location(&mut n, &location[0], &location[1]).unwrap();
				n
			})
			.collect::<Vec<_>>();
		let align = file_names.iter().map(|e| e.len()).max().unwrap_or(0);
		for (i, (el, file)) in error.trace().0.iter().zip(file_names).enumerate() {
			if i != 0 {
				writeln!(out)?;
			}
			write!(
				out,
				"{:<p$}{:<w$}: {}",
				"",
				file,
				el.desc,
				p = self.padding,
				w = align
			)?;
		}
		Ok(())
	}
}

pub struct JSFormat;
impl TraceFormat for JSFormat {
	fn write_trace(
		&self,
		out: &mut dyn std::fmt::Write,
		evaluation_state: &EvaluationState,
		error: &LocError,
	) -> Result<(), std::fmt::Error> {
		writeln!(out, "{}", error.error())?;
		for (i, item) in error.trace().0.iter().enumerate() {
			if i != 0 {
				writeln!(out)?;
			}
			let desc = &item.desc;
			let source = item.location.clone();
			let start_end = evaluation_state.map_source_locations(&source.0, &[source.1, source.2]);

			write!(
				out,
				"    at {} ({}:{}:{})",
				desc,
				source.0.to_str().unwrap(),
				start_end[0].line,
				start_end[0].column,
			)?;
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
		evaluation_state: &EvaluationState,
		error: &LocError,
	) -> Result<(), std::fmt::Error> {
		writeln!(out, "{}", error.error())?;
		if let Error::ImportSyntaxError {
			path,
			source_code,
			error,
		} = error.error()
		{
			let mut offset = error.location.offset;
			if offset >= source_code.len() {
				offset = source_code.len() - 1;
			}
			let mut location = offset_to_location(source_code, &[offset])
				.into_iter()
				.next()
				.unwrap();
			if location.column >= 1 {
				location.column -= 1;
			}

			self.print_snippet(
				out,
				source_code,
				path,
				&location,
				&location,
				"^ syntax error",
			)?;
		}
		let trace = &error.trace();
		for item in trace.0.iter() {
			let desc = &item.desc;
			let source = item.location.clone();
			let start_end = evaluation_state.map_source_locations(&source.0, &[source.1, source.2]);

			self.print_snippet(
				out,
				&evaluation_state.get_source(&source.0).unwrap(),
				&source.0,
				&start_end[0],
				&start_end[1],
				desc,
			)?;
		}
		Ok(())
	}
}

impl ExplainingFormat {
	fn print_snippet(
		&self,
		out: &mut dyn std::fmt::Write,
		source: &str,
		origin: &PathBuf,
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

		let origin = self.resolver.resolve(origin);
		let snippet = Snippet {
			opt: FormatOptions {
				color: true,
				..Default::default()
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
						end.offset - start.line_start_offset,
					),
				}],
			}],
		};

		let dl = DisplayList::from(snippet);
		writeln!(out, "{}", dl)?;

		Ok(())
	}
}
