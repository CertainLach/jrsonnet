use std::{
	fs, io,
	path::{Path, PathBuf},
};

use jrsonnet_evaluator::{
	trace::{CompactFormat, TraceFormat},
	FileImportResolver, State, Val,
};
use jrsonnet_stdlib::StateExt;

mod common;

fn run(file: &Path) {
	let s = State::default();
	s.with_stdlib();
	common::with_test(&s);
	s.set_import_resolver(Box::new(FileImportResolver::default()));
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
	};
}

#[test]
fn test() -> io::Result<()> {
	let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	root.push("suite");

	for entry in fs::read_dir(&root)? {
		let entry = entry?;
		if !entry.path().extension().map_or(false, |e| e == "jsonnet") {
			continue;
		}

		run(&entry.path());
	}

	Ok(())
}
