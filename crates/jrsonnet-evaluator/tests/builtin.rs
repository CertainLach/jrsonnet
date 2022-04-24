mod common;

use std::path::PathBuf;

use gcmodule::Cc;
use jrsonnet_evaluator::{
	error::Result,
	function::{builtin, builtin::Builtin, CallLocation, FuncVal},
	gc::TraceBox,
	tb,
	typed::Typed,
	State, Val,
};

#[builtin]
fn a() -> Result<u32> {
	Ok(1)
}

#[test]
fn basic_function() -> Result<()> {
	let s = State::default();
	let a: a = a {};
	let v = u32::from_untyped(
		a.call(
			s.clone(),
			s.create_default_context(),
			CallLocation::native(),
			&(),
		)?,
		s.clone(),
	)?;

	ensure_eq!(v, 1);
	Ok(())
}

#[builtin]
fn native_add(a: u32, b: u32) -> Result<u32> {
	Ok(a + b)
}

#[test]
fn call_from_code() -> Result<()> {
	let s = State::default();
	s.with_stdlib();
	s.settings_mut().globals.insert(
		"nativeAdd".into(),
		Val::Func(FuncVal::StaticBuiltin(native_add::INST)),
	);

	let v = s.evaluate_snippet_raw(
		PathBuf::new().into(),
		"
            assert nativeAdd(1, 2) == 3;
            assert nativeAdd(100, 200) == 300;
            null
        "
		.into(),
	)?;
	ensure_val_eq!(s.clone(), v, Val::Null);
	Ok(())
}

#[builtin(fields(
    a: u32
))]
fn curried_add(this: &curried_add, b: u32) -> Result<u32> {
	Ok(this.a + b)
}

#[builtin]
fn curry_add(a: u32) -> Result<FuncVal> {
	Ok(FuncVal::Builtin(Cc::new(tb!(curried_add { a }))))
}

#[test]
fn nonstatic_builtin() -> Result<()> {
	let s = State::default();
	s.with_stdlib();
	s.settings_mut().globals.insert(
		"curryAdd".into(),
		Val::Func(FuncVal::StaticBuiltin(curry_add::INST)),
	);

	let v = s.evaluate_snippet_raw(
		PathBuf::new().into(),
		"
            local a = curryAdd(1);
            local b = curryAdd(4);

            assert a(2) == 3;
            assert a(200) == 201;

            assert b(2) == 6;
            assert b(200) == 204;
            null
        "
		.into(),
	)?;
	ensure_val_eq!(s.clone(), v, Val::Null);
	Ok(())
}
