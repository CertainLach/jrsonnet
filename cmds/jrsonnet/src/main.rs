use clap::Clap;
use jrsonnet_evaluator::Val;
use jrsonnet_parser::{el, Arg, ArgsDesc, Expr, LocExpr, ParserSettings};
use jrsonnet_trace::{CompactFormat, ExplainingFormat, PathResolver, TraceFormat};
use std::env::current_dir;
use std::{collections::HashMap, path::PathBuf, rc::Rc, str::FromStr};

#[global_allocator]
static GLOBAL: mimallocator::Mimalloc = mimallocator::Mimalloc;

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
enum TraceFormatName {
	Compact,
	Explaining,
}
impl FromStr for TraceFormatName {
	type Err = &'static str;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(match s {
			"compact" => TraceFormatName::Compact,
			"explaining" => TraceFormatName::Explaining,
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
#[clap(name = "jrsonnet", version, author)]
pub struct Opts {
	#[clap(long, about = "Disable global std variable")]
	no_stdlib: bool,
	#[clap(long, about = "Add external string", number_of_values = 1)]
	ext_str: Vec<ExtStr>,
	#[clap(long, about = "Add external string from code", number_of_values = 1)]
	ext_code: Vec<ExtStr>,
	#[clap(long, about = "Add TLA", number_of_values = 1)]
	tla_str: Vec<ExtStr>,
	#[clap(long, about = "Add TLA from code", number_of_values = 1)]
	tla_code: Vec<ExtStr>,
	#[clap(long, short = "f", default_value = "json", possible_values = &["none", "json", "yaml"], about = "Output format, wraps resulting value to corresponding std.manifest call")]
	format: Format,
	#[clap(long, default_value = "compact", possible_values = &["compact", "explaining"], about = "Choose format of displayed stacktraces")]
	trace_format: TraceFormatName,

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
		about = "Required os stack size, probally you shouldn't change it"
	)]
	thread_stack_size: Option<usize>,

	#[clap(long, short = "J", about = "Library search dir")]
	jpath: Vec<PathBuf>,

	#[clap(
		long,
		default_value = "3",
		about = "When using --format, this option specifies string to pad output with"
	)]
	line_padding: usize,

	#[clap(about = "File to compile")]
	input: String,
}

fn main() {
	let opts: Opts = Opts::parse();
	if let Some(size) = opts.thread_stack_size {
		std::thread::Builder::new()
			.stack_size(size * 1024 * 1024)
			.spawn(|| main_real(opts))
			.unwrap()
			.join()
			.unwrap();
	} else {
		main_real(opts)
	}
}

fn main_real(opts: Opts) {
	let evaluator = jrsonnet_evaluator::EvaluationState::default();
	{
		let mut settings = evaluator.settings_mut();
		settings.max_stack = opts.max_stack;
		settings.max_trace = opts.max_trace;
		settings.import_resolver = Box::new(jrsonnet_evaluator::FileImportResolver {
			library_paths: opts.jpath.clone(),
		});
	}
	if !opts.no_stdlib {
		evaluator.with_stdlib();
	}
	for ExtStr { name, value } in opts.ext_str.iter().cloned() {
		evaluator
			.settings_mut()
			.ext_vars
			.insert(name.into(), Val::Str(value.into()));
	}
	for ExtStr { name, value } in opts.ext_code.iter().cloned() {
		evaluator.settings_mut().ext_vars.insert(
			name.clone().into(),
			evaluator
				.parse_evaluate_raw(PathBuf::from(format!("ext_code {}", name)).into(), &value)
				.unwrap(),
		);
	}

	let resolver = PathResolver::Relative(std::env::current_dir().unwrap());
	let trace_format: Box<dyn TraceFormat> = match opts.trace_format {
		TraceFormatName::Compact => Box::new(CompactFormat { resolver }),
		TraceFormatName::Explaining => Box::new(ExplainingFormat { resolver }),
	};

	let mut input = current_dir().unwrap();
	input.push(opts.input.clone());
	let code_string = String::from_utf8(std::fs::read(opts.input.clone()).unwrap()).unwrap();
	if let Err(e) = evaluator.add_file(Rc::new(input.clone()), code_string.into()) {
		trace_format.print_trace(&evaluator, &e).unwrap();
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
							jrsonnet_parser::parse(
								&value,
								&ParserSettings {
									file_name: Rc::new(PathBuf::new()),
									loc_data: false,
								},
							)
							.unwrap(),
						);
					}
					evaluator
						.settings_mut()
						.globals
						.insert("__tmp__tlf__".into(), Val::Func(f));
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
				Format::Yaml => Ok(Val::Str(v.into_yaml(opts.line_padding)?)),
				_ => Ok(v),
			});
			let v = match v {
				Ok(v) => v,
				Err(err) => {
					trace_format.print_trace(&evaluator, &err).unwrap();
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
			trace_format.print_trace(&evaluator, &err).unwrap();
			std::process::exit(1);
		}
	}
}
