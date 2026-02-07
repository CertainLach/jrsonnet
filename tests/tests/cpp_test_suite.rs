use std::{
	env, fs,
	io::{self, ErrorKind},
	path::{Path, PathBuf},
};

use jrsonnet_evaluator::{
	apply_tla,
	function::TlaArg,
	gc::WithCapacityExt as _,
	manifest::JsonFormat,
	rustc_hash::FxHashMap,
	trace::{CompactFormat, PathResolver, TraceFormat},
	FileImportResolver, IStr, ObjValueBuilder, State, Val,
};
use jrsonnet_stdlib::ContextInitializer;
mod common;
use common::ContextInitializer as TestContextInitializer;

fn run(file: &Path, root: &Path) -> String {
	let mut s = State::builder();

	let std_context = ContextInitializer::new(PathResolver::Relative(root.to_owned()));
	std_context.add_ext_str("var1".into(), "test".into());
	std_context
		.add_ext_code("var2".into(), "{x:1,y:2}")
		.expect("code is valid");

	s.context_initializer((std_context, TestContextInitializer))
		.import_resolver(FileImportResolver::default());
	let s = s.build();

	let trace_format = CompactFormat {
		resolver: PathResolver::FileName,
		max_trace: 20,
		padding: 4,
	};

	let mut v = match s.import(file) {
		Ok(v) => v,
		Err(e) => return trace_format.format(&e).unwrap(),
	};

	if file
		.file_name()
		.expect("file has basename")
		.to_str()
		.expect("jsonnet testsuite has ascii names")
		.starts_with("tla.")
	{
		let mut args = FxHashMap::new();
		args.insert(IStr::from("var1"), TlaArg::String("test".into()));
		args.insert(
			IStr::from("var2"),
			TlaArg::Val({
				let mut o = ObjValueBuilder::new();

				o.field("x").value(Val::num(1));
				o.field("y").value(Val::num(2));

				Val::Obj(o.build())
			}),
		);
		v = apply_tla(s, &args, v).expect("failed to apply tla");
	}

	match v.manifest(JsonFormat::default()) {
		Ok(v) => v,
		Err(e) => trace_format.format(&e).unwrap(),
	}
}

fn read_file(path: &Path) -> io::Result<Option<String>> {
	match fs::read_to_string(path) {
		Ok(v) => Ok(Some(v)),
		Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
		Err(e) => Err(e),
	}
}

const SKIPPED: &[&str] = &[
	// Parser fails with stack overflow. While is a bug, this is a too unusual
	// thing to run untrusted jsonnet code? Will be fixed with nom/rowan.
	"error.parse.deep_array_nesting.jsonnet",
	// Runtime error in jrsonnet
	"error.parse.object_local_clash.jsonnet",
	// Too slow to throw due to how lazyness is implemented in jrsonnet
	"error.recursive_object_non_term.jsonnet",
	// In jrsonnet returns the one passed argument, works as Rust's dbg!()
	"error.trace_one_param.jsonnet",
	// In jrsonnet can display any value
	"error.trace_two_param.jsonnet",
	// Depends on unsafe handling of strings as arrays in jsonnet stdlib
	"invariant_manifest.jsonnet",
	// Little bit hard to capture trace logs in this test suite at this moment
	"trace.jsonnet",
];

#[test]
fn cpp_test_suite() -> io::Result<()> {
	use json_structural_diff::JsonDiff;

	let root_tests = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	let root = root_tests.join("cpp_test_suite");
	let root_override = root_tests.join("cpp_test_suite_golden_override");

	for entry in fs::read_dir(&root).map_err(|e| io::Error::new(ErrorKind::Other, format!("failed to enumerate cpp_test_suite dir (Note: it needs to be cloned from C++ jsonnet repo for this test): {e}")))? {
		let entry = entry?;
		if !entry.path().extension().map_or(false, |e| e == "jsonnet") {
			continue;
		}

		if entry
			.path()
			.file_name()
			.and_then(|v| v.to_str())
			.map_or(false, |v| SKIPPED.contains(&v))
		{
			continue;
		}

		let result = run(&entry.path(), &root);

		let mut golden_path = entry.path();
		golden_path.set_extension("jsonnet.golden");
		let golden_override =
			root_override.join(&golden_path.file_name().expect("file has basename"));

		let mut golden = read_file(&golden_path)?;

		if let Some(golden_path) = read_file(&golden_override)? {
			golden = Some(golden_path);
		}

		let golden = golden.unwrap_or_else(|| "true".to_owned());

		match (serde_json::from_str(&result), serde_json::from_str(&golden)) {
			(Err(_), Ok(_)) => panic!(
				"unexpected error for golden {}:\n<got>\n{result}\n</got>\n<golden>\n{golden}\n</golden>",
				entry.path().display()
			),
			(Ok(_), Err(_)) => panic!(
				"expected error for golden {}:\n<got>\n{result}\n</got>\n<golden>\n{golden}\n</golden>",
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
			(Err(_), Err(_)) => {
				if result != golden.trim_end() {
					if env::var_os("UPDATE_GOLDEN").is_some() {
						fs::write(golden_override, result)?;
					} else {
						panic!(
						"golden didn't match for {}:\n<got>\n{result}\n</got>\n<golden>\n{golden}\n</golden>",
						entry.path().display()
					)
					}
				}
			}
		};
	}

	Ok(())
}
