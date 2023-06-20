use std::{fs::read_to_string, str::FromStr};

use clap::Parser;
use jrsonnet_evaluator::{error::Result, trace::PathResolver, State};
use jrsonnet_stdlib::ContextInitializer;

#[derive(Clone)]
pub struct ExtStr {
	pub name: String,
	pub value: String,
}

/// Parses a string like `name=<value>`, or `name` and reads value from env variable.
/// With no value it will be read from env variable.
/// If env variable is not found then it will be an error.
/// Value can contain `=` symbol.
///
/// ```
/// use std::str::FromStr;
/// use jrsonnet_cli::ExtStr;
///
/// let ext = ExtStr::from_str("name=value").unwrap();
/// assert_eq!(ext.name, "name");
/// assert_eq!(ext.value, "value");
///
/// std::env::set_var("name", "value");
///
/// let ext = ExtStr::from_str("name").unwrap();
/// assert_eq!(ext.name, "name");
/// assert_eq!(ext.value, "value");
///
/// let ext = ExtStr::from_str("name=value=with=equals").unwrap();
/// assert_eq!(ext.name, "name");
/// assert_eq!(ext.value, "value=with=equals");
/// ```
///
impl FromStr for ExtStr {
	type Err = &'static str;

	fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
		match s.find('=') {
			Some(idx) => Ok(ExtStr {
				name: s[..idx].to_owned(),
				value: s[idx + 1..].to_owned(),
			}),
			None => Ok(ExtStr {
				name: s.to_owned(),
				value: std::env::var(s).or(Err("missing env var"))?,
			}),
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
			Err(e) => Err(format!("{e}")),
		}
	}
}

#[derive(Parser)]
#[clap(next_help_heading = "STANDARD LIBRARY")]
pub struct StdOpts {
	/// Disable standard library.
	/// By default standard library will be available via global `std` variable.
	#[clap(long)]
	no_stdlib: bool,
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
impl StdOpts {
	pub fn context_initializer(&self, s: &State) -> Result<Option<ContextInitializer>> {
		if self.no_stdlib {
			return Ok(None);
		}
		let ctx = ContextInitializer::new(s.clone(), PathResolver::new_cwd_fallback());
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
		Ok(Some(ctx))
	}
}
