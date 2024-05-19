use jrsonnet_evaluator::{trace::PathResolver, FileImportResolver, Result, State};
use jrsonnet_stdlib::ContextInitializer;

mod common;

#[test]
fn as_native() -> Result<()> {
	let mut s = State::builder();
	s.context_initializer(ContextInitializer::new(PathResolver::new_cwd_fallback()))
		.import_resolver(FileImportResolver::default());
	let s = s.build();

	let val = s.evaluate_snippet("snip".to_owned(), r"function(a, b) a + b")?;
	let func = val.as_func().expect("this is function");

	let native = func.into_native::<((u32, u32), u32)>();

	ensure_eq!(native(1, 2)?, 3);
	ensure_eq!(native(3, 4)?, 7);

	Ok(())
}
