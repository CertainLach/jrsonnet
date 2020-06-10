pub mod location;

use clap::Clap;
use jsonnet_evaluator::{EvaluationState, LocError, StackTrace, Val};
use location::{offset_to_location, CodeLocation};
use std::env::current_dir;
use std::{path::PathBuf, str::FromStr};

enum Format {
	None,
	Json,
	Yaml,
}

impl FromStr for Format {
	type Err = &'static str;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(match s {
			"none" => Format::None,
			"json" => Format::Json,
			"yaml" => Format::Yaml,
			_ => return Err("no such format"),
		})
	}
}

#[derive(PartialEq)]
enum TraceFormat {
	CppJsonnet,
	GoJsonnet,
	Custom,
}
impl FromStr for TraceFormat {
	type Err = &'static str;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(match s {
			"cpp" => TraceFormat::CppJsonnet,
			"go" => TraceFormat::GoJsonnet,
			"default" => TraceFormat::Custom,
			_ => return Err("no such format"),
		})
	}
}

#[derive(Clone)]
struct ExtStr {
	name: String,
	value: String,
}
impl FromStr for ExtStr {
	type Err = &'static str;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let out: Vec<_> = s.split('=').collect();
		match out.len() {
			1 => Ok(ExtStr {
				name: out[0].to_owned(),
				value: std::env::var(out[0]).or(Err("missing env var"))?,
			}),
			2 => Ok(ExtStr {
				name: out[0].to_owned(),
				value: out[1].to_owned(),
			}),
			_ => Err("bad ext-str syntax"),
		}
	}
}

#[derive(Clap)]
#[clap(version = "0.1.0", author = "Lach <iam@lach.pw>")]
struct Opts {
	#[clap(long, about = "Disable global std variable")]
	no_stdlib: bool,
	#[clap(long, about = "Add external string")]
	ext_str: Vec<ExtStr>,
	#[clap(long, about = "Add external string from code")]
	ext_code: Vec<ExtStr>,
	#[clap(long, about = "Add TLA")]
	tla_str: Vec<ExtStr>,
	#[clap(long, about = "Add TLA from code")]
	tla_code: Vec<ExtStr>,
	#[clap(long, short = "f", default_value = "json", possible_values = &["none", "json", "yaml"], about = "Output format, wraps resulting value to corresponding std.manifest call")]
	format: Format,
	#[clap(long, default_value = "default", possible_values = &["cpp", "go", "default"], about = "Emulated needed stacktrace display")]
	trace_format: TraceFormat,

	#[clap(
		long,
		short = "s",
		default_value = "200",
		about = "Number of allowed stack frames"
	)]
	max_stack: usize,
	#[clap(
		long,
		short = "t",
		default_value = "20",
		about = "Max length of stack trace before cropping"
	)]
	max_trace: usize,

	#[clap(
		long,
		default_value = "3",
		about = "When using --format, this option specifies string to pad output with"
	)]
	line_padding: usize,

	#[clap(about = "File to compile", index = 1)]
	input: String,
}

fn main() {
	let opts: Opts = Opts::parse();
	let evaluator = jsonnet_evaluator::EvaluationState::default();
	if !opts.no_stdlib {
		evaluator.with_stdlib();
	}
	for ExtStr { name, value } in opts.ext_str.iter().cloned() {
		evaluator.add_ext_var(name, Val::Str(value));
	}
	for ExtStr { name, value } in opts.ext_code.iter().cloned() {
		evaluator.add_ext_var(name, evaluator.parse_evaluate_raw(&value).unwrap());
	}
	let mut input = current_dir().unwrap();
	input.push(opts.input.clone());
	let code_string = String::from_utf8(std::fs::read(opts.input.clone()).unwrap()).unwrap();
	if let Err(e) = evaluator.add_file(input.clone(), code_string.clone()) {
		print_syntax_error(e, &input, &code_string);
		std::process::exit(1);
	}
	let result = evaluator.evaluate_file(&input);
	match result {
		Ok(v) => {
			let v = match opts.format {
				Format::Json => {
					if opts.no_stdlib {
						evaluator.with_stdlib();
					}
					evaluator.add_global("__tmp__to_json__".to_owned(), v);
					let v = evaluator.parse_evaluate_raw(&format!(
						"std.manifestJsonEx(__tmp__to_json__, \"{}\")",
						" ".repeat(opts.line_padding),
					));
					match v {
						Ok(v) => v,
						Err(err) => {
							print_error(&err, evaluator, &opts);
							std::process::exit(1);
						}
					}
				}
				Format::Yaml => {
					if opts.no_stdlib {
						evaluator.with_stdlib();
					}
					evaluator.add_global("__tmp__to_yaml__".to_owned(), v);
					let v = evaluator
						.parse_evaluate_raw("std.manifestYamlDoc(__tmp__to_yaml__, \"  \")");
					match v {
						Ok(v) => v,
						Err(err) => {
							print_error(&err, evaluator, &opts);
							std::process::exit(1);
						}
					}
				}
				_ => v,
			};
			match v {
				Val::Str(s) => println!("{}", s),
				Val::Num(n) => println!("{}", n),
				_v => eprintln!(
					"jsonnet output is not a string.\nDid you forgot to set --format, or wrap your data with std.manifestJson?"
				),
			}
		}
		Err(err) => {
			print_error(&err, evaluator, &opts);
			std::process::exit(1);
		}
	}
}

fn print_error(err: &LocError, evaluator: EvaluationState, opts: &Opts) {
	println!("Error: {:?}", err.0);
	print_trace(&(err.1), evaluator, &opts);
}

fn print_syntax_error(error: jsonnet_parser::ParseError, file: &PathBuf, code: &str) {
	use annotate_snippets::{
		display_list::{DisplayList, FormatOptions},
		snippet::{Annotation, AnnotationType, Slice, Snippet, SourceAnnotation},
	};
	//&("Expected: ".to_owned() + error.expected)
	let origin = file.to_str().unwrap();
	let error_message = format!("Expected: {}", error.expected);
	let snippet = Snippet {
		opt: FormatOptions {
			color: true,
			..Default::default()
		},
		title: Some(Annotation {
			label: Some(&error_message),
			id: None,
			annotation_type: AnnotationType::Error,
		}),
		footer: vec![],
		slices: vec![Slice {
			source: &code,
			line_start: 1,
			origin: Some(origin),
			fold: false,
			annotations: vec![SourceAnnotation {
				label: "At this position",
				annotation_type: AnnotationType::Error,
				range: (error.location.offset, error.location.offset + 1),
			}],
		}],
	};

	let dl = DisplayList::from(snippet);
	println!("{}", dl);
}

fn print_trace(trace: &StackTrace, evaluator: EvaluationState, opts: &Opts) {
	use annotate_snippets::{
		display_list::{DisplayList, FormatOptions},
		snippet::{Annotation, AnnotationType, Slice, Snippet, SourceAnnotation},
	};
	for item in trace.0.iter() {
		let desc = &item.1;
		if (item.0).1.is_none() {
			continue;
		}
		let source = (item.0).1.clone().unwrap();
		let code = evaluator.get_source(&source.0);
		if code.is_none() {
			continue;
		}
		let code = code.unwrap();
		let start_end = offset_to_location(&code, &[source.1, source.2]);
		if opts.trace_format == TraceFormat::Custom {
			let source_fragment: String = code
				.chars()
				.skip(start_end[0].line_start_offset)
				.take(start_end[1].line_end_offset - start_end[0].line_start_offset)
				.collect();
			let snippet = Snippet {
				opt: FormatOptions {
					color: true,
					..Default::default()
				},
				title: Some(Annotation {
					label: Some(&item.1),
					id: None,
					annotation_type: AnnotationType::Error,
				}),
				footer: vec![],
				slices: vec![Slice {
					source: &source_fragment,
					line_start: start_end[0].line,
					origin: Some(&source.0.to_str().unwrap()),
					fold: false,
					annotations: vec![SourceAnnotation {
						label: desc,
						annotation_type: AnnotationType::Error,
						range: (
							source.1 - start_end[0].line_start_offset,
							source.2 - start_end[0].line_start_offset,
						),
					}],
				}],
			};

			let dl = DisplayList::from(snippet);
			println!("{}", dl);
		} else {
			print_jsonnet_pair(
				source.0.to_str().unwrap(),
				&start_end[0],
				&start_end[1],
				opts.trace_format == TraceFormat::GoJsonnet,
			);
		}
	}
}

fn print_jsonnet_pair(file: &str, start: &CodeLocation, end: &CodeLocation, is_go: bool) {
	if is_go {
		print!("        ");
	} else {
		print!("  ");
	}
	print!("{}:", file);
	if start.line == end.line {
		// IDK why, but this is the behavior original jsonnet cpp impl shows
		if start.column == end.column || !is_go && start.column + 1 == end.column {
			println!("{}:{}", start.line, end.column)
		} else {
			println!("{}:{}-{}", start.line, start.column, end.column);
		}
	} else {
		println!(
			"({}:{})-({}:{})",
			start.line, end.column, start.line, end.column
		);
	}
}
