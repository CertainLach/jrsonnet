mod ext;
mod manifest;
mod tla;
mod trace;

pub use ext::*;
pub use manifest::*;
pub use tla::*;
pub use trace::*;

use clap::Clap;
use jrsonnet_evaluator::{error::Result, EvaluationState, FileImportResolver};
use std::path::PathBuf;

pub trait ConfigureState {
	fn configure(&self, state: &EvaluationState) -> Result<()>;
}

#[derive(Clap)]
#[clap(help_heading = "INPUT")]
pub struct InputOpts {
	#[clap(
		long,
		short = 'e',
		about = "Treat input as code, evaluate them instead of reading file"
	)]
	pub exec: bool,

	#[clap(
		about = "Path to the file to be compiled if `--evaluate` is unset, otherwise code itself"
	)]
	pub input: String,
}

#[derive(Clap)]
#[clap(help_heading = "OPTIONS")]
pub struct MiscOpts {
	/// Disable standard library.
	/// By default standard library will be available via global `std` variable.
	/// Note that standard library will still be loaded
	/// if chosen manifestification method is not `none`.
	#[clap(long)]
	no_stdlib: bool,

	/// Maximal allowed number of stack frames,
	/// stack overflow error will be raised if this number gets exceeded.
	#[clap(long, short = 's', default_value = "200")]
	max_stack: usize,

	/// Library search dirs.
	/// Any not found `imported` file will be searched in these.
	/// This can also be specified via `JSONNET_PATH` variable,
	/// which should contain a colon-separated (semicolon-separated on Windows) list of directories.
	#[clap(long, short = 'J')]
	jpath: Vec<PathBuf>,
}
impl ConfigureState for MiscOpts {
	fn configure(&self, state: &EvaluationState) -> Result<()> {
		if !self.no_stdlib {
			state.with_stdlib();
		}

		state.set_import_resolver(Box::new(FileImportResolver {
			library_paths: self.jpath.clone(),
		}));

		state.set_max_stack(self.max_stack);
		Ok(())
	}
}

/// General configuration of jsonnet
#[derive(Clap)]
#[clap(name = "jrsonnet", version, author)]
pub struct GeneralOpts {
	#[clap(flatten)]
	misc: MiscOpts,

	#[clap(flatten)]
	tla: TlaOpts,
	#[clap(flatten)]
	ext: ExtVarOpts,

	#[clap(flatten)]
	trace: TraceOpts,
}

impl ConfigureState for GeneralOpts {
	fn configure(&self, state: &EvaluationState) -> Result<()> {
		// Configure trace first, because tla-code/ext-code can throw
		self.trace.configure(state)?;
		self.misc.configure(state)?;
		self.tla.configure(state)?;
		self.ext.configure(state)?;
		Ok(())
	}
}
