use clap::AppSettings;
use clap::Clap;
use jrsonnet_cli::{ConfigureState, GeneralOpts, InputOpts, ManifestOpts, OutputOpts};
use jrsonnet_evaluator::{error::LocError, EvaluationState, ManifestFormat};
use std::{
	fs::{create_dir_all, File},
	io::Read,
	io::Write,
	path::PathBuf,
	rc::Rc,
};

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimallocator::Mimalloc = mimallocator::Mimalloc;

#[derive(Clap)]
#[clap(help_heading = "DEBUG")]
struct DebugOpts {
	/// Required OS stack size.
	/// This shouldn't be changed unless jrsonnet is failing with stack overflow error.
	#[clap(long, name = "size")]
	pub os_stack: Option<usize>,
}

#[derive(Clap)]
#[clap(
	global_setting = AppSettings::ColoredHelp,
	global_setting = AppSettings::DeriveDisplayOrder,
)]
struct Opts {
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
}

fn main() {
	let opts: Opts = Opts::parse();
	let success;
	if let Some(size) = opts.debug.os_stack {
		success = std::thread::Builder::new()
			.stack_size(size * 1024 * 1024)
			.spawn(|| main_catch(opts))
			.expect("new thread spawned")
			.join()
			.expect("thread finished successfully");
	} else {
		success = main_catch(opts)
	}
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
	let state = EvaluationState::default();
	if let Err(e) = main_real(&state, opts) {
		if let Error::Evaluation(e) = e {
			eprintln!("{}", state.stringify_err(&e));
		} else {
			eprintln!("{}", e);
		}
		return false;
	}
	true
}

fn main_real(state: &EvaluationState, opts: Opts) -> Result<(), Error> {
	opts.general.configure(&state)?;
	opts.manifest.configure(&state)?;

	let val = if opts.input.exec {
		state.set_manifest_format(ManifestFormat::ToString);
		state.evaluate_snippet_raw(
			Rc::new(PathBuf::from("args")),
			(&opts.input.input as &str).into(),
		)?
	} else if opts.input.input == "-" {
		let mut input = Vec::new();
		std::io::stdin().read_to_end(&mut input)?;
		let input_str = std::str::from_utf8(&input)?.into();
		state.evaluate_snippet_raw(Rc::new(PathBuf::from("<stdin>")), input_str)?
	} else {
		state.evaluate_file_raw(&PathBuf::from(opts.input.input))?
	};

	let val = state.with_tla(val)?;

	if let Some(multi) = opts.output.multi {
		if opts.output.create_output_dirs {
			let mut dir = multi.clone();
			dir.pop();
			create_dir_all(dir)?;
		}
		for (file, data) in state.manifest_multi(val)?.iter() {
			let mut path = multi.clone();
			path.push(&file as &str);
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
		writeln!(file, "{}", state.manifest(val)?)?;
	} else {
		println!("{}", state.manifest(val)?);
	}

	Ok(())
}
