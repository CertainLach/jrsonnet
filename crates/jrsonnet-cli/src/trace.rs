use std::str::FromStr;

use clap::Parser;
use jrsonnet_evaluator::{
	error::Result,
	trace::{CompactFormat, ExplainingFormat, PathResolver},
	EvaluationState,
};

use crate::ConfigureState;

#[derive(PartialEq)]
pub enum TraceFormatName {
	Compact,
	Explaining,
}

impl FromStr for TraceFormatName {
	type Err = &'static str;
	fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
		Ok(match s {
			"compact" => TraceFormatName::Compact,
			"explaining" => TraceFormatName::Explaining,
			_ => return Err("no such format"),
		})
	}
}

#[derive(Parser)]
#[clap(next_help_heading = "STACK TRACE VISUAL")]
pub struct TraceOpts {
	/// Format of stack traces' display in console.
	/// `compact` format only shows `filename:line:column`s
	/// while `explaining` displays source code with attached trace annotations
	/// thus being more verbose.
	#[clap(long, possible_values = &["compact", "explaining"])]
	trace_format: Option<TraceFormatName>,
	/// Amount of stack trace elements to be displayed.
	/// If set to `0` then full stack trace will be displayed.
	#[clap(long, short = 't', default_value = "20")]
	max_trace: usize,
}
impl ConfigureState for TraceOpts {
	fn configure(&self, state: &EvaluationState) -> Result<()> {
		let resolver = PathResolver::Absolute;
		match self
			.trace_format
			.as_ref()
			.unwrap_or(&TraceFormatName::Compact)
		{
			TraceFormatName::Compact => state.set_trace_format(Box::new(CompactFormat {
				resolver,
				padding: 4,
			})),
			TraceFormatName::Explaining => {
				state.set_trace_format(Box::new(ExplainingFormat { resolver }))
			}
		}
		state.set_max_trace(self.max_trace);
		Ok(())
	}
}
