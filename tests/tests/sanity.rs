use jrsonnet_evaluator::{
	bail,
	trace::{CompactFormat, TraceFormat},
	Result, State, Val,
};
use jrsonnet_stdlib::StateExt;

mod common;

#[test]
fn assert_positive() -> Result<()> {
	let s = State::default();
	s.with_stdlib();

	let v = s.evaluate_snippet("snip".to_owned(), "assert 1 == 1: 'fail'; null")?;
	ensure_val_eq!(v, Val::Null);
	let v = s.evaluate_snippet("snip".to_owned(), "std.assertEqual(1, 1)")?;
	ensure_val_eq!(v, Val::Bool(true));

	Ok(())
}

#[test]
fn assert_negative() -> Result<()> {
	let s = State::default();
	s.with_stdlib();
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
		ensure!(e.starts_with("runtime error: Assertion failed. 1 != 2"))
	}

	Ok(())
}
