use std::{collections::HashMap, hash::BuildHasher};

use jrsonnet_interner::IStr;

use crate::{
	function::{CallLocation, PreparedFuncVal, TlaArg},
	in_description_frame, Result, Val,
};

pub fn apply_tla<H: BuildHasher>(args: &HashMap<IStr, TlaArg, H>, val: Val) -> Result<Val> {
	Ok(if let Val::Func(func) = val {
		in_description_frame(
			|| "during TLA call".to_owned(),
			|| {
				let mut names = Vec::with_capacity(args.len());
				let mut values = Vec::with_capacity(args.len());
				for (name, value) in args {
					names.push(name.clone());
					values.push(value.evaluate()?);
				}
				let prepared = PreparedFuncVal::new(func, 0, &names)?;
				prepared.call(CallLocation::native(), &[], &values)
			},
		)?
	} else {
		val
	})
}
