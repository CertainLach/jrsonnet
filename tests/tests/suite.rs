use std::path::{Path, PathBuf};

use jrsonnet_evaluator::{
	FileImportResolver, State, Val,
	trace::{CompactFormat, PathResolver, TraceFormat},
};
use jrsonnet_stdlib::ContextInitializer;

mod common;
use common::ContextInitializer as TestContextInitializer;
use rstest::rstest;

fn run(file: &Path) {
	let mut s = State::builder();
	s.context_initializer((
		ContextInitializer::new(PathResolver::new_cwd_fallback()),
		TestContextInitializer,
	))
	.import_resolver(FileImportResolver::default());
	let s = s.build();

	let trace_format = CompactFormat::default();

	match s.import(file) {
		Ok(Val::Bool(true)) => {}
		Ok(Val::Bool(false)) => panic!("test {} returned false", file.display()),
		Ok(_) => panic!("test {} returned wrong type as result", file.display()),
		Err(e) => panic!(
			"test {} failed:\n{}",
			file.display(),
			trace_format.format(&e).unwrap()
		),
	}
}

#[rstest]
fn test_suite(
	#[base_dir = "suite"]
	#[files("*.jsonnet")]
	path: PathBuf,
) {
	run(&path);
}
