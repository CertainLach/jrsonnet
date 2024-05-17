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
	use json_structural_diff::JsonDiff;

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

			match (serde_json::from_str(&result), serde_json::from_str(&golden)) {
				(Err(_), Ok(_)) => assert_eq!(
					result,
					golden,
					"unexpected error for golden {}",
					entry.path().display()
				),
				(Ok(_), Err(_)) => assert_eq!(
					result,
					golden,
					"expected error for golden {}",
					entry.path().display()
				),
				(Ok(result), Ok(golden)) => {
					// Show diff relative to golden`.
					let diff = JsonDiff::diff_string(&golden, &result, false);
					if let Some(diff) = diff {
						panic!(
							"Result \n{result:#}\n\
								and golden \n{golden:#}\n\
								did not match structurally:\n{diff:#}\n\
								for golden {}",
							entry.path().display()
						);
					}
				}
				(Err(_), Err(_)) => {}
			};

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
