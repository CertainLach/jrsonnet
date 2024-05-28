use jrsonnet_interner::IStr;
use jrsonnet_parser::Source;

use crate::{
	function::{ArgsLike, CallLocation},
	in_description_frame, Result, State, Val,
};

pub fn apply_tla<A: ArgsLike>(s: State, args: &A, val: Val) -> Result<Val> {
	Ok(if let Val::Func(func) = val {
		in_description_frame(
			|| "during TLA call".to_owned(),
			|| {
				func.evaluate(
					s.create_default_context(Source::new_virtual(
						"<top-level-arg>".into(),
						IStr::empty(),
					)),
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
