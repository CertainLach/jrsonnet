use std::{
	fs, io,
	path::{Path, PathBuf},
};

use jrsonnet_evaluator::{
	manifest::JsonFormat,
	trace::{CompactFormat, PathResolver, TraceFormat},
	FileImportResolver, State,
};
use jrsonnet_stdlib::StateExt;

mod common;

fn run(file: &Path) -> String {
	let s = State::default();
	s.with_stdlib();
	common::with_test(&s);
	s.set_import_resolver(FileImportResolver::default());
	let trace_format = CompactFormat {
		resolver: PathResolver::FileName,
		max_trace: 20,
		padding: 4,
	};

	let v = match s.import(file) {
		Ok(v) => v,
		Err(e) => return trace_format.format(&e).unwrap(),
	};
	match v.manifest(JsonFormat::default()) {
		Ok(v) => v.to_string(),
		Err(e) => trace_format.format(&e).unwrap(),
	}
}

#[test]
fn test() -> io::Result<()> {
	let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	root.push("golden");

	for entry in fs::read_dir(&root)? {
		let entry = entry?;
		if !entry.path().extension().map_or(false, |e| e == "jsonnet") {
			continue;
		}

		let result = run(&entry.path());

		let mut golden_path = entry.path();
		golden_path.set_extension("jsonnet.golden");

		if !golden_path.exists() {
			fs::write(golden_path, &result)?;
		} else {
			let golden = fs::read_to_string(golden_path)?;

			assert_eq!(
				result,
				golden,
				"golden didn't match for {}",
				entry.path().display()
			)
		}
	}

	Ok(())
}
