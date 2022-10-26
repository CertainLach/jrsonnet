use jrsonnet_evaluator::{error::Result, State};
use jrsonnet_stdlib::StateExt;

mod common;

#[test]
fn as_native() -> Result<()> {
	let s = State::default();
	s.with_stdlib();

	let val = s.evaluate_snippet("snip".to_owned(), r#"function(a, b) a + b"#)?;
	let func = val.as_func().expect("this is function");

	let native = func.into_native::<((u32, u32), u32)>();

	ensure_eq!(native(1, 2)?, 3);
	ensure_eq!(native(3, 4)?, 7);

	Ok(())
}
