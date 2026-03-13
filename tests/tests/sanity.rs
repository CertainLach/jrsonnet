use jrsonnet_evaluator::{
	FileImportResolver, Result, State, Val, bail,
	trace::{CompactFormat, PathResolver, TraceFormat},
};
use jrsonnet_stdlib::ContextInitializer;

mod common;

#[test]
fn assert_positive() -> Result<()> {
	let mut s = State::builder();
	s.context_initializer(ContextInitializer::new(PathResolver::new_cwd_fallback()))
		.import_resolver(FileImportResolver::default());
	let s = s.build();

	let v = s.evaluate_snippet("snip".to_owned(), "assert 1 == 1: 'fail'; null")?;
	ensure_val_eq!(v, Val::Null);
	let v = s.evaluate_snippet("snip".to_owned(), "std.assertEqual(1, 1)")?;
	ensure_val_eq!(v, Val::Bool(true));

	Ok(())
}

#[test]
fn assert_negative() -> Result<()> {
	let mut s = State::builder();
	s.context_initializer(ContextInitializer::new(PathResolver::new_cwd_fallback()))
		.import_resolver(FileImportResolver::default());
	let s = s.build();

	let trace_format = CompactFormat::default();

	{
		let Err(e) = s.evaluate_snippet("snip".to_owned(), "assert 1 == 2: 'fail'; null") else {
			bail!("assertion should fail");
		};
		let e = trace_format.format(&e).unwrap();
		ensure!(e.starts_with("assert failed: fail\n"));
	}
	{
		let Err(e) = s.evaluate_snippet("snip".to_owned(), "std.assertEqual(1, 2)") else {
			bail!("assertion should fail")
		};
		let e = trace_format.format(&e).unwrap();
		ensure!(e.starts_with("runtime error: assertion failed: A != B\nA: 1\nB: 2\n"));
	}

	Ok(())
}
