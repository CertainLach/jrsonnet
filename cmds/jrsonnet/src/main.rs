use std::{
	fs::{create_dir_all, File},
	io::{Read, Write},
	path::PathBuf,
};

use clap::{AppSettings, IntoApp, Parser};
use clap_complete::Shell;
use jrsonnet_cli::{ConfigureState, GcOpts, GeneralOpts, InputOpts, ManifestOpts, OutputOpts};
use jrsonnet_evaluator::{error::LocError, State};

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
#[clap(
	global_setting = AppSettings::DeriveDisplayOrder,
	// args_conflicts_with_subcommands = true,
)]
struct Opts {
	#[clap(subcommand)]
	sub: Option<SubOpts>,

	#[clap(flatten)]
	input: InputOpts,
	#[clap(flatten)]
	general: GeneralOpts,
	#[clap(flatten)]
	manifest: ManifestOpts,
	#[clap(flatten)]
	output: OutputOpts,
	#[clap(flatten)]
	debug: DebugOpts,
	#[clap(flatten)]
	gc: GcOpts,
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
	Evaluation(jrsonnet_evaluator::error::LocError),
	#[error("io error")]
	Io(#[from] std::io::Error),
	#[error("input is not utf8 encoded")]
	Utf8(#[from] std::str::Utf8Error),
}
impl From<LocError> for Error {
	fn from(e: LocError) -> Self {
		Self::Evaluation(e)
	}
}

fn main_catch(opts: Opts) -> bool {
	let _printer = opts.gc.stats_printer();
	let s = State::default();
	if let Err(e) = main_real(&s, opts) {
		if let Error::Evaluation(e) = e {
			eprintln!("{}", s.stringify_err(&e));
		} else {
			eprintln!("{}", e);
		}
		return false;
	}
	true
}

fn main_real(s: &State, opts: Opts) -> Result<(), Error> {
	opts.gc.configure_global();
	opts.general.configure(s)?;
	opts.manifest.configure(s)?;

	let val = if opts.input.exec {
		s.evaluate_snippet("<cmdline>".to_owned(), (&opts.input.input as &str).into())?
	} else if opts.input.input == "-" {
		let mut input = Vec::new();
		std::io::stdin().read_to_end(&mut input)?;
		let input_str = std::str::from_utf8(&input)?.into();
		s.evaluate_snippet("<stdin>".to_owned(), input_str)?
	} else {
		s.import(s.resolve_file(&PathBuf::new(), &opts.input.input)?)?
	};

	let val = s.with_tla(val)?;

	if let Some(multi) = opts.output.multi {
		if opts.output.create_output_dirs {
			let mut dir = multi.clone();
			dir.pop();
			create_dir_all(dir)?;
		}
		for (file, data) in s.manifest_multi(val)?.iter() {
			let mut path = multi.clone();
			path.push(file as &str);
			if opts.output.create_output_dirs {
				let mut dir = path.clone();
				dir.pop();
				create_dir_all(dir)?;
			}
			println!("{}", path.to_str().expect("path"));
			let mut file = File::create(path)?;
			writeln!(file, "{}", data)?;
		}
	} else if let Some(path) = opts.output.output_file {
		if opts.output.create_output_dirs {
			let mut dir = path.clone();
			dir.pop();
			create_dir_all(dir)?;
		}
		let mut file = File::create(path)?;
		writeln!(file, "{}", s.manifest(val)?)?;
	} else {
		let output = s.manifest(val)?;
		if !output.is_empty() {
			println!("{}", output);
		}
	}

	Ok(())
}
