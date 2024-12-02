use jrsonnet_interner::IStr;
use jrsonnet_parser::Source;

use crate::{
	function::{CallLocation, TlaArg},
	gc::GcHashMap,
	in_description_frame, with_state, Result, Val,
};

pub fn apply_tla(args: &GcHashMap<IStr, TlaArg>, val: Val) -> Result<Val> {
	Ok(if let Val::Func(func) = val {
		in_description_frame(
			|| "during TLA call".to_owned(),
			|| {
				func.evaluate(
					with_state(|s| {
						s.create_default_context(Source::new_virtual(
							"<top-level-arg>".into(),
							IStr::empty(),
						))
					}),
					CallLocation::native(),
					args,
					false,
				)
			},
		)?
	} else {
		val
	})
}
