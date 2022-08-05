use std::{borrow::Cow, env, fs::File, io::Write, path::Path};

use bincode::serialize;
use jrsonnet_parser::{parse, ParserSettings, Source};

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

	{
		let out_dir = env::var("OUT_DIR").unwrap();
		let dest_path = Path::new(&out_dir).join("stdlib.bincode");
		let mut f = File::create(&dest_path).unwrap();
		f.write_all(&serialize(&parsed).unwrap()).unwrap();
	}
}
