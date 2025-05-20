// All builtins should return results
#![allow(clippy::unnecessary_wraps)]

pub use format::FormatError;
use format::{format_arr, format_obj};

use crate::{function::CallLocation, in_frame, Result, Val};

mod format;

pub fn std_format(str: &str, vals: Val) -> Result<String> {
	in_frame(
		CallLocation::native(),
		|| format!("std.format of {str}"),
		|| {
			Ok(match vals {
				Val::Arr(vals) => format_arr(str, &vals.iter().collect::<Result<Vec<_>>>()?)?,
				Val::Obj(obj) => format_obj(str, &obj)?,
				o => format_arr(str, &[o])?,
			})
		},
	)
}
