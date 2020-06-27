pub mod location;

use clap::Clap;
use jrsonnet_evaluator::{EvaluationSettings, EvaluationState, LocError, StackTrace, Val};
use jsonnet_parser::{el, Arg, ArgsDesc, Expr, LocExpr, ParserSettings};
use location::{offset_to_location, CodeLocation};
use std::env::current_dir;
use std::{collections::HashMap, path::PathBuf, rc::Rc, str::FromStr};

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

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

	#[clap(long, short = "J", about = "Library search dir")]
	jpath: Vec<PathBuf>,

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
	let evaluator = jrsonnet_evaluator::EvaluationState::new(
		EvaluationSettings {
			max_stack_trace_size: opts.max_trace,
			max_stack_frames: opts.max_stack,
		},
		Box::new(jrsonnet_evaluator::FileImportResolver {
			library_paths: opts.jpath.clone(),
		}),
	);
	if !opts.no_stdlib {
		evaluator.with_stdlib();
	}
	for ExtStr { name, value } in opts.ext_str.iter().cloned() {
		evaluator.add_ext_var(name.into(), Val::Str(value.into()));
	}
	for ExtStr { name, value } in opts.ext_code.iter().cloned() {
		evaluator.add_ext_var(name.into(), evaluator.parse_evaluate_raw(&value).unwrap());
	}
	let mut input = current_dir().unwrap();
	input.push(opts.input.clone());
	let code_string = String::from_utf8(std::fs::read(opts.input.clone()).unwrap()).unwrap();
	if let Err(e) = evaluator.add_file(Rc::new(input.clone()), code_string.clone().into()) {
		print_syntax_error(e, &input, &code_string);
		std::process::exit(1);
	}
	let result = evaluator.evaluate_file(&input);
	match result {
		Ok(v) => {
			let v = match v {
				Val::Func(f) => {
					let mut desc_map = HashMap::new();
					for ExtStr { name, value } in opts.tla_str.iter().cloned() {
						desc_map.insert(name, el!(Expr::Str(value.into())));
					}
					for ExtStr { name, value } in opts.tla_code.iter().cloned() {
						desc_map.insert(
							name,
							jsonnet_parser::parse(
								&value,
								&ParserSettings {
									file_name: Rc::new(PathBuf::new()),
									loc_data: false,
								},
							)
							.unwrap(),
						);
					}
					evaluator.add_global("__tmp__tlf__".into(), Val::Func(f));
					evaluator
						.evaluate_raw(el!(Expr::Apply(
							el!(Expr::Var("__tmp__tlf__".into())),
							ArgsDesc(desc_map.into_iter().map(|(k, v)| Arg(Some(k), v)).collect()),
							false,
						)))
						.unwrap()
				}
				v => v,
			};
			let v = evaluator.run_in_state(|| match opts.format {
				Format::Json => Ok(Val::Str(v.into_json(opts.line_padding)?)),
				Format::Yaml => {
					evaluator.add_global("__tmp__to_yaml__".into(), v);
					evaluator.parse_evaluate_raw("std.manifestYamlDoc(__tmp__to_yaml__, \"  \")")
				}
				_ => Ok(v),
			});
			let v = match v {
				Ok(v) => v,
				Err(err) => {
					print_error(&err, evaluator, &opts);
					std::process::exit(1);
				}
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
		let source = item.0.clone();
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
