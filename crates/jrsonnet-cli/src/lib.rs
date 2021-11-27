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
use std::{env, path::PathBuf};

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

	/// Library search dirs. (right-most wins)
	/// Any not found `imported` file will be searched in these.
	/// This can also be specified via `JSONNET_PATH` variable,
	/// which should contain a colon-separated (semicolon-separated on Windows) list of directories.
	#[clap(long, short = 'J', multiple_occurrences = true)]
	jpath: Vec<PathBuf>,
}
impl ConfigureState for MiscOpts {
	fn configure(&self, state: &EvaluationState) -> Result<()> {
		if !self.no_stdlib {
			state.with_stdlib();
		}

		let mut library_paths = self.jpath.clone();
		library_paths.reverse();
		if let Some(path) = env::var_os("JSONNET_PATH") {
			library_paths.extend(env::split_paths(path.as_os_str()));
		}

		state.set_import_resolver(Box::new(FileImportResolver { library_paths }));

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
	tla: TLAOpts,
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

#[derive(Clap)]
#[clap(help_heading = "GARBAGE COLLECTION")]
pub struct GcOpts {
	/// Do not skip gc on exit
	#[clap(long)]
	gc_collect_on_exit: bool,
	/// Print gc stats before exit
	#[clap(long)]
	gc_print_stats: bool,
	/// Force garbage collection before printing stats
	/// Useful for checking for memory leaks
	/// Does nothing useless --gc-print-stats is specified
	#[clap(long)]
	gc_collect_before_printing_stats: bool,
}
impl GcOpts {
	pub fn configure_global(&self) {
		if !self.gc_collect_on_exit {
			gcmodule::set_thread_collect_on_drop(false)
		}
	}
	pub fn stats_printer(&self) -> Option<GcStatsPrinter> {
		self.gc_print_stats.then(|| GcStatsPrinter {
			collect_before_printing_stats: self.gc_collect_before_printing_stats,
		})
	}
}

pub struct GcStatsPrinter {
	collect_before_printing_stats: bool,
}
impl Drop for GcStatsPrinter {
	fn drop(&mut self) {
		eprintln!("=== GC STATS ===");
		if self.collect_before_printing_stats {
			let collected = gcmodule::collect_thread_cycles();
			eprintln!("Collected: {}", collected);
		}
		eprintln!("Tracked: {}", gcmodule::count_thread_tracked())
	}
}
