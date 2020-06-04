use bincode::serialize;
use jsonnet_parser::{parse, ParserSettings};
use jsonnet_stdlib::STDLIB_STR;
use std::{env, fs::File, io::Write, path::Path};

fn main() {
	let parsed = parse(
		STDLIB_STR,
		&ParserSettings {
			file_name: "std.jsonnet".to_owned(),
			loc_data: true,
		},
	)
	.expect("parse");

	let out_dir = env::var("OUT_DIR").unwrap();
	let dest_path = Path::new(&out_dir).join("stdlib.bincode");
	let mut f = File::create(&dest_path).unwrap();
	f.write_all(&serialize(&parsed).expect("serialize"))
		.unwrap();
}
