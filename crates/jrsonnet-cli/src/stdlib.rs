use std::{fs::read_to_string, str::FromStr};

use clap::Parser;
use jrsonnet_evaluator::{error::Result, trace::PathResolver, State};

use crate::ConfigureState;

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
		let file = read_to_string(out[1]);
		match file {
			Ok(content) => Ok(Self {
				name: out[0].into(),
				value: content,
			}),
			Err(e) => Err(format!("{}", e)),
		}
	}
}

#[derive(Parser)]
#[clap(next_help_heading = "STANDARD LIBRARY")]
pub struct StdOpts {
	/// Disable standard library.
	/// By default standard library will be available via global `std` variable.
	/// Note that standard library will still be loaded
	/// if chosen manifestification method is not `none`.
	#[clap(long)]
	no_stdlib: bool,
	/// Add string external variable.
	/// External variables are globally available so it is preferred
	/// to use top level arguments whenever it's possible.
	/// If [=data] is not set then it will be read from `name` env variable.
	/// Can be accessed from code via `std.extVar("name")`.
	#[clap(
		long,
		short = 'V',
		name = "name[=var data]",
		number_of_values = 1,
		multiple_occurrences = true
	)]
	ext_str: Vec<ExtStr>,
	/// Read string external variable from file.
	/// See also `--ext-str`
	#[clap(
		long,
		name = "name=var path",
		number_of_values = 1,
		multiple_occurrences = true
	)]
	ext_str_file: Vec<ExtFile>,
	/// Add external variable from code.
	/// See also `--ext-str`
	#[clap(
		long,
		name = "name[=var source]",
		number_of_values = 1,
		multiple_occurrences = true
	)]
	ext_code: Vec<ExtStr>,
	/// Read string external variable from file.
	/// See also `--ext-str`
	#[clap(
		long,
		name = "name=var code path",
		number_of_values = 1,
		multiple_occurrences = true
	)]
	ext_code_file: Vec<ExtFile>,
}
impl ConfigureState for StdOpts {
	fn configure(&self, s: &State) -> Result<()> {
		if self.no_stdlib {
			return Ok(());
		}
		let ctx =
			jrsonnet_stdlib::ContextInitializer::new(s.clone(), PathResolver::new_cwd_fallback());
		for ext in self.ext_str.iter() {
			ctx.add_ext_str((&ext.name as &str).into(), (&ext.value as &str).into());
		}
		for ext in self.ext_str_file.iter() {
			ctx.add_ext_str((&ext.name as &str).into(), (&ext.value as &str).into());
		}
		for ext in self.ext_code.iter() {
			ctx.add_ext_code(&ext.name as &str, &ext.value as &str)?;
		}
		for ext in self.ext_code_file.iter() {
			ctx.add_ext_code(&ext.name as &str, &ext.value as &str)?;
		}
		s.settings_mut().context_initializer = Box::new(ctx);
		Ok(())
	}
}
