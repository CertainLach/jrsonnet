use jrsonnet_evaluator::{error::Result, throw, State, Val};
use jrsonnet_stdlib::StateExt;

mod common;

#[test]
fn assert_positive() -> Result<()> {
	let s = State::default();
	s.with_stdlib();

	let v = s.evaluate_snippet("snip".to_owned(), "assert 1 == 1: 'fail'; null")?;
	ensure_val_eq!(s, v, Val::Null);
	let v = s.evaluate_snippet("snip".to_owned(), "std.assertEqual(1, 1)")?;
	ensure_val_eq!(s, v, Val::Bool(true));

	Ok(())
}

#[test]
fn assert_negative() -> Result<()> {
	let s = State::default();
	s.with_stdlib();

	{
		let e = match s.evaluate_snippet("snip".to_owned(), "assert 1 == 2: 'fail'; null") {
			Ok(_) => throw!("assertion should fail"),
			Err(e) => e,
		};
		let e = s.stringify_err(&e);
		ensure!(e.starts_with("assert failed: fail\n"));
	}
	{
		let e = match s.evaluate_snippet("snip".to_owned(), "std.assertEqual(1, 2)") {
			Ok(_) => throw!("assertion should fail"),
			Err(e) => e,
		};
		let e = s.stringify_err(&e);
		ensure!(e.starts_with("runtime error: Assertion failed. 1 != 2"))
	}

	Ok(())
}
