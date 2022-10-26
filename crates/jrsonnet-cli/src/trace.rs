use clap::{Parser, ValueEnum};
use jrsonnet_evaluator::{
	error::Result,
	trace::{CompactFormat, ExplainingFormat, PathResolver},
	State,
};

use crate::ConfigureState;

#[derive(PartialEq, Eq, ValueEnum, Clone)]
pub enum TraceFormatName {
	/// Only show `filename:line:column`
	Compact,
	/// Display source code with attached trace annotations
	Explaining,
}

#[derive(Parser)]
#[clap(next_help_heading = "STACK TRACE VISUAL")]
pub struct TraceOpts {
	/// Format of stack traces' display in console.
	#[clap(long)]
	trace_format: Option<TraceFormatName>,
	/// Amount of stack trace elements to be displayed.
	/// If set to `0` then full stack trace will be displayed.
	#[clap(long, short = 't', default_value = "20")]
	max_trace: usize,
}
impl ConfigureState for TraceOpts {
	type Guards = ();
	fn configure(&self, s: &State) -> Result<()> {
		let resolver = PathResolver::new_cwd_fallback();
		match self
			.trace_format
			.as_ref()
			.unwrap_or(&TraceFormatName::Compact)
		{
			TraceFormatName::Compact => s.set_trace_format(CompactFormat {
				resolver,
				padding: 4,
			}),
			TraceFormatName::Explaining => s.set_trace_format(ExplainingFormat { resolver }),
		}
		s.set_max_trace(self.max_trace);
		Ok(())
	}
}
