mod common;

use std::{fmt::Debug, path::PathBuf};

use jrsonnet_evaluator::{error::Result, typed::Typed, State};

#[derive(Clone, Typed, PartialEq, Debug)]
struct A {
	a: u32,
	b: u16,
}

fn test_roundtrip<T: Typed + PartialEq + Debug + Clone>(value: T, s: State) -> Result<()> {
	let untyped = T::into_untyped(value.clone(), s.clone())?;
	let value2 = T::from_untyped(untyped.clone(), s.clone())?;
	ensure_eq!(value, value2);
	let untyped2 = T::into_untyped(value2, s.clone())?;
	ensure_val_eq!(s, untyped, untyped2);

	Ok(())
}

#[test]
fn simple_object() -> Result<()> {
	let s = State::default();
	s.with_stdlib();
	let a = A::from_untyped(
		s.evaluate_snippet_raw(PathBuf::new().into(), "{a: 1, b: 2}".into())?,
		s.clone(),
	)?;
	ensure_eq!(a, A { a: 1, b: 2 });
	test_roundtrip(a.clone(), s.clone())?;
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
	let s = State::default();
	s.with_stdlib();
	let b = B::from_untyped(
		s.evaluate_snippet_raw(PathBuf::new().into(), "{a: 1, c: 2}".into())?,
		s.clone(),
	)?;
	ensure_eq!(b, B { a: 1, b: 2 });
	ensure_eq!(
		&B::into_untyped(b.clone(), s.clone())?.to_string(s.clone())? as &str,
		r#"{"a": 1, "c": 2}"#,
	);
	test_roundtrip(b.clone(), s.clone())?;
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
	let s = State::default();
	s.with_stdlib();
	let obj = Object::from_untyped(
		s.evaluate_snippet_raw(
			PathBuf::new().into(),
			"{apiVersion: 'ver', kind: 'kind', b: 2}".into(),
		)?,
		s.clone(),
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
		&Object::into_untyped(obj.clone(), s.clone())?.to_string(s.clone())? as &str,
		r#"{"apiVersion": "ver", "b": 2, "kind": "kind"}"#,
	);
	test_roundtrip(obj.clone(), s.clone())?;
	Ok(())
}

#[derive(Clone, Typed, PartialEq, Debug)]
struct C {
	a: Option<u32>,
	b: u16,
}

#[test]
fn optional_field_some() -> Result<()> {
	let s = State::default();
	s.with_stdlib();
	let c = C::from_untyped(
		s.evaluate_snippet_raw(PathBuf::new().into(), "{a: 1, b: 2}".into())?,
		s.clone(),
	)?;
	ensure_eq!(c, C { a: Some(1), b: 2 });
	ensure_eq!(
		&C::into_untyped(c.clone(), s.clone())?.to_string(s.clone())? as &str,
		r#"{"a": 1, "b": 2}"#,
	);
	test_roundtrip(c.clone(), s.clone())?;
	Ok(())
}

#[test]
fn optional_field_none() -> Result<()> {
	let s = State::default();
	s.with_stdlib();
	let c = C::from_untyped(
		s.evaluate_snippet_raw(PathBuf::new().into(), "{b: 2}".into())?,
		s.clone(),
	)?;
	ensure_eq!(c, C { a: None, b: 2 });
	ensure_eq!(
		&C::into_untyped(c.clone(), s.clone())?.to_string(s.clone())? as &str,
		r#"{"b": 2}"#,
	);
	test_roundtrip(c.clone(), s.clone())?;
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
	let s = State::default();
	s.with_stdlib();
	let d = D::from_untyped(
		s.evaluate_snippet_raw(PathBuf::new().into(), "{b: 2, v:1}".into())?,
		s.clone(),
	)?;
	ensure_eq!(
		d,
		D {
			e: Some(E { v: 1 }),
			b: 2
		}
	);
	ensure_eq!(
		&D::into_untyped(d.clone(), s.clone())?.to_string(s.clone())? as &str,
		r#"{"b": 2, "v": 1}"#,
	);
	test_roundtrip(d.clone(), s.clone())?;
	Ok(())
}

#[test]
fn flatten_optional_none() -> Result<()> {
	let s = State::default();
	s.with_stdlib();
	let d = D::from_untyped(
		s.evaluate_snippet_raw(PathBuf::new().into(), "{b: 2, v: '1'}".into())?,
		s.clone(),
	)?;
	ensure_eq!(d, D { e: None, b: 2 });
	ensure_eq!(
		&D::into_untyped(d.clone(), s.clone())?.to_string(s.clone())? as &str,
		r#"{"b": 2}"#,
	);
	test_roundtrip(d.clone(), s.clone())?;
	Ok(())
}
