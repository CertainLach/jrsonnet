// All builtins should return results
#![allow(clippy::unnecessary_wraps)]

use format::{format_arr, format_obj};
use jrsonnet_interner::IStr;

use crate::{error::Result, function::CallLocation, State, Val};

pub mod format;

pub fn std_format(str: IStr, vals: Val) -> Result<String> {
	State::push(
		CallLocation::native(),
		|| format!("std.format of {str}"),
		|| {
			Ok(match vals {
				Val::Arr(vals) => format_arr(&str, &vals.evaluatedcc()?)?,
				Val::Obj(obj) => format_obj(&str, &obj)?,
				o => format_arr(&str, &[o])?,
			})
		},
	)
}
