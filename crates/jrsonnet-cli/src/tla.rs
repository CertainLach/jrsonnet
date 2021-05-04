use crate::{ConfigureState, ExtFile, ExtStr};
use clap::Clap;
use jrsonnet_evaluator::{error::Result, EvaluationState};

#[derive(Clap)]
#[clap(help_heading = "TOP LEVEL ARGUMENTS")]
pub struct TlaOpts {
	/// Add top level string argument.
	/// Top level arguments will be passed to function before manifestification stage.
	/// This is preferred to ExtVars method.
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
impl ConfigureState for TlaOpts {
	fn configure(&self, state: &EvaluationState) -> Result<()> {
		for tla in self.tla_str.iter() {
			state.add_tla_str((&tla.name as &str).into(), (&tla.value as &str).into());
		}
		for tla in self.tla_str_file.iter() {
			state.add_tla_str((&tla.name as &str).into(), (&tla.value as &str).into())
		}
		for tla in self.tla_code.iter() {
			state.add_tla_code((&tla.name as &str).into(), (&tla.value as &str).into())?;
		}
		for tla in self.tla_code_file.iter() {
			state.add_tla_code((&tla.name as &str).into(), (&tla.value as &str).into())?;
		}
		Ok(())
	}
}
