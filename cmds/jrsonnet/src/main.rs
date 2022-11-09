use std::{
	fs::{create_dir_all, File},
	io::{Read, Write},
};

use clap::{CommandFactory, Parser};
use clap_complete::Shell;
use jrsonnet_cli::{ConfigureState, GeneralOpts, ManifestOpts, OutputOpts, TraceOpts};
use jrsonnet_evaluator::{apply_tla, error::LocError, throw, ResultExt, State, Val};

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimallocator::Mimalloc = mimallocator::Mimalloc;

#[derive(Parser)]
enum SubOpts {
	/// Generate completions for specified shell
	Generate {
		/// Target shell name
		shell: Shell,
	},
}

#[derive(Parser)]
#[clap(next_help_heading = "DEBUG")]
struct DebugOpts {
	/// Required OS stack size.
	/// This shouldn't be changed unless jrsonnet is failing with stack overflow error.
	#[clap(long, name = "size")]
	pub os_stack: Option<usize>,
}

#[derive(Parser)]
#[clap(next_help_heading = "INPUT")]
struct InputOpts {
	/// Treat input as code, evaluate them instead of reading file
	#[clap(long, short = 'e')]
	pub exec: bool,

	/// Path to the file to be compiled if `--evaluate` is unset, otherwise code itself
	pub input: Option<String>,
}

#[derive(Parser)]
#[clap(args_conflicts_with_subcommands = true, disable_version_flag = true)]
struct Opts {
	#[clap(subcommand)]
	sub: Option<SubOpts>,

	#[clap(flatten)]
	input: InputOpts,
	#[clap(flatten)]
	general: GeneralOpts,

	#[clap(flatten)]
	trace: TraceOpts,
	#[clap(flatten)]
	manifest: ManifestOpts,
	#[clap(flatten)]
	output: OutputOpts,
	#[clap(flatten)]
	debug: DebugOpts,
}

fn main() {
	let opts: Opts = Opts::parse();

	if let Some(sub) = opts.sub {
		match sub {
			SubOpts::Generate { shell } => {
				use clap_complete::generate;
				let app = &mut Opts::command();
				let buf = &mut std::io::stdout();
				generate(shell, app, "jrsonnet", buf);
				std::process::exit(0)
			}
		}
	}

	let success = if let Some(size) = opts.debug.os_stack {
		std::thread::Builder::new()
			.stack_size(size * 1024 * 1024)
			.spawn(|| main_catch(opts))
			.expect("new thread spawned")
			.join()
			.expect("thread finished successfully")
	} else {
		main_catch(opts)
	};
	if !success {
		std::process::exit(1);
	}
}

#[derive(thiserror::Error, Debug)]
enum Error {
	// Handled differently
	#[error("evaluation error")]
	Evaluation(LocError),
	#[error("io error")]
	Io(#[from] std::io::Error),
	#[error("input is not utf8 encoded")]
	Utf8(#[from] std::str::Utf8Error),
	#[error("missing input argument")]
	MissingInputArgument,
}
impl From<LocError> for Error {
	fn from(e: LocError) -> Self {
		Self::Evaluation(e)
	}
}
impl From<jrsonnet_evaluator::error::Error> for Error {
	fn from(e: jrsonnet_evaluator::error::Error) -> Self {
		Self::from(LocError::from(e))
	}
}

fn main_catch(opts: Opts) -> bool {
	let s = State::default();
	let trace = opts
		.trace
		.configure(&s)
		.expect("this configurator doesn't fail");
	if let Err(e) = main_real(&s, opts) {
		if let Error::Evaluation(e) = e {
			let mut out = String::new();
			trace.write_trace(&mut out, &e).expect("format error");
			eprintln!("{out}")
		} else {
			eprintln!("{}", e);
		}
		return false;
	}
	true
}

fn main_real(s: &State, opts: Opts) -> Result<(), Error> {
	let (_stack_guard, tla, _gc_guard) = opts.general.configure(s)?;
	let manifest_format = opts.manifest.configure(s)?;

	let input = opts.input.input.ok_or(Error::MissingInputArgument)?;
	let val = if opts.input.exec {
		s.evaluate_snippet("<cmdline>".to_owned(), &input as &str)?
	} else if input == "-" {
		let mut input = Vec::new();
		std::io::stdin().read_to_end(&mut input)?;
		let input_str = std::str::from_utf8(&input)?;
		s.evaluate_snippet("<stdin>".to_owned(), input_str)?
	} else {
		s.import(&input)?
	};

	let val = apply_tla(s.clone(), &tla, val)?;

	if let Some(multi) = opts.output.multi {
		if opts.output.create_output_dirs {
			let mut dir = multi.clone();
			dir.pop();
			create_dir_all(dir)?;
		}
		let Val::Obj(obj) = val else {
			throw!("value should be object for --multi manifest, got {}", val.value_type())
		};
		for (field, data) in obj.iter(
			#[cfg(feature = "exp-preserve-order")]
			opts.manifest.preserve_order,
		) {
			let data = data.with_description(|| format!("getting field {field} for manifest"))?;

			let mut path = multi.clone();
			path.push(&field as &str);
			if opts.output.create_output_dirs {
				let mut dir = path.clone();
				dir.pop();
				create_dir_all(dir)?;
			}
			println!("{}", path.to_str().expect("path"));
			let mut file = File::create(path)?;
			writeln!(
				file,
				"{}",
				data.manifest(&manifest_format)
					.with_description(|| format!("manifesting {field}"))?
			)?;
		}
	} else if let Some(path) = opts.output.output_file {
		if opts.output.create_output_dirs {
			let mut dir = path.clone();
			dir.pop();
			create_dir_all(dir)?;
		}
		let mut file = File::create(path)?;
		writeln!(file, "{}", val.manifest(manifest_format)?)?;
	} else {
		let output = val.manifest(manifest_format)?;
		if !output.is_empty() {
			println!("{}", output);
		}
	}

	Ok(())
}
