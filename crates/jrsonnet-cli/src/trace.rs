use clap::{Parser, ValueEnum};
use jrsonnet_evaluator::{
	error::Result,
	trace::{CompactFormat, ExplainingFormat, PathResolver, TraceFormat},
	State,
};

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
impl TraceOpts {
	pub fn trace_format(&self) -> Box<dyn TraceFormat> {
		let resolver = PathResolver::new_cwd_fallback();
		let max_trace = self.max_trace;
		let format: Box<dyn TraceFormat> = match self
			.trace_format
			.as_ref()
			.unwrap_or(&TraceFormatName::Compact)
		{
			TraceFormatName::Compact => Box::new(CompactFormat {
				resolver,
				padding: 4,
				max_trace,
			}),
			TraceFormatName::Explaining => Box::new(ExplainingFormat {
				resolver,
				max_trace,
			}),
		};
		format
	}
}
