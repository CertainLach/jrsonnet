mod manifest;
mod stdlib;
mod tla;
mod trace;

use std::{env, marker::PhantomData, path::PathBuf};

use clap::Parser;
use jrsonnet_evaluator::{
	error::Result, stack::StackDepthLimitOverrideGuard, FileImportResolver, State,
};
use jrsonnet_gcmodule::with_thread_object_space;
pub use manifest::*;
pub use stdlib::*;
pub use tla::*;
pub use trace::*;

pub trait ConfigureState {
	type Guards;

	fn configure(&self, s: &State) -> Result<Self::Guards>;
}

#[derive(Parser)]
#[clap(next_help_heading = "INPUT")]
pub struct InputOpts {
	/// Treat input as code, evaluate them instead of reading file
	#[clap(long, short = 'e')]
	pub exec: bool,

	/// Path to the file to be compiled if `--evaluate` is unset, otherwise code itself
	pub input: String,
}

#[derive(Parser)]
#[clap(next_help_heading = "OPTIONS")]
pub struct MiscOpts {
	/// Maximal allowed number of stack frames,
	/// stack overflow error will be raised if this number gets exceeded.
	#[clap(long, short = 's', default_value = "200")]
	max_stack: usize,

	/// Library search dirs. (right-most wins)
	/// Any not found `imported` file will be searched in these.
	/// This can also be specified via `JSONNET_PATH` variable,
	/// which should contain a colon-separated (semicolon-separated on Windows) list of directories.
	#[clap(long, short = 'J')]
	jpath: Vec<PathBuf>,
}
impl ConfigureState for MiscOpts {
	type Guards = StackDepthLimitOverrideGuard;
	fn configure(&self, s: &State) -> Result<Self::Guards> {
		let mut library_paths = self.jpath.clone();
		library_paths.reverse();
		if let Some(path) = env::var_os("JSONNET_PATH") {
			library_paths.extend(env::split_paths(path.as_os_str()));
		}

		s.set_import_resolver(Box::new(FileImportResolver::new(library_paths)));

		let _depth_limit = jrsonnet_evaluator::stack::limit_stack_depth(self.max_stack);
		Ok(_depth_limit)
	}
}

/// General configuration of jsonnet
#[derive(Parser)]
#[clap(name = "jrsonnet", version, author)]
pub struct GeneralOpts {
	#[clap(flatten)]
	misc: MiscOpts,

	#[clap(flatten)]
	tla: TlaOpts,
	#[clap(flatten)]
	std: StdOpts,

	#[clap(flatten)]
	gc: GcOpts,
}

impl ConfigureState for GeneralOpts {
	type Guards = (
		<MiscOpts as ConfigureState>::Guards,
		<TlaOpts as ConfigureState>::Guards,
		<GcOpts as ConfigureState>::Guards,
	);
	fn configure(&self, s: &State) -> Result<Self::Guards> {
		// Configure trace first, because tla-code/ext-code can throw
		let misc_guards = self.misc.configure(s)?;
		let tla_guards = self.tla.configure(s)?;
		self.std.configure(s)?;
		let gc_guards = self.gc.configure(s)?;
		Ok((misc_guards, tla_guards, gc_guards))
	}
}

#[derive(Parser)]
#[clap(next_help_heading = "GARBAGE COLLECTION")]
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
impl ConfigureState for GcOpts {
	type Guards = (Option<GcStatsPrinter>, Option<LeakSpace>);

	fn configure(&self, _s: &State) -> Result<Self::Guards> {
		// Constructed structs have side-effects in Drop impl
		#[allow(clippy::unnecessary_lazy_evaluations)]
		Ok((
			self.gc_print_stats.then(|| GcStatsPrinter {
				collect_before_printing_stats: self.gc_collect_before_printing_stats,
			}),
			(!self.gc_collect_on_exit).then(|| LeakSpace(PhantomData)),
		))
	}
}

pub struct LeakSpace(PhantomData<()>);

impl Drop for LeakSpace {
	fn drop(&mut self) {
		with_thread_object_space(|s| s.leak())
	}
}

pub struct GcStatsPrinter {
	collect_before_printing_stats: bool,
}
impl Drop for GcStatsPrinter {
	fn drop(&mut self) {
		eprintln!("=== GC STATS ===");
		if self.collect_before_printing_stats {
			let collected = jrsonnet_gcmodule::collect_thread_cycles();
			eprintln!("Collected: {}", collected);
		}
		eprintln!("Tracked: {}", jrsonnet_gcmodule::count_thread_tracked())
	}
}
