// All builtins should return results
#![allow(clippy::unnecessary_wraps)]

use std::collections::HashMap;

use format::{format_arr, format_obj};
use gcmodule::Cc;
use jrsonnet_interner::{IBytes, IStr};
use serde::Deserialize;
use serde_yaml::DeserializingQuirks;

use crate::{
	error::{Error::*, Result},
	function::{builtin::StaticBuiltin, ArgLike, CallLocation, FuncVal},
	operator::evaluate_mod_op,
	stdlib::manifest::{manifest_yaml_ex, ManifestYamlOptions},
	throw,
	typed::{Any, BoundedUsize, Either2, Either4, PositiveF64, Typed, VecVal, M1},
	val::{equals, primitive_equals, ArrValue, IndexableVal, Slice},
	Either, ObjValue, State, Val,
};

pub mod expr;
pub use expr::*;

use self::manifest::{escape_string_json, manifest_json_ex, ManifestJsonOptions, ManifestType};

pub mod format;
pub mod manifest;
pub mod sort;

pub fn std_format(s: State, str: IStr, vals: Val) -> Result<String> {
	s.push(
		CallLocation::native(),
		|| format!("std.format of {}", str),
		|| {
			Ok(match vals {
				Val::Arr(vals) => format_arr(s.clone(), &str, &vals.evaluated(s.clone())?)?,
				Val::Obj(obj) => format_obj(s.clone(), &str, &obj)?,
				o => format_arr(s.clone(), &str, &[o])?,
			})
		},
	)
}

pub fn std_slice(
	indexable: IndexableVal,
	index: Option<BoundedUsize<0, { i32::MAX as usize }>>,
	end: Option<BoundedUsize<0, { i32::MAX as usize }>>,
	step: Option<BoundedUsize<1, { i32::MAX as usize }>>,
) -> Result<Val> {
	match &indexable {
		IndexableVal::Str(s) => {
			let index = index.as_deref().copied().unwrap_or(0);
			let end = end.as_deref().copied().unwrap_or(usize::MAX);
			let step = step.as_deref().copied().unwrap_or(1);

			if index >= end {
				return Ok(Val::Str("".into()));
			}

			Ok(Val::Str(
				(s.chars()
					.skip(index)
					.take(end - index)
					.step_by(step)
					.collect::<String>())
				.into(),
			))
		}
		IndexableVal::Arr(arr) => {
			let index = index.as_deref().copied().unwrap_or(0);
			let end = end.as_deref().copied().unwrap_or(usize::MAX).min(arr.len());
			let step = step.as_deref().copied().unwrap_or(1);

			if index >= end {
				return Ok(Val::Arr(ArrValue::new_eager()));
			}

			Ok(Val::Arr(ArrValue::Slice(Box::new(Slice {
				inner: arr.clone(),
				from: index as u32,
				to: end as u32,
				step: step as u32,
			}))))
		}
	}
}

type BuiltinsType = HashMap<IStr, &'static dyn StaticBuiltin>;

thread_local! {
	pub static BUILTINS: BuiltinsType = {
		[
			("length".into(), builtin_length::INST),
			("type".into(), builtin_type::INST),
			("makeArray".into(), builtin_make_array::INST),
			("codepoint".into(), builtin_codepoint::INST),
			("objectFieldsEx".into(), builtin_object_fields_ex::INST),
			("objectHasEx".into(), builtin_object_has_ex::INST),
			("slice".into(), builtin_slice::INST),
			("substr".into(), builtin_substr::INST),
			("primitiveEquals".into(), builtin_primitive_equals::INST),
			("equals".into(), builtin_equals::INST),
			("modulo".into(), builtin_modulo::INST),
			("mod".into(), builtin_mod::INST),
			("floor".into(), builtin_floor::INST),
			("ceil".into(), builtin_ceil::INST),
			("log".into(), builtin_log::INST),
			("pow".into(), builtin_pow::INST),
			("sqrt".into(), builtin_sqrt::INST),
			("sin".into(), builtin_sin::INST),
			("cos".into(), builtin_cos::INST),
			("tan".into(), builtin_tan::INST),
			("asin".into(), builtin_asin::INST),
			("acos".into(), builtin_acos::INST),
			("atan".into(), builtin_atan::INST),
			("exp".into(), builtin_exp::INST),
			("mantissa".into(), builtin_mantissa::INST),
			("exponent".into(), builtin_exponent::INST),
			("extVar".into(), builtin_ext_var::INST),
			("native".into(), builtin_native::INST),
			("filter".into(), builtin_filter::INST),
			("map".into(), builtin_map::INST),
			("flatMap".into(), builtin_flatmap::INST),
			("foldl".into(), builtin_foldl::INST),
			("foldr".into(), builtin_foldr::INST),
			("sort".into(), builtin_sort::INST),
			("format".into(), builtin_format::INST),
			("range".into(), builtin_range::INST),
			("char".into(), builtin_char::INST),
			("encodeUTF8".into(), builtin_encode_utf8::INST),
			("decodeUTF8".into(), builtin_decode_utf8::INST),
			("md5".into(), builtin_md5::INST),
			("base64".into(), builtin_base64::INST),
			("base64DecodeBytes".into(), builtin_base64_decode_bytes::INST),
			("base64Decode".into(), builtin_base64_decode::INST),
			("trace".into(), builtin_trace::INST),
			("join".into(), builtin_join::INST),
			("escapeStringJson".into(), builtin_escape_string_json::INST),
			("manifestJsonEx".into(), builtin_manifest_json_ex::INST),
			("manifestYamlDoc".into(), builtin_manifest_yaml_doc::INST),
			("reverse".into(), builtin_reverse::INST),
			("strReplace".into(), builtin_str_replace::INST),
			("splitLimit".into(), builtin_splitlimit::INST),
			("parseJson".into(), builtin_parse_json::INST),
			("parseYaml".into(), builtin_parse_yaml::INST),
			("asciiUpper".into(), builtin_ascii_upper::INST),
			("asciiLower".into(), builtin_ascii_lower::INST),
			("member".into(), builtin_member::INST),
			("count".into(), builtin_count::INST),
			("any".into(), builtin_any::INST),
			("all".into(), builtin_all::INST),
		].iter().cloned().collect()
	};
}

#[jrsonnet_macros::builtin]
fn builtin_length(x: Either![IStr, ArrValue, ObjValue, FuncVal]) -> Result<usize> {
	use Either4::*;
	Ok(match x {
		A(x) => x.chars().count(),
		B(x) => x.len(),
		C(x) => x.len(),
		D(f) => f.params_len(),
	})
}

#[jrsonnet_macros::builtin]
fn builtin_type(x: Any) -> Result<IStr> {
	Ok(x.0.value_type().name().into())
}

#[jrsonnet_macros::builtin]
fn builtin_make_array(s: State, sz: usize, func: FuncVal) -> Result<VecVal> {
	let mut out = Vec::with_capacity(sz);
	for i in 0..sz {
		out.push(func.evaluate_simple(s.clone(), &(i as f64,))?);
	}
	Ok(VecVal(Cc::new(out)))
}

#[jrsonnet_macros::builtin]
const fn builtin_codepoint(str: char) -> Result<u32> {
	Ok(str as u32)
}

#[jrsonnet_macros::builtin]
fn builtin_object_fields_ex(
	obj: ObjValue,
	inc_hidden: bool,
	#[cfg(feature = "exp-preserve-order")] preserve_order: Option<bool>,
) -> Result<VecVal> {
	#[cfg(feature = "exp-preserve-order")]
	let preserve_order = preserve_order.unwrap_or(false);
	let out = obj.fields_ex(
		inc_hidden,
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	);
	Ok(VecVal(Cc::new(
		out.into_iter().map(Val::Str).collect::<Vec<_>>(),
	)))
}

#[jrsonnet_macros::builtin]
fn builtin_object_has_ex(obj: ObjValue, f: IStr, inc_hidden: bool) -> Result<bool> {
	Ok(obj.has_field_ex(f, inc_hidden))
}

#[jrsonnet_macros::builtin]
fn builtin_parse_json(st: State, s: IStr) -> Result<Any> {
	use serde_json::Value;
	let value: Value = serde_json::from_str(&s)
		.map_err(|e| RuntimeError(format!("failed to parse json: {}", e).into()))?;
	Ok(Any(Value::into_untyped(value, st)?))
}

#[jrsonnet_macros::builtin]
fn builtin_parse_yaml(st: State, s: IStr) -> Result<Any> {
	use serde_json::Value;
	let value = serde_yaml::Deserializer::from_str_with_quirks(
		&s,
		DeserializingQuirks { old_octals: true },
	);
	let mut out = vec![];
	for item in value {
		let value = Value::deserialize(item)
			.map_err(|e| RuntimeError(format!("failed to parse yaml: {}", e).into()))?;
		let val = Value::into_untyped(value, st.clone())?;
		out.push(val);
	}
	Ok(Any(if out.is_empty() {
		Val::Null
	} else if out.len() == 1 {
		out.into_iter().next().unwrap()
	} else {
		Val::Arr(out.into())
	}))
}

#[jrsonnet_macros::builtin]
fn builtin_slice(
	indexable: IndexableVal,
	index: Option<BoundedUsize<0, { i32::MAX as usize }>>,
	end: Option<BoundedUsize<0, { i32::MAX as usize }>>,
	step: Option<BoundedUsize<1, { i32::MAX as usize }>>,
) -> Result<Any> {
	std_slice(indexable, index, end, step).map(Any)
}

#[jrsonnet_macros::builtin]
fn builtin_substr(str: IStr, from: usize, len: usize) -> Result<String> {
	Ok(str.chars().skip(from as usize).take(len as usize).collect())
}

#[jrsonnet_macros::builtin]
fn builtin_primitive_equals(a: Any, b: Any) -> Result<bool> {
	primitive_equals(&a.0, &b.0)
}

#[jrsonnet_macros::builtin]
fn builtin_equals(s: State, a: Any, b: Any) -> Result<bool> {
	equals(s, &a.0, &b.0)
}

#[jrsonnet_macros::builtin]
fn builtin_modulo(a: f64, b: f64) -> Result<f64> {
	Ok(a % b)
}

#[jrsonnet_macros::builtin]
fn builtin_mod(s: State, a: Either![f64, IStr], b: Any) -> Result<Any> {
	use Either2::*;
	Ok(Any(evaluate_mod_op(
		s,
		&match a {
			A(v) => Val::Num(v),
			B(s) => Val::Str(s),
		},
		&b.0,
	)?))
}

#[jrsonnet_macros::builtin]
fn builtin_floor(x: f64) -> Result<f64> {
	Ok(x.floor())
}

#[jrsonnet_macros::builtin]
fn builtin_ceil(x: f64) -> Result<f64> {
	Ok(x.ceil())
}

#[jrsonnet_macros::builtin]
fn builtin_log(n: f64) -> Result<f64> {
	Ok(n.ln())
}

#[jrsonnet_macros::builtin]
fn builtin_pow(x: f64, n: f64) -> Result<f64> {
	Ok(x.powf(n))
}

#[jrsonnet_macros::builtin]
fn builtin_sqrt(x: PositiveF64) -> Result<f64> {
	Ok(x.0.sqrt())
}

#[jrsonnet_macros::builtin]
fn builtin_sin(x: f64) -> Result<f64> {
	Ok(x.sin())
}

#[jrsonnet_macros::builtin]
fn builtin_cos(x: f64) -> Result<f64> {
	Ok(x.cos())
}

#[jrsonnet_macros::builtin]
fn builtin_tan(x: f64) -> Result<f64> {
	Ok(x.tan())
}

#[jrsonnet_macros::builtin]
fn builtin_asin(x: f64) -> Result<f64> {
	Ok(x.asin())
}

#[jrsonnet_macros::builtin]
fn builtin_acos(x: f64) -> Result<f64> {
	Ok(x.acos())
}

#[jrsonnet_macros::builtin]
fn builtin_atan(x: f64) -> Result<f64> {
	Ok(x.atan())
}

#[jrsonnet_macros::builtin]
fn builtin_exp(x: f64) -> Result<f64> {
	Ok(x.exp())
}

fn frexp(s: f64) -> (f64, i16) {
	if 0.0 == s {
		(s, 0)
	} else {
		let lg = s.abs().log2();
		let x = (lg - lg.floor() - 1.0).exp2();
		let exp = lg.floor() + 1.0;
		(s.signum() * x, exp as i16)
	}
}

#[jrsonnet_macros::builtin]
fn builtin_mantissa(x: f64) -> Result<f64> {
	Ok(frexp(x).0)
}

#[jrsonnet_macros::builtin]
fn builtin_exponent(x: f64) -> Result<i16> {
	Ok(frexp(x).1)
}

#[jrsonnet_macros::builtin]
fn builtin_ext_var(s: State, x: IStr) -> Result<Any> {
	let ctx = s.create_default_context();
	Ok(Any(s
		.clone()
		.settings()
		.ext_vars
		.get(&x)
		.cloned()
		.ok_or(UndefinedExternalVariable(x))?
		.evaluate_arg(s.clone(), ctx, true)?
		.evaluate(s)?))
}

#[jrsonnet_macros::builtin]
fn builtin_native(s: State, name: IStr) -> Result<Any> {
	Ok(Any(s
		.settings()
		.ext_natives
		.get(&name)
		.cloned()
		.map_or(Val::Null, |v| {
			Val::Func(FuncVal::Builtin(v.clone()))
		})))
}

#[jrsonnet_macros::builtin]
fn builtin_filter(s: State, func: FuncVal, arr: ArrValue) -> Result<ArrValue> {
	arr.filter(s.clone(), |val| {
		bool::from_untyped(
			func.evaluate_simple(s.clone(), &(Any(val.clone()),))?,
			s.clone(),
		)
	})
}

#[jrsonnet_macros::builtin]
fn builtin_map(s: State, func: FuncVal, arr: ArrValue) -> Result<ArrValue> {
	arr.map(s.clone(), |val| {
		func.evaluate_simple(s.clone(), &(Any(val),))
	})
}

#[jrsonnet_macros::builtin]
fn builtin_flatmap(s: State, func: FuncVal, arr: IndexableVal) -> Result<IndexableVal> {
	match arr {
		IndexableVal::Str(str) => {
			let mut out = String::new();
			for c in str.chars() {
				match func.evaluate_simple(s.clone(), &(c.to_string(),))? {
					Val::Str(o) => out.push_str(&o),
					Val::Null => continue,
					_ => throw!(RuntimeError(
						"in std.join all items should be strings".into()
					)),
				};
			}
			Ok(IndexableVal::Str(out.into()))
		}
		IndexableVal::Arr(a) => {
			let mut out = Vec::new();
			for el in a.iter(s.clone()) {
				let el = el?;
				match func.evaluate_simple(s.clone(), &(Any(el),))? {
					Val::Arr(o) => {
						for oe in o.iter(s.clone()) {
							out.push(oe?);
						}
					}
					Val::Null => continue,
					_ => throw!(RuntimeError(
						"in std.join all items should be arrays".into()
					)),
				};
			}
			Ok(IndexableVal::Arr(out.into()))
		}
	}
}

#[jrsonnet_macros::builtin]
fn builtin_foldl(s: State, func: FuncVal, arr: ArrValue, init: Any) -> Result<Any> {
	let mut acc = init.0;
	for i in arr.iter(s.clone()) {
		acc = func.evaluate_simple(s.clone(), &(Any(acc), Any(i?)))?;
	}
	Ok(Any(acc))
}

#[jrsonnet_macros::builtin]
fn builtin_foldr(s: State, func: FuncVal, arr: ArrValue, init: Any) -> Result<Any> {
	let mut acc = init.0;
	for i in arr.iter(s.clone()).rev() {
		acc = func.evaluate_simple(s.clone(), &(Any(i?), Any(acc)))?;
	}
	Ok(Any(acc))
}

#[jrsonnet_macros::builtin]
#[allow(non_snake_case)]
fn builtin_sort(s: State, arr: ArrValue, keyF: Option<FuncVal>) -> Result<ArrValue> {
	if arr.len() <= 1 {
		return Ok(arr);
	}
	Ok(ArrValue::Eager(sort::sort(
		s.clone(),
		arr.evaluated(s)?,
		keyF.unwrap_or_else(FuncVal::identity),
	)?))
}

#[jrsonnet_macros::builtin]
fn builtin_format(s: State, str: IStr, vals: Any) -> Result<String> {
	std_format(s, str, vals.0)
}

#[jrsonnet_macros::builtin]
fn builtin_range(from: i32, to: i32) -> Result<ArrValue> {
	if to < from {
		return Ok(ArrValue::new_eager());
	}
	Ok(ArrValue::new_range(from, to))
}

#[jrsonnet_macros::builtin]
fn builtin_char(n: u32) -> Result<char> {
	Ok(std::char::from_u32(n as u32).ok_or(InvalidUnicodeCodepointGot(n as u32))?)
}

#[jrsonnet_macros::builtin]
fn builtin_encode_utf8(str: IStr) -> Result<IBytes> {
	Ok(str.cast_bytes())
}

#[jrsonnet_macros::builtin]
fn builtin_decode_utf8(arr: IBytes) -> Result<IStr> {
	Ok(arr
		.cast_str()
		.ok_or_else(|| RuntimeError("bad utf8".into()))?)
}

#[jrsonnet_macros::builtin]
fn builtin_md5(str: IStr) -> Result<String> {
	Ok(format!("{:x}", md5::compute(&str.as_bytes())))
}

#[jrsonnet_macros::builtin]
fn builtin_trace(s: State, loc: CallLocation, str: IStr, rest: Any) -> Result<Any> {
	eprint!("TRACE:");
	if let Some(loc) = loc.0 {
		let locs = s.map_source_locations(loc.0.clone(), &[loc.1]);
		eprint!(" {}:{}", loc.0.short_display(), locs[0].line);
	}
	eprintln!(" {}", str);
	Ok(rest) as Result<Any>
}

#[jrsonnet_macros::builtin]
fn builtin_base64(input: Either![IBytes, IStr]) -> Result<String> {
	use Either2::*;
	Ok(match input {
		A(a) => base64::encode(a.as_slice()),
		B(l) => base64::encode(l.bytes().collect::<Vec<_>>()),
	})
}

#[jrsonnet_macros::builtin]
fn builtin_base64_decode_bytes(input: IStr) -> Result<IBytes> {
	Ok(base64::decode(&input.as_bytes())
		.map_err(|_| RuntimeError("bad base64".into()))?
		.as_slice()
		.into())
}

#[jrsonnet_macros::builtin]
fn builtin_base64_decode(input: IStr) -> Result<String> {
	let bytes = base64::decode(&input.as_bytes()).map_err(|_| RuntimeError("bad base64".into()))?;
	Ok(String::from_utf8(bytes).map_err(|_| RuntimeError("bad utf8".into()))?)
}

#[jrsonnet_macros::builtin]
fn builtin_join(s: State, sep: IndexableVal, arr: ArrValue) -> Result<IndexableVal> {
	Ok(match sep {
		IndexableVal::Arr(joiner_items) => {
			let mut out = Vec::new();

			let mut first = true;
			for item in arr.iter(s.clone()) {
				let item = item?.clone();
				if let Val::Arr(items) = item {
					if !first {
						out.reserve(joiner_items.len());
						// TODO: extend
						for item in joiner_items.iter(s.clone()) {
							out.push(item?);
						}
					}
					first = false;
					out.reserve(items.len());
					for item in items.iter(s.clone()) {
						out.push(item?);
					}
				} else if matches!(item, Val::Null) {
					continue;
				} else {
					throw!(RuntimeError(
						"in std.join all items should be arrays".into()
					));
				}
			}

			IndexableVal::Arr(out.into())
		}
		IndexableVal::Str(sep) => {
			let mut out = String::new();

			let mut first = true;
			for item in arr.iter(s) {
				let item = item?.clone();
				if let Val::Str(item) = item {
					if !first {
						out += &sep;
					}
					first = false;
					out += &item;
				} else if matches!(item, Val::Null) {
					continue;
				} else {
					throw!(RuntimeError(
						"in std.join all items should be strings".into()
					));
				}
			}

			IndexableVal::Str(out.into())
		}
	})
}

#[jrsonnet_macros::builtin]
fn builtin_escape_string_json(str_: IStr) -> Result<String> {
	Ok(escape_string_json(&str_))
}

#[jrsonnet_macros::builtin]
fn builtin_manifest_json_ex(
	s: State,
	value: Any,
	indent: IStr,
	newline: Option<IStr>,
	key_val_sep: Option<IStr>,
	#[cfg(feature = "exp-preserve-order")] preserve_order: Option<bool>,
) -> Result<String> {
	let newline = newline.as_deref().unwrap_or("\n");
	let key_val_sep = key_val_sep.as_deref().unwrap_or(": ");
	manifest_json_ex(
		s,
		&value.0,
		&ManifestJsonOptions {
			padding: &indent,
			mtype: ManifestType::Std,
			newline,
			key_val_sep,
			#[cfg(feature = "exp-preserve-order")]
			preserve_order: preserve_order.unwrap_or(false),
		},
	)
}

#[jrsonnet_macros::builtin]
fn builtin_manifest_yaml_doc(
	s: State,
	value: Any,
	indent_array_in_object: Option<bool>,
	quote_keys: Option<bool>,
	#[cfg(feature = "exp-preserve-order")] preserve_order: Option<bool>,
) -> Result<String> {
	manifest_yaml_ex(
		s,
		&value.0,
		&ManifestYamlOptions {
			padding: "  ",
			arr_element_padding: if indent_array_in_object.unwrap_or(false) {
				"  "
			} else {
				""
			},
			quote_keys: quote_keys.unwrap_or(true),
			#[cfg(feature = "exp-preserve-order")]
			preserve_order: preserve_order.unwrap_or(false),
		},
	)
}

#[jrsonnet_macros::builtin]
fn builtin_reverse(value: ArrValue) -> Result<ArrValue> {
	Ok(value.reversed())
}

#[jrsonnet_macros::builtin]
fn builtin_str_replace(str: String, from: IStr, to: IStr) -> Result<String> {
	Ok(str.replace(&from as &str, &to as &str))
}

#[jrsonnet_macros::builtin]
fn builtin_splitlimit(str: IStr, c: IStr, maxsplits: Either![usize, M1]) -> Result<VecVal> {
	use Either2::*;
	Ok(VecVal(Cc::new(match maxsplits {
		A(n) => str
			.splitn(n + 1, &c as &str)
			.map(|s| Val::Str(s.into()))
			.collect(),
		B(_) => str.split(&c as &str).map(|s| Val::Str(s.into())).collect(),
	})))
}

#[jrsonnet_macros::builtin]
fn builtin_ascii_upper(str: IStr) -> Result<String> {
	Ok(str.to_ascii_uppercase())
}

#[jrsonnet_macros::builtin]
fn builtin_ascii_lower(str: IStr) -> Result<String> {
	Ok(str.to_ascii_lowercase())
}

#[jrsonnet_macros::builtin]
fn builtin_member(s: State, arr: IndexableVal, x: Any) -> Result<bool> {
	match arr {
		IndexableVal::Str(str) => {
			let x: IStr = IStr::from_untyped(x.0, s)?;
			Ok(!x.is_empty() && str.contains(&*x))
		}
		IndexableVal::Arr(a) => {
			for item in a.iter(s.clone()) {
				let item = item?;
				if equals(s.clone(), &item, &x.0)? {
					return Ok(true);
				}
			}
			Ok(false)
		}
	}
}

#[jrsonnet_macros::builtin]
fn builtin_count(s: State, arr: Vec<Any>, v: Any) -> Result<usize> {
	let mut count = 0;
	for item in &arr {
		if equals(s.clone(), &item.0, &v.0)? {
			count += 1;
		}
	}
	Ok(count)
}

#[jrsonnet_macros::builtin]
fn builtin_any(s: State, arr: ArrValue) -> Result<bool> {
	for v in arr.iter(s.clone()) {
		let v = bool::from_untyped(v?, s.clone())?;
		if v {
			return Ok(true);
		}
	}
	Ok(false)
}

#[jrsonnet_macros::builtin]
fn builtin_all(s: State, arr: ArrValue) -> Result<bool> {
	for v in arr.iter(s.clone()) {
		let v = bool::from_untyped(v?, s.clone())?;
		if !v {
			return Ok(false);
		}
	}
	Ok(true)
}
