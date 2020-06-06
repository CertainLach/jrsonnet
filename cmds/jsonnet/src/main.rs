pub mod location;

use clap::Clap;
use jsonnet_evaluator::{EvaluationState, LocError, StackTrace, Val};
use location::{offset_to_location, CodeLocation};
use std::env::current_dir;
use std::str::FromStr;

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

#[derive(Clap)]
#[clap(version = "0.1.0", author = "Lach <iam@lach.pw>")]
struct Opts {
	#[clap(long, about = "Disable global std variable")]
	no_stdlib: bool,
	#[clap(long, about = "Add external string")]
	ext_str: Option<Vec<String>>,
	#[clap(long, about = "Add external string from code")]
	ext_code: Option<Vec<String>>,
	#[clap(long, about = "Add TLA")]
	tla_str: Option<Vec<String>>,
	#[clap(long, about = "Add TLA from code")]
	tla_code: Option<Vec<String>>,
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

	#[clap(about = "File to compile", index = 1)]
	input: String,
}

fn main() {
	let opts: Opts = Opts::parse();
	let evaluator = jsonnet_evaluator::EvaluationState::default();
	if !opts.no_stdlib {
		evaluator.add_stdlib();
	}
	let mut input = current_dir().unwrap();
	input.push(opts.input.clone());
	evaluator
		.add_file(
			input.clone(),
			String::from_utf8(std::fs::read(opts.input.clone()).unwrap()).unwrap(),
		)
		.unwrap();
	let result = evaluator.evaluate_file(&input);
	match result {
		Ok(v) => {
			let v = match opts.format {
				Format::Json => {
					if opts.no_stdlib {
						evaluator.add_stdlib();
					}
					evaluator.add_global("__tmp__to_json__".to_owned(), v);
					let v = evaluator
						.parse_evaluate_raw("std.manifestJsonEx(__tmp__to_json__, \"  \")");
					match v {
						Ok(v) => v,
						Err(err) => {
							print_error(&err, evaluator, &opts);
							std::process::exit(2);
						}
					}
				}
				Format::Yaml => {
					if opts.no_stdlib {
						evaluator.add_stdlib();
					}
					evaluator.add_global("__tmp__to_yaml__".to_owned(), v);
					let v = evaluator
						.parse_evaluate_raw("std.manifestYamlDoc(__tmp__to_yaml__, \"  \")");
					match v {
						Ok(v) => v,
						Err(err) => {
							print_error(&err, evaluator, &opts);
							std::process::exit(2);
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
		}
	}
}

fn print_error(err: &LocError, evaluator: EvaluationState, opts: &Opts) {
	println!("Error: {:?}", err.0);
	print_trace(&(err.1), evaluator, &opts);
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
