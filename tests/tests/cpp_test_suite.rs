use std::{
	env, fs,
	io::{self, ErrorKind},
	path::{Path, PathBuf},
};

use jrsonnet_evaluator::{
	FileImportResolver, IStr, ObjValueBuilder, Result, State, Val, apply_tla,
	function::builtin,
	gc::WithCapacityExt as _,
	manifest::JsonFormat,
	rustc_hash::FxHashMap,
	stack::limit_stack_depth,
	tla::TlaArg,
	trace::{CompactFormat, PathResolver, TraceFormat},
};
use jrsonnet_stdlib::ContextInitializer;
mod common;
use common::ContextInitializer as TestContextInitializer;
use rstest::rstest;

#[builtin]
fn json_to_string(x: Val) -> Result<String> {
	x.manifest(JsonFormat::minify(
		#[cfg(feature = "exp-preserve-order")]
		false,
	))
}

fn run(file: &Path, root: &Path) -> String {
	let mut s = State::builder();

	let resolver = PathResolver::Relative(root.to_owned());
	let std_context = ContextInitializer::new(resolver.clone());
	// C++ test suite
	std_context.add_ext_str("var1".into(), "test".into());
	std_context
		.add_ext_code("var2", "{x:1,y:2}")
		.expect("code is valid");

	// Golang test suite
	std_context.add_native("jsonToString", json_to_string {});
	std_context
		.add_ext_code("codeVar", "3+3")
		.expect("code is valid");
	std_context.add_ext_str("stringVar".into(), "2 + 2".into());
	std_context
		.add_ext_code(
			"selfRecursiveVar",
			r#"[42, std.extVar("selfRecursiveVar")[0] + 1]"#,
		)
		.expect("code is valid");
	std_context
		.add_ext_code(
			"mutuallyRecursiveVar1",
			r#"[42, std.extVar("mutuallyRecursiveVar2")[0] + 1]"#,
		)
		.expect("code is valid");
	std_context
		.add_ext_code(
			"mutuallyRecursiveVar2",
			r#"[42, std.extVar("mutuallyRecursiveVar1")[0] + 1]"#,
		)
		.expect("code is valid");

	s.context_initializer((std_context, TestContextInitializer))
		.import_resolver(FileImportResolver::default());
	let s = s.build();

	let _entered = s.enter();

	let trace_format = CompactFormat {
		resolver,
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
		v = apply_tla(&args, v).expect("failed to apply tla");
	} else {
		v = match apply_tla(&FxHashMap::new(), v) {
			Ok(v) => v,
			Err(e) => return trace_format.format(&e).unwrap(),
		};
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

fn check(file: &Path, root: &Path, override_root: &Path) {
	let _stack = if file
		.file_stem()
		.is_some_and(|e| e == "recursive_function" || e == "tailstrict" || e == "tailstrict5")
	{
		Some(limit_stack_depth(100_000))
	} else {
		None
	};

	let result = run(file, root);

	let mut golden_path = file.to_path_buf();
	golden_path.set_extension("jsonnet.golden");

	let mut golden_path2 = file.to_path_buf();
	golden_path2.set_extension("golden");

	let golden_override = override_root.join(golden_path.file_name().expect("file has basename"));

	// .jsonnet.golden for C++ tests
	let mut golden = read_file(&golden_path).expect("read golden");
	// .golden for Go tests
	if golden.is_none()
		&& let Some(v) = read_file(&golden_path2).expect("read golden")
	{
		golden = Some(v);
	}

	// Any of them can be overriden by overrides
	if let Some(v) = read_file(&golden_override).expect("read golden override") {
		golden = Some(v);
	}

	// Otherwise assume test should just not fail and return true.
	let golden = golden.unwrap_or_else(|| "true".to_owned());

	let update_golden_path = &golden_override;

	match (
		serde_json::from_str::<serde_json::Value>(&result),
		serde_json::from_str::<serde_json::Value>(&golden),
	) {
		(Err(_), Ok(_)) => panic!(
			"unexpected error for golden {}:\n<got>\n{result}\n</got>\n<golden>\n{golden}\n</golden>",
			file.display()
		),
		(Ok(_), Err(_)) => panic!(
			"expected error for golden {}:\n<got>\n{result}\n</got>\n<golden>\n{golden}\n</golden>",
			file.display()
		),
		(Ok(result_v), Ok(golden_v)) => {
			if result_v != golden_v {
				if env::var_os("UPDATE_GOLDEN").is_some() {
					fs::write(update_golden_path, result).expect("write golden override");
				} else {
					panic!(
						"Result \n{result_v:#}\n\
							and golden \n{golden_v:#}\n\
							did not match structurally\n\
							for golden {}",
						file.display()
					);
				}
			}
		}
		(Err(_), Err(_)) => {
			if result != golden.trim_end() {
				if env::var_os("UPDATE_GOLDEN").is_some() {
					fs::write(update_golden_path, result).expect("write golden override");
				} else {
					panic!(
						"golden didn't match for {}:\n<got>\n{result}\n</got>\n<golden>\n{golden}\n</golden>",
						file.display()
					)
				}
			}
		}
	}
}

#[rstest]
fn cpp_test_suite(
	#[base_dir = "$CPP_JSONNET_FOR_TESTS/test_suite"]
	#[files("*.jsonnet")]
	// Parser fails with stack overflow. While is a bug, this is a too unusual
	// thing to run untrusted jsonnet code? Will be fixed with nom/rowan.
	#[exclude("^error\\.parse\\.deep_array_nesting\\.jsonnet$")]
	// Too slow to throw due to how lazyness is implemented in jrsonnet
	#[exclude("^error\\.recursive_object_non_term\\.jsonnet$")]
	// In jrsonnet returns the one passed argument, works as Rust's dbg!()
	#[exclude("^error\\.trace_one_param\\.jsonnet$")]
	// In jrsonnet can display any value
	#[exclude("^error\\.trace_two_param\\.jsonnet$")]
	// Depends on unsafe handling of strings as arrays in jsonnet stdlib
	#[exclude("^invariant_manifest\\.jsonnet$")]
	// Little bit hard to capture trace logs in this test suite at this moment
	#[exclude("^trace\\.jsonnet$")]
	file: PathBuf,
) {
	let root = PathBuf::from(env!("CPP_JSONNET_FOR_TESTS")).join("test_suite");
	let override_root =
		PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cpp_test_suite_golden_override");
	check(&file, &root, &override_root);
}

#[rstest]
fn go_test_suite(
	#[base_dir = "$GO_JSONNET_FOR_TESTS/testdata"]
	#[files("*.jsonnet")]
	// Something is wrong, go-jsonnet skips safe integer range check here
	#[exclude("^bitwise_or9\\.jsonnet$")]
	// Bad check: https://github.com/databricks/sjsonnet/issues/793#issuecomment-4323153709
	#[exclude("^builtinBase64_string_high_codepoint\\.jsonnet$")]
	// Split by empty string is string characters, same as everywhere else
	#[exclude("^builtinSplitLimitR6\\.jsonnet$")]
	// escapeStringJson only accepts string in jrsonnet
	#[exclude("^builtin_escapeStringJson\\.jsonnet$")]
	// golang float formatting is inefficient and not portable
	#[exclude("^builtin_manifestTomlEx\\.jsonnet$")]
	// golang escapes "e" yaml key, does it think it is float?
	#[exclude("^builtin_manifestYamlDoc\\.jsonnet$")]
	// multi output is a CLI part, not an interpreter.
	#[exclude("^multi\\.jsonnet$")]
	#[exclude("^multi_no_newline\\.jsonnet$")]
	#[exclude("^multi_no_newline_string_output\\.jsonnet$")]
	#[exclude("^multi_string_output\\.jsonnet$")]
	// Golang fails with max stack frames exceeded error
	#[exclude("^std\\.makeArray_recursive_evalutation_order_matters\\.jsonnet$")]
	// Tailstrict semantics is partially unspecified
	#[exclude("^tailstrict3\\.jsonnet$")]
	// Jrsonnet has this overload
	#[exclude("^number_times_string\\.jsonnet$")]
	// Jrsonnet has this overload
	#[exclude("^string_times_number\\.jsonnet$")]
	file: PathBuf,
) {
	let root = PathBuf::from(env!("GO_JSONNET_FOR_TESTS")).join("testdata");
	let override_root =
		PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("go_testdata_golden_override");
	check(&file, &root, &override_root);
}
