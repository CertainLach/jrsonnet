use clap::Clap;
use jrsonnet_cli::{ConfigureState, GeneralOpts, InputOpts, ManifestOpts};
use jrsonnet_evaluator::{error::Result, EvaluationState};
use std::{path::PathBuf, rc::Rc};

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimallocator::Mimalloc = mimallocator::Mimalloc;

#[derive(Clap)]
// #[clap(help_heading = "DEBUG")]
struct DebugOpts {
	/// Required OS stack size, probally you shouldn't change it, unless jrsonnet is failing with stack overflow
	#[clap(long, name = "size")]
	pub os_stack: Option<usize>,
}

#[derive(Clap)]
struct Opts {
	#[clap(flatten)]
	input: InputOpts,
	#[clap(flatten)]
	general: GeneralOpts,
	#[clap(flatten)]
	manifest: ManifestOpts,
	#[clap(flatten)]
	debug: DebugOpts,
}

fn main() {
	let opts: Opts = Opts::parse();
	if let Some(size) = opts.debug.os_stack {
		std::thread::Builder::new()
			.stack_size(size * 1024 * 1024)
			.spawn(|| main_catch(opts))
			.expect("new thread spawned")
			.join()
			.expect("thread finished successfully");
	} else {
		main_catch(opts)
	}
}

fn main_catch(opts: Opts) {
	let state = EvaluationState::default();
	if let Err(e) = main_real(&state, opts) {
		println!("{}", state.stringify_err(&e));
	}
}

fn main_real(state: &EvaluationState, opts: Opts) -> Result<()> {
	opts.general.configure(&state)?;
	opts.manifest.configure(&state)?;

	let val = if opts.input.evaluate {
		state.evaluate_snippet_raw(
			Rc::new(PathBuf::from("args")),
			(&opts.input.input as &str).into(),
		)?
	} else {
		state.evaluate_file_raw(&PathBuf::from(opts.input.input))?
	};

	let val = state.with_tla(val)?;

	println!("{}", state.manifest(val)?);

	Ok(())
}
