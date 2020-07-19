use crate::ConfigureState;
use clap::Clap;
use jrsonnet_evaluator::{EvaluationState, Result};
use std::str::FromStr;

#[derive(Clone)]
pub struct ExtStr {
	pub name: String,
	pub value: String,
}

impl FromStr for ExtStr {
	type Err = &'static str;
	fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
		let out: Vec<_> = s.split('=').collect();
		match out.len() {
			1 => Ok(ExtStr {
				name: out[0].to_owned(),
				value: std::env::var(out[0]).or(Err("missing env var"))?,
			}),
			2 => Ok(ExtStr {
				name: out[0].to_owned(),
				value: out[1].to_owned(),
			}),

			_ => Err("bad ext-str syntax"),
		}
	}
}

#[derive(Clap)]
// #[clap(help_heading = "EXTERNAL VARIABLES")]
pub struct ExtVarOpts {
	/// Add string external variable.
	/// External variables are globally available, so prefer to use top level arguments where possible.
	/// If [=data] is not set, then it will be read from `name` env variable.
	/// Can be accessed from code via `std.extVar("name")`
	#[clap(long, short = 'V', name = "name[=var data]", number_of_values = 1)]
	ext_str: Vec<ExtStr>,
	/// Read string external variable from file.
	/// See also `--ext-str`
	// #[clap(long, name = "name[=var path]", number_of_values = 1)]
	// ext_str_file: Vec<ExtStr>,
	/// Add external variable from code.
	/// See also `--ext-str`
	#[clap(long, name = "name[=var source]", number_of_values = 1)]
	ext_code: Vec<ExtStr>,
}
impl ConfigureState for ExtVarOpts {
	fn configure(&self, state: &EvaluationState) -> Result<()> {
		for ext in self.ext_str.iter() {
			state.add_ext_str((&ext.name as &str).into(), (&ext.value as &str).into());
		}
		for ext in self.ext_code.iter() {
			state.add_ext_code((&ext.name as &str).into(), (&ext.value as &str).into())?;
		}
		Ok(())
	}
}
