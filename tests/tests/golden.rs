use std::{
	fs, io,
	path::{Path, PathBuf},
};

use jrsonnet_evaluator::{
	trace::{CompactFormat, PathResolver},
	FileImportResolver, State,
};
use jrsonnet_stdlib::StateExt;

mod common;

fn run(root: &Path, file: &Path) -> String {
	let s = State::default();
	s.set_trace_format(Box::new(CompactFormat {
		resolver: PathResolver::Relative(root.to_owned()),
		padding: 3,
	}));
	s.with_stdlib();
	common::with_test(&s);
	s.set_import_resolver(Box::new(FileImportResolver::default()));

	let v = match s.import(file) {
		Ok(v) => v,
		Err(e) => return s.stringify_err(&e),
	};
	match v.to_json(
		s.clone(),
		3,
		#[cfg(feature = "exp-preserve-order")]
		false,
	) {
		Ok(v) => v.to_string(),
		Err(e) => s.stringify_err(&e),
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

		let result = run(&root, &entry.path());

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
