use std::path::PathBuf;

use jrsonnet_evaluator::{error::Result, throw_runtime, State, Val};

mod common;

#[test]
fn assert_positive() -> Result<()> {
	let s = State::default();
	s.with_stdlib();

	let v = s.evaluate_snippet_raw(PathBuf::new().into(), "assert 1 == 1: 'fail'; null".into())?;
	ensure_val_eq!(s.clone(), v, Val::Null);
	let v = s.evaluate_snippet_raw(PathBuf::new().into(), "std.assertEqual(1, 1)".into())?;
	ensure_val_eq!(s.clone(), v, Val::Bool(true));

	Ok(())
}

#[test]
fn assert_negative() -> Result<()> {
	let s = State::default();
	s.with_stdlib();

	{
		let e = match s
			.evaluate_snippet_raw(PathBuf::new().into(), "assert 1 == 2: 'fail'; null".into())
		{
			Ok(_) => throw_runtime!("assertion should fail"),
			Err(e) => e,
		};
		let e = s.stringify_err(&e);
		ensure!(e.starts_with("assert failed: fail\n"));
	}
	{
		let e = match s.evaluate_snippet_raw(PathBuf::new().into(), "std.assertEqual(1, 2)".into())
		{
			Ok(_) => throw_runtime!("assertion should fail"),
			Err(e) => e,
		};
		let e = s.stringify_err(&e);
		ensure!(e.starts_with("runtime error: Assertion failed. 1 != 2"))
	}

	Ok(())
}
