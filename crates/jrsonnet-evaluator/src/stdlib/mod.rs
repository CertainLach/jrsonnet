// All builtins should return results
#![allow(clippy::unnecessary_wraps)]

use format::{format_arr, format_obj};
use jrsonnet_interner::IStr;

use crate::{error::Result, function::CallLocation, State, Val};

pub mod format;
pub mod manifest;

pub fn std_format(s: State, str: IStr, vals: Val) -> Result<String> {
	s.push(
		CallLocation::native(),
		|| format!("std.format of {str}"),
		|| {
			Ok(match vals {
				Val::Arr(vals) => format_arr(s.clone(), &str, &vals.evaluated(s.clone())?)?,
				Val::Obj(obj) => format_obj(s.clone(), &str, &obj)?,
				o => format_arr(s.clone(), &str, &[o])?,
			})
		},
	)
}
