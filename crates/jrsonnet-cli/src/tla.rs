use std::{
	ffi::{OsStr, OsString},
	os::unix::ffi::OsStrExt,
	path::{Path, PathBuf},
};

use clap::Parser;
use jrsonnet_evaluator::{
	bail,
	error::{ErrorKind, Result},
	function::TlaArg,
	gc::GcHashMap,
	val::ThunkValue,
	IStr, State, Thunk, Val,
};
use jrsonnet_gcmodule::Trace;
use jrsonnet_parser::{ParserSettings, Source};

use crate::ExtStr;

#[derive(Parser)]
#[clap(next_help_heading = "TOP LEVEL ARGUMENTS")]
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
	tla_str_file: Vec<OsString>,
	/// Add top level argument from code.
	/// See also `--tla-str`
	#[clap(long, name = "name[=tla source]", number_of_values = 1)]
	tla_code: Vec<ExtStr>,
	/// Read top level argument code from file.
	/// See also `--tla-str`
	#[clap(long, name = "name=tla code path", number_of_values = 1)]
	tla_code_file: Vec<OsString>,
}
impl TlaOpts {
	pub fn into_args_in(self, state: &State) -> Result<GcHashMap<IStr, TlaArg>> {
		let mut out = GcHashMap::new();
		for (name, value) in self.tla_str.iter().map(|c| (&c.name, &c.value)) {
			out.insert(name.into(), TlaArg::String(value.into()));
		}
		for file in self.tla_str_file {
			let (key, path) = parse_named_tla_path(&file)?;
			out.insert(
				key.into(),
				TlaArg::Lazy(Thunk::new(ImportStrThunk {
					state: state.clone(),
					path,
				})),
			);
		}
		for (name, code) in self.tla_code.iter().map(|c| (&c.name, &c.value)) {
			let source = Source::new_virtual(format!("<top-level-arg:{name}>").into(), code.into());
			out.insert(
				name.into(),
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
		for file in self.tla_code_file {
			let (key, path) = parse_named_tla_path(&file)?;
			out.insert(
				key.into(),
				TlaArg::Lazy(Thunk::new(ImportCodeThunk {
					state: state.clone(),
					path,
				})),
			);
		}
		Ok(out)
	}
}

fn parse_named_tla_path(raw: &OsString) -> Result<(&str, PathBuf)> {
	let mut parts = raw.as_bytes().splitn(2, |&byte| byte == b'=');
	let Some(key) = parts.next() else {
		bail!("No TLA key was specified");
	};

	let Ok(key) = std::str::from_utf8(key) else {
		bail!("Invalid TLA map");
	};
	Ok(if let Some(value) = parts.next() {
		(key, Path::new(OsStr::from_bytes(value)).to_owned())
	} else {
		(key, std::env::var_os(key).unwrap_or_default().into())
	})
}

#[derive(Trace)]
struct ImportStrThunk {
	path: PathBuf,
	state: State,
}
impl ThunkValue for ImportStrThunk {
	type Output = Val;

	fn get(self: Box<Self>) -> Result<Self::Output> {
		self.state.import_str(self.path).map(|s| Val::Str(s.into()))
	}
}

#[derive(Trace)]
struct ImportCodeThunk {
	path: PathBuf,
	state: State,
}
impl ThunkValue for ImportCodeThunk {
	type Output = Val;

	fn get(self: Box<Self>) -> Result<Self::Output> {
		self.state.import(self.path)
	}
}
