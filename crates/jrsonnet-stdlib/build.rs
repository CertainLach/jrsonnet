use std::{borrow::Cow, env, fs::File, io::Write, path::Path};

use jrsonnet_parser::{parse, ParserSettings, Source};
use structdump::CodegenResult;

fn main() {
	let parsed = parse(
		include_str!("./src/std.jsonnet"),
		&ParserSettings {
			file_name: Source::new_virtual(
				Cow::Borrowed("<std>"),
				include_str!("./src/std.jsonnet").into(),
			),
		},
	)
	.expect("parse");

	let mut out = CodegenResult::default();

	let v = out.codegen(&parsed, true);

	{
		let out_dir = env::var("OUT_DIR").unwrap();
		let dest_path = Path::new(&out_dir).join("stdlib.rs");
		let mut f = File::create(&dest_path).unwrap();
		f.write_all(
			("#[allow(clippy::redundant_clone)]".to_owned() + &v.to_string())
				.replace(';', ";\n")
				.as_bytes(),
		)
		.unwrap();
	}
}
