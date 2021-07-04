use bincode::serialize;
use jrsonnet_parser::{parse, ParserSettings};
use jrsonnet_stdlib::STDLIB_STR;
use std::{
	env,
	fs::File,
	io::Write,
	path::{Path, PathBuf},
};

fn main() {
	let parsed = parse(
		STDLIB_STR,
		&ParserSettings {
			file_name: PathBuf::from("std.jsonnet").into(),
			loc_data: true,
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
