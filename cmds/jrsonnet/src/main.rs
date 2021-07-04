use clap::{AppSettings, Clap, IntoApp};
use jrsonnet_cli::{ConfigureState, GcOpts, GeneralOpts, InputOpts, ManifestOpts, OutputOpts};
use jrsonnet_evaluator::{error::LocError, EvaluationState, ManifestFormat};
use std::{
	fs::{create_dir_all, File},
	io::Read,
	io::Write,
	path::PathBuf,
	str::FromStr,
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
	/// Generate completions script
	#[clap(long)]
	generate: Option<GenerateTarget>,
}

enum GenerateTarget {
	Bash,
	Zsh,
	Fish,
	PowerShell,
}
impl FromStr for GenerateTarget {
	type Err = &'static str;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"bash" => Ok(Self::Bash),
			"zsh" => Ok(Self::Zsh),
			"fish" => Ok(Self::Fish),
			"powershell" => Ok(Self::PowerShell),
			_ => Err("unknown target"),
		}
	}
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
	#[clap(flatten)]
	gc: GcOpts,
}

fn main() {
	let opts: Opts = Opts::parse();

	if let Some(target) = opts.debug.generate {
		use clap_generate::{generate, generators};
		use GenerateTarget::*;
		let app = &mut Opts::into_app();
		let buf = &mut std::io::stdout();
		let bin = "jrsonnet";
		match target {
			Bash => generate::<generators::Bash, _>(app, bin, buf),
			Zsh => generate::<generators::Zsh, _>(app, bin, buf),
			Fish => generate::<generators::Fish, _>(app, bin, buf),
			PowerShell => generate::<generators::PowerShell, _>(app, bin, buf),
		}
		std::process::exit(0);
	};

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
	let _printer = opts.gc.stats_printer();
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
	opts.gc.configure_global();
	opts.general.configure(&state)?;
	opts.manifest.configure(&state)?;

	let val = if opts.input.exec {
		state.set_manifest_format(ManifestFormat::ToString);
		state.evaluate_snippet_raw(
			PathBuf::from("args").into(),
			(&opts.input.input as &str).into(),
		)?
	} else if opts.input.input == "-" {
		let mut input = Vec::new();
		std::io::stdin().read_to_end(&mut input)?;
		let input_str = std::str::from_utf8(&input)?.into();
		state.evaluate_snippet_raw(PathBuf::from("<stdin>").into(), input_str)?
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
		let output = state.manifest(val)?;
		if !output.is_empty() {
			println!("{}", output);
		}
	}

	Ok(())
}
