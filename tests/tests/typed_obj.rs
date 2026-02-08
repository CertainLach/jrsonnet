mod common;

use std::fmt::Debug;

use jrsonnet_evaluator::{Result, State, trace::PathResolver, typed::Typed};
use jrsonnet_stdlib::ContextInitializer;

#[derive(Clone, Typed, PartialEq, Debug)]
struct A {
	a: u32,
	b: u16,
}

fn test_roundtrip<T: Typed + PartialEq + Debug + Clone>(value: T) -> Result<()> {
	let untyped = T::into_untyped(value.clone())?;
	let value2 = T::from_untyped(untyped.clone())?;
	ensure_eq!(value, value2);
	let untyped2 = T::into_untyped(value2)?;
	ensure_val_eq!(untyped, untyped2);

	Ok(())
}

#[test]
fn simple_object() -> Result<()> {
	let mut s = State::builder();
	s.context_initializer(ContextInitializer::new(PathResolver::new_cwd_fallback()));
	let s = s.build();

	let a = A::from_untyped(s.evaluate_snippet("snip".to_owned(), "{a: 1, b: 2}")?)?;
	ensure_eq!(a, A { a: 1, b: 2 });
	test_roundtrip(a)?;
	Ok(())
}

#[derive(Clone, Typed, PartialEq, Debug)]
struct B {
	a: u32,
	#[typed(rename = "c")]
	b: u16,
}

#[test]
fn renamed_field() -> Result<()> {
	let mut s = State::builder();
	s.context_initializer(ContextInitializer::new(PathResolver::new_cwd_fallback()));
	let s = s.build();

	let b = B::from_untyped(s.evaluate_snippet("snip".to_owned(), "{a: 1, c: 2}")?)?;
	ensure_eq!(b, B { a: 1, b: 2 });
	ensure_eq!(
		&B::into_untyped(b.clone())?.to_string()? as &str,
		r#"{"a": 1, "c": 2}"#,
	);
	test_roundtrip(b)?;
	Ok(())
}

#[derive(Clone, Typed, PartialEq, Debug)]
struct ObjectKind {
	#[typed(rename = "apiVersion")]
	api_version: String,
	#[typed(rename = "kind")]
	kind: String,
}

#[derive(Clone, Typed, PartialEq, Debug)]
struct Object {
	#[typed(flatten)]
	kind: ObjectKind,
	b: u16,
}

#[test]
fn flattened_object() -> Result<()> {
	let mut s = State::builder();
	s.context_initializer(ContextInitializer::new(PathResolver::new_cwd_fallback()));
	let s = s.build();

	let obj = Object::from_untyped(
		s.evaluate_snippet("snip".to_owned(), "{apiVersion: 'ver', kind: 'kind', b: 2}")?,
	)?;
	ensure_eq!(
		obj,
		Object {
			kind: ObjectKind {
				api_version: "ver".into(),
				kind: "kind".into(),
			},
			b: 2
		}
	);
	ensure_eq!(
		&Object::into_untyped(obj.clone())?.to_string()? as &str,
		r#"{"apiVersion": "ver", "b": 2, "kind": "kind"}"#,
	);
	test_roundtrip(obj)?;
	Ok(())
}

#[derive(Clone, Typed, PartialEq, Debug)]
struct C {
	a: Option<u32>,
	b: u16,
}

#[test]
fn optional_field_some() -> Result<()> {
	let mut s = State::builder();
	s.context_initializer(ContextInitializer::new(PathResolver::new_cwd_fallback()));
	let s = s.build();

	let c = C::from_untyped(s.evaluate_snippet("snip".to_owned(), "{a: 1, b: 2}")?)?;
	ensure_eq!(c, C { a: Some(1), b: 2 });
	ensure_eq!(
		&C::into_untyped(c.clone())?.to_string()? as &str,
		r#"{"a": 1, "b": 2}"#,
	);
	test_roundtrip(c)?;
	Ok(())
}

#[test]
fn optional_field_none() -> Result<()> {
	let mut s = State::builder();
	s.context_initializer(ContextInitializer::new(PathResolver::new_cwd_fallback()));
	let s = s.build();

	let c = C::from_untyped(s.evaluate_snippet("snip".to_owned(), "{b: 2}")?)?;
	ensure_eq!(c, C { a: None, b: 2 });
	ensure_eq!(
		&C::into_untyped(c.clone())?.to_string()? as &str,
		r#"{"b": 2}"#,
	);
	test_roundtrip(c)?;
	Ok(())
}

#[derive(Clone, Typed, PartialEq, Debug)]
struct D {
	#[typed(flatten(ok))]
	e: Option<E>,
	b: u16,
}

#[derive(Clone, Typed, PartialEq, Debug)]
struct E {
	v: u32,
}

#[test]
fn flatten_optional_some() -> Result<()> {
	let mut s = State::builder();
	s.context_initializer(ContextInitializer::new(PathResolver::new_cwd_fallback()));
	let s = s.build();

	let d = D::from_untyped(s.evaluate_snippet("snip".to_owned(), "{b: 2, v:1}")?)?;
	ensure_eq!(
		d,
		D {
			e: Some(E { v: 1 }),
			b: 2
		}
	);
	ensure_eq!(
		&D::into_untyped(d.clone())?.to_string()? as &str,
		r#"{"b": 2, "v": 1}"#,
	);
	test_roundtrip(d)?;
	Ok(())
}

#[test]
fn flatten_optional_none() -> Result<()> {
	let mut s = State::builder();
	s.context_initializer(ContextInitializer::new(PathResolver::new_cwd_fallback()));
	let s = s.build();

	let d = D::from_untyped(s.evaluate_snippet("snip".to_owned(), "{b: 2, v: '1'}")?)?;
	ensure_eq!(d, D { e: None, b: 2 });
	ensure_eq!(
		&D::into_untyped(d.clone())?.to_string()? as &str,
		r#"{"b": 2}"#,
	);
	test_roundtrip(d)?;
	Ok(())
}
