use crate::ConfigureState;
use clap::Clap;
use jrsonnet_evaluator::{error::Result, EvaluationState};
use std::{fs::read_to_string, str::FromStr};

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

#[derive(Clone)]
pub struct ExtFile {
	pub name: String,
	pub value: String,
}

impl FromStr for ExtFile {
	type Err = String;

	fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
		let out: Vec<&str> = s.split('=').collect();
		if out.len() != 2 {
			return Err("bad ext-file syntax".to_owned());
		}
		let file = read_to_string(&out[1]);
		match file {
			Ok(content) => Ok(Self {
				name: out[0].into(),
				value: content,
			}),
			Err(e) => Err(format!("{}", e)),
		}
	}
}

#[derive(Clap)]
#[clap(help_heading = "EXTERNAL VARIABLES")]
pub struct ExtVarOpts {
	/// Add string external variable.
	/// External variables are globally available so it is preferred
	/// to use top level arguments whenever it's possible.
	/// If [=data] is not set then it will be read from `name` env variable.
	/// Can be accessed from code via `std.extVar("name")`.
	#[clap(long, short = 'V', name = "name[=var data]", number_of_values = 1)]
	ext_str: Vec<ExtStr>,
	/// Read string external variable from file.
	/// See also `--ext-str`
	#[clap(long, name = "name=var path", number_of_values = 1)]
	ext_str_file: Vec<ExtFile>,
	/// Add external variable from code.
	/// See also `--ext-str`
	#[clap(long, name = "name[=var source]", number_of_values = 1)]
	ext_code: Vec<ExtStr>,
	/// Read string external variable from file.
	/// See also `--ext-str`
	#[clap(long, name = "name=var code path", number_of_values = 1)]
	ext_code_file: Vec<ExtFile>,
}
impl ConfigureState for ExtVarOpts {
	fn configure(&self, state: &EvaluationState) -> Result<()> {
		for ext in self.ext_str.iter() {
			state.add_ext_str((&ext.name as &str).into(), (&ext.value as &str).into());
		}
		for ext in self.ext_str_file.iter() {
			state.add_ext_str((&ext.name as &str).into(), (&ext.value as &str).into());
		}
		for ext in self.ext_code.iter() {
			state.add_ext_code((&ext.name as &str).into(), (&ext.value as &str).into())?;
		}
		for ext in self.ext_code_file.iter() {
			state.add_ext_code((&ext.name as &str).into(), (&ext.value as &str).into())?;
		}
		Ok(())
	}
}
