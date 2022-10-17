mod common;

use jrsonnet_evaluator::{
	error::Result,
	function::{builtin, builtin::Builtin, CallLocation, FuncVal},
	tb,
	typed::Typed,
	Context, State, Thunk, Val,
};
use jrsonnet_gcmodule::Cc;
use jrsonnet_stdlib::StateExt;

#[builtin]
fn a() -> Result<u32> {
	Ok(1)
}

#[test]
fn basic_function() -> Result<()> {
	let s = State::default();
	let a: a = a {};
	let v = u32::from_untyped(
		a.call(s.clone(), Context::new(), CallLocation::native(), &())?,
		s,
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
	s.add_global(
		"nativeAdd".into(),
		Thunk::evaluated(Val::Func(FuncVal::StaticBuiltin(native_add::INST))),
	);

	let v = s.evaluate_snippet(
		"snip".to_owned(),
		"
            assert nativeAdd(1, 2) == 3;
            assert nativeAdd(100, 200) == 300;
            null
        ",
	)?;
	ensure_val_eq!(s, v, Val::Null);
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
	s.add_global(
		"curryAdd".into(),
		Thunk::evaluated(Val::Func(FuncVal::StaticBuiltin(curry_add::INST))),
	);

	let v = s.evaluate_snippet(
		"snip".to_owned(),
		"
            local a = curryAdd(1);
            local b = curryAdd(4);

            assert a(2) == 3;
            assert a(200) == 201;

            assert b(2) == 6;
            assert b(200) == 204;
            null
        ",
	)?;
	ensure_val_eq!(s, v, Val::Null);
	Ok(())
}
