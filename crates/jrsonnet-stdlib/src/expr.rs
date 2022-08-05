use std::borrow::Cow;

use jrsonnet_parser::{LocExpr, ParserSettings, Source};

pub const STDLIB_STR: &str = include_str!("./std.jsonnet");

pub fn stdlib_expr() -> LocExpr {
	#[cfg(feature = "serialized-stdlib")]
	{
		// Should not panic, stdlib.bincode is generated in build.rs
		return bincode::deserialize(include_bytes!(concat!(env!("OUT_DIR"), "/stdlib.bincode")))
			.unwrap();
	}

	jrsonnet_parser::parse(
		STDLIB_STR,
		&ParserSettings {
			file_name: Source::new_virtual(Cow::Borrowed("<std>"), STDLIB_STR.into()),
		},
	)
	.unwrap()
}
