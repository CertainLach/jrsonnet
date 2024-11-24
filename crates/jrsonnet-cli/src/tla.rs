use clap::Parser;
use jrsonnet_evaluator::{error::Result, function::TlaArg, gc::GcHashMap, IStr};

use crate::{ExtFile, ExtStr};

#[derive(Parser)]
#[clap(next_help_heading = "TOP LEVEL ARGUMENTS")]
pub struct TlaOpts {
	/// Add top level string argument.
	/// Top level arguments will be passed to function before manifestification stage.
	/// This is preferred to [`ExtVars`] method.
	/// If [=data] is not set then it will be read from `name` env variable.
	#[clap(long, short = 'A', name = "name[=tla data]", number_of_values = 1)]
	tla_str: Vec<ExtStr>,
	/// Read top level argument string from file.
	/// See also `--tla-str`
	#[clap(long, name = "name=tla path", number_of_values = 1)]
	tla_str_file: Vec<ExtFile>,
	/// Add top level argument from code.
	/// See also `--tla-str`
	#[clap(long, name = "name[=tla source]", number_of_values = 1)]
	tla_code: Vec<ExtStr>,
	/// Read top level argument code from file.
	/// See also `--tla-str`
	#[clap(long, name = "name=tla code path", number_of_values = 1)]
	tla_code_file: Vec<ExtFile>,
}
impl TlaOpts {
	pub fn tla_opts(&self) -> Result<GcHashMap<IStr, TlaArg>> {
		let mut out = GcHashMap::new();
		for ext in &self.tla_str {
			out.insert(
				ext.name.as_str().into(),
				TlaArg::String(ext.value.as_str().into()),
			);
		}
		for ext in &self.tla_str_file {
			out.insert(
				ext.name.as_str().into(),
				TlaArg::ImportStr(ext.name.as_str().into()),
			);
		}
		for ext in &self.tla_code {
			out.insert(
				ext.name.as_str().into(),
				TlaArg::InlineCode(ext.value.clone()),
			);
		}
		for ext in &self.tla_code_file {
			out.insert(ext.name.as_str().into(), TlaArg::Import(ext.path.clone()));
		}
		Ok(out)
	}
}
