use clap::Parser;
use jrsonnet_evaluator::{
	error::{ErrorKind, Result},
	function::TlaArg,
	gc::GcHashMap,
	IStr,
};
use jrsonnet_parser::{ParserSettings, Source};

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
		for (name, value) in self
			.tla_str
			.iter()
			.map(|c| (&c.name, &c.value))
			.chain(self.tla_str_file.iter().map(|c| (&c.name, &c.value)))
		{
			out.insert(name.into(), TlaArg::String(value.into()));
		}
		for (name, code) in self
			.tla_code
			.iter()
			.map(|c| (&c.name, &c.value))
			.chain(self.tla_code_file.iter().map(|c| (&c.name, &c.value)))
		{
			let source = Source::new_virtual(format!("<top-level-arg:{name}>").into(), code.into());
			out.insert(
				(name as &str).into(),
				TlaArg::Code(
					jrsonnet_parser::parse(
						code,
						&ParserSettings {
							source: source.clone(),
						},
					)
					.map_err(|e| ErrorKind::ImportSyntaxError {
						path: source,
						error: Box::new(e),
					})?,
				),
			);
		}
		Ok(out)
	}
}
