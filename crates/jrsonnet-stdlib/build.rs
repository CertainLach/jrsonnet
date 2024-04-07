fn main() {
	#[cfg(feature = "codegenerated-stdlib")]
	{
		use std::{env, fs::File, io::Write, path::Path};

		use jrsonnet_parser::{parse, ParserSettings, Source};
		use structdump::CodegenResult;

		let parsed = parse(
			include_str!("./src/std.jsonnet"),
			&ParserSettings {
				source: Source::new_virtual(
					"<std>".into(),
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
			let mut f = File::create(dest_path).unwrap();
			f.write_all(
				("#[allow(clippy::redundant_clone, clippy::similar_names)]".to_owned()
					+ &v.to_string())
					.as_bytes(),
			)
			.unwrap();
		}
	}
}
