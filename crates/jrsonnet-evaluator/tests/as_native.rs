use std::path::PathBuf;

use jrsonnet_evaluator::{error::Result, State};

mod common;

#[test]
fn as_native() -> Result<()> {
	let s = State::default();
	s.with_stdlib();

	let val = s.evaluate_snippet_raw(PathBuf::new().into(), r#"function(a, b) a + b"#.into())?;
	let func = val.as_func().expect("this is function");

	let native = func.into_native::<((u32, u32), u32)>();

	ensure_eq!(native(s.clone(), 1, 2)?, 3);
	ensure_eq!(native(s, 3, 4)?, 7);

	Ok(())
}
