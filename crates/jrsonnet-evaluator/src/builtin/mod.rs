use crate::typed::{Any, Either, Null, PositiveF64, VecVal, M1};
use crate::{self as jrsonnet_evaluator, ObjValue};
use crate::{
	builtin::manifest::{manifest_yaml_ex, ManifestYamlOptions},
	equals,
	error::{Error::*, Result},
	operator::evaluate_mod_op,
	primitive_equals, push_frame, throw, with_state, ArrValue, Context, FuncVal,
	IndexableVal, Val,
};
use format::{format_arr, format_obj};
use gcmodule::Cc;
use jrsonnet_interner::IStr;
use jrsonnet_parser::{ArgsDesc, ExprLocation};
use serde::Deserialize;
use serde_yaml::DeserializingQuirks;
use std::{
	collections::HashMap,
	convert::{TryFrom, TryInto},
	path::PathBuf,
	rc::Rc,
};

pub mod stdlib;
pub use stdlib::*;

use self::manifest::{escape_string_json, manifest_json_ex, ManifestJsonOptions, ManifestType};

pub mod format;
pub mod manifest;
pub mod sort;

pub fn std_format(str: IStr, vals: Val) -> Result<String> {
	push_frame(
		&ExprLocation(Rc::from(PathBuf::from("std.jsonnet")), 0, 0),
		|| format!("std.format of {}", str),
		|| {
			Ok(match vals {
				Val::Arr(vals) => format_arr(&str, &vals.evaluated()?)?,
				Val::Obj(obj) => format_obj(&str, &obj)?,
				o => format_arr(&str, &[o])?,
			})
		},
	)
}

pub fn std_slice(
	indexable: IndexableVal,
	index: Option<usize>,
	end: Option<usize>,
	step: Option<usize>,
) -> Result<Val> {
	let index = index.unwrap_or(0);
	let end = end.unwrap_or_else(|| match &indexable {
		IndexableVal::Str(_) => usize::MAX,
		IndexableVal::Arr(v) => v.len(),
	});
	let step = step.unwrap_or(1);
	match &indexable {
		IndexableVal::Str(s) => Ok(Val::Str(
			(s.chars()
				.skip(index)
				.take(end - index)
				.step_by(step)
				.collect::<String>())
			.into(),
		)),
		IndexableVal::Arr(arr) => Ok(Val::Arr(
			(arr.iter()
				.skip(index)
				.take(end - index)
				.step_by(step)
				.collect::<Result<Vec<Val>>>()?)
			.into(),
		)),
	}
}

type Builtin = fn(context: Context, loc: &ExprLocation, args: &ArgsDesc) -> Result<Val>;

type BuiltinsType = HashMap<Box<str>, Builtin>;

thread_local! {
	static BUILTINS: BuiltinsType = {
		[
			("length".into(), builtin_length as Builtin),
			("type".into(), builtin_type),
			("makeArray".into(), builtin_make_array),
			("codepoint".into(), builtin_codepoint),
			("objectFieldsEx".into(), builtin_object_fields_ex),
			("objectHasEx".into(), builtin_object_has_ex),
			("slice".into(), builtin_slice),
			("substr".into(), builtin_substr),
			("primitiveEquals".into(), builtin_primitive_equals),
			("equals".into(), builtin_equals),
			("modulo".into(), builtin_modulo),
			("mod".into(), builtin_mod),
			("floor".into(), builtin_floor),
			("ceil".into(), builtin_ceil),
			("log".into(), builtin_log),
			("pow".into(), builtin_pow),
			("sqrt".into(), builtin_sqrt),
			("sin".into(), builtin_sin),
			("cos".into(), builtin_cos),
			("tan".into(), builtin_tan),
			("asin".into(), builtin_asin),
			("acos".into(), builtin_acos),
			("atan".into(), builtin_atan),
			("exp".into(), builtin_exp),
			("mantissa".into(), builtin_mantissa),
			("exponent".into(), builtin_exponent),
			("extVar".into(), builtin_ext_var),
			("native".into(), builtin_native),
			("filter".into(), builtin_filter),
			("map".into(), builtin_map),
			("flatMap".into(), builtin_flatmap),
			("foldl".into(), builtin_foldl),
			("foldr".into(), builtin_foldr),
			("sortImpl".into(), builtin_sort_impl),
			("format".into(), builtin_format),
			("range".into(), builtin_range),
			("char".into(), builtin_char),
			("encodeUTF8".into(), builtin_encode_utf8),
			("decodeUTF8".into(), builtin_decode_utf8),
			("md5".into(), builtin_md5),
			("base64".into(), builtin_base64),
			("base64DecodeBytes".into(), builtin_base64_decode_bytes),
			("base64Decode".into(), builtin_base64_decode),
			("trace".into(), builtin_trace),
			("join".into(), builtin_join),
			("escapeStringJson".into(), builtin_escape_string_json),
			("manifestJsonEx".into(), builtin_manifest_json_ex),
			("manifestYamlDocImpl".into(), builtin_manifest_yaml_doc),
			("reverse".into(), builtin_reverse),
			("id".into(), builtin_id),
			("strReplace".into(), builtin_str_replace),
			("splitLimit".into(), builtin_splitlimit),
			("parseJson".into(), builtin_parse_json),
			("parseYaml".into(), builtin_parse_yaml),
			("asciiUpper".into(), builtin_ascii_upper),
			("asciiLower".into(), builtin_ascii_lower),
			("member".into(), builtin_member),
			("count".into(), builtin_count),
		].iter().cloned().collect()
	};
}

#[jrsonnet_macros::builtin]
fn builtin_length(x: Either<IStr, Either<VecVal, ObjValue>>) -> Result<usize> {
	Ok(match x {
		Either::Left(x) => x.len(),
		Either::Right(Either::Left(x)) => x.0.len(),
		Either::Right(Either::Right(x)) => x
			.fields_visibility()
			.into_iter()
			.filter(|(_k, v)| *v)
			.count(),
	})
}

#[jrsonnet_macros::builtin]
fn builtin_type(x: Any) -> Result<IStr> {
	Ok(x.0.value_type().name().into())
}

#[jrsonnet_macros::builtin]
fn builtin_make_array(sz: usize, func: Cc<FuncVal>) -> Result<VecVal> {
	let mut out = Vec::with_capacity(sz);
	for i in 0..sz {
		out.push(func.evaluate_values(&[Val::Num(i as f64)])?)
	}
	Ok(VecVal(out))
}

#[jrsonnet_macros::builtin]
const fn builtin_codepoint(str: char) -> Result<u32> {
	Ok(str as u32)
}

#[jrsonnet_macros::builtin]
fn builtin_object_fields_ex(obj: ObjValue, inc_hidden: bool) -> Result<VecVal> {
	let out = obj.fields_ex(inc_hidden);
	Ok(VecVal(out.into_iter().map(Val::Str).collect::<Vec<_>>()))
}

#[jrsonnet_macros::builtin]
fn builtin_object_has_ex(obj: ObjValue, f: IStr, inc_hidden: bool) -> Result<bool> {
	Ok(obj.has_field_ex(f, inc_hidden))
}

#[jrsonnet_macros::builtin]
fn builtin_parse_json(s: IStr) -> Result<Any> {
	let value: serde_json::Value = serde_json::from_str(&s)
		.map_err(|e| RuntimeError(format!("failed to parse json: {}", e).into()))?;
	Ok(Any(Val::try_from(&value)?))
}

#[jrsonnet_macros::builtin]
fn builtin_parse_yaml(s: IStr) -> Result<Any> {
	let value = serde_yaml::Deserializer::from_str_with_quirks(
		&s,
		DeserializingQuirks { old_octals: true },
	);
	let mut out = vec![];
	for item in value {
		let value = serde_json::Value::deserialize(item)
			.map_err(|e| RuntimeError(format!("failed to parse yaml: {}", e).into()))?;
		let val = Val::try_from(&value)?;
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
	index: Either<usize, Null>,
	end: Either<usize, Null>,
	step: Either<usize, Null>,
) -> Result<Any> {
	std_slice(indexable, index.left(), end.left(), step.left()).map(Any)
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
fn builtin_equals(a: Any, b: Any) -> Result<bool> {
	equals(&a.0, &b.0)
}

#[jrsonnet_macros::builtin]
fn builtin_modulo(a: f64, b: f64) -> Result<f64> {
	Ok(a % b)
}

#[jrsonnet_macros::builtin]
fn builtin_mod(a: Either<f64, IStr>, b: Any) -> Result<Any> {
	Ok(Any(evaluate_mod_op(
		&match a {
			Either::Left(v) => Val::Num(v),
			Either::Right(s) => Val::Str(s),
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
fn builtin_ext_var(x: IStr) -> Result<Any> {
	Ok(Any(with_state(|s| s.settings().ext_vars.get(&x).cloned())
		.ok_or(UndefinedExternalVariable(x))?))
}

#[jrsonnet_macros::builtin]
fn builtin_native(name: IStr) -> Result<Cc<FuncVal>> {
	Ok(with_state(|s| s.settings().ext_natives.get(&name).cloned())
		.map(|v| Cc::new(FuncVal::NativeExt(name.clone(), v)))
		.ok_or(UndefinedExternalFunction(name))?)
}

#[jrsonnet_macros::builtin]
fn builtin_filter(func: Cc<FuncVal>, arr: ArrValue) -> Result<ArrValue> {
	arr.filter(|val| bool::try_from(func.evaluate_values(&[val.clone()])?))
}

#[jrsonnet_macros::builtin]
fn builtin_map(func: Cc<FuncVal>, arr: ArrValue) -> Result<ArrValue> {
	arr.map(|val| func.evaluate_values(&[val]))
}

#[jrsonnet_macros::builtin]
fn builtin_flatmap(func: Cc<FuncVal>, arr: IndexableVal) -> Result<IndexableVal> {
	match arr {
		IndexableVal::Str(s) => {
			let mut out = String::new();
			for c in s.chars() {
				match func.evaluate_values(&[Val::Str(c.to_string().into())])? {
					Val::Str(o) => out.push_str(&o),
					_ => throw!(RuntimeError(
						"in std.join all items should be strings".into()
					)),
				};
			}
			Ok(IndexableVal::Str(out.into()))
		}
		IndexableVal::Arr(a) => {
			let mut out = Vec::new();
			for el in a.iter() {
				let el = el?;
				match func.evaluate_values(&[el])? {
					Val::Arr(o) => {
						for oe in o.iter() {
							out.push(oe?)
						}
					}
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
fn builtin_foldl(func: Cc<FuncVal>, arr: ArrValue, init: Any) -> Result<Any> {
	let mut acc = init.0;
	for i in arr.iter() {
		acc = func.evaluate_values(&[acc, i?])?;
	}
	Ok(Any(acc))
}

#[jrsonnet_macros::builtin]
fn builtin_foldr(func: Cc<FuncVal>, arr: ArrValue, init: Any) -> Result<Any> {
	let mut acc = init.0;
	for i in arr.iter().rev() {
		acc = func.evaluate_values(&[i?, acc])?;
	}
	Ok(Any(acc))
}

#[jrsonnet_macros::builtin]
#[allow(non_snake_case)]
fn builtin_sort_impl(arr: ArrValue, keyF: Cc<FuncVal>) -> Result<ArrValue> {
	if arr.len() <= 1 {
		return Ok(arr);
	}
	Ok(ArrValue::Eager(sort::sort(arr.evaluated()?, &keyF)?))
}

#[jrsonnet_macros::builtin]
fn builtin_format(str: IStr, vals: Any) -> Result<String> {
	std_format(str, vals.0)
}

#[jrsonnet_macros::builtin]
fn builtin_range(from: i32, to: i32) -> Result<VecVal> {
	if to < from {
		return Ok(VecVal(Vec::new()));
	}
	let mut out = Vec::with_capacity((1 + to as usize - from as usize).max(0));
	for i in from as usize..=to as usize {
		out.push(Val::Num(i as f64));
	}
	Ok(VecVal(out))
}

#[jrsonnet_macros::builtin]
fn builtin_char(n: u32) -> Result<char> {
	Ok(std::char::from_u32(n as u32).ok_or_else(|| InvalidUnicodeCodepointGot(n as u32))?)
}

#[jrsonnet_macros::builtin]
fn builtin_encode_utf8(str: IStr) -> Result<VecVal> {
	Ok(VecVal(
		str.bytes()
			.map(|b| Val::Num(b as f64))
			.collect::<Vec<Val>>(),
	))
}

#[jrsonnet_macros::builtin]
fn builtin_decode_utf8(arr: Vec<u8>) -> Result<String> {
	Ok(String::from_utf8(arr).map_err(|_| RuntimeError("bad utf8".into()))?)
}

#[jrsonnet_macros::builtin]
fn builtin_md5(str: IStr) -> Result<String> {
	Ok(format!("{:x}", md5::compute(&str.as_bytes())))
}

#[jrsonnet_macros::builtin]
fn builtin_trace(#[location] loc: &ExprLocation, str: IStr, rest: Any) -> Result<Any> {
	eprint!("TRACE:");
	with_state(|s| {
		let locs = s.map_source_locations(&loc.0, &[loc.1]);
		eprint!(
			" {}:{}",
			loc.0.file_name().unwrap().to_str().unwrap(),
			locs[0].line
		);
	});
	eprintln!(" {}", str);
	Ok(rest) as Result<Any>
}

#[jrsonnet_macros::builtin]
fn builtin_base64(input: Either<Vec<u8>, IStr>) -> Result<String> {
	Ok(match input {
		Either::Left(a) => base64::encode(a),
		Either::Right(l) => base64::encode(l.bytes().collect::<Vec<_>>()),
	})
}

#[jrsonnet_macros::builtin]
fn builtin_base64_decode_bytes(input: IStr) -> Result<Vec<u8>> {
	Ok(base64::decode(&input.as_bytes()).map_err(|_| RuntimeError("bad base64".into()))?)
}

#[jrsonnet_macros::builtin]
fn builtin_base64_decode(input: IStr) -> Result<String> {
	let bytes = base64::decode(&input.as_bytes()).map_err(|_| RuntimeError("bad base64".into()))?;
	Ok(String::from_utf8(bytes).map_err(|_| RuntimeError("bad utf8".into()))?)
}

#[jrsonnet_macros::builtin]
fn builtin_join(sep: IndexableVal, arr: ArrValue) -> Result<IndexableVal> {
	Ok(match sep {
		IndexableVal::Arr(joiner_items) => {
			let mut out = Vec::new();

			let mut first = true;
			for item in arr.iter() {
				let item = item?.clone();
				if let Val::Arr(items) = item {
					if !first {
						out.reserve(joiner_items.len());
						// TODO: extend
						for item in joiner_items.iter() {
							out.push(item?);
						}
					}
					first = false;
					out.reserve(items.len());
					// TODO: extend
					for item in items.iter() {
						out.push(item?);
					}
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
			for item in arr.iter() {
				let item = item?.clone();
				if let Val::Str(item) = item {
					if !first {
						out += &sep;
					}
					first = false;
					out += &item;
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
fn builtin_manifest_json_ex(value: Any, indent: IStr) -> Result<String> {
	manifest_json_ex(
		&value.0,
		&ManifestJsonOptions {
			padding: &indent,
			mtype: ManifestType::Std,
		},
	)
}

#[jrsonnet_macros::builtin]
fn builtin_manifest_yaml_doc(
	value: Any,
	indent_array_in_object: bool,
	quote_keys: bool,
) -> Result<String> {
	manifest_yaml_ex(
		&value.0,
		&ManifestYamlOptions {
			padding: "  ",
			arr_element_padding: if indent_array_in_object { "  " } else { "" },
			quote_keys,
		},
	)
}

#[jrsonnet_macros::builtin]
fn builtin_reverse(value: ArrValue) -> Result<ArrValue> {
	Ok(value.reversed())
}

#[jrsonnet_macros::builtin]
const fn builtin_id(v: Any) -> Result<Any> {
	Ok(v)
}

#[jrsonnet_macros::builtin]
fn builtin_str_replace(str: String, from: IStr, to: IStr) -> Result<String> {
	Ok(str.replace(&from as &str, &to as &str))
}

#[jrsonnet_macros::builtin]
fn builtin_splitlimit(str: IStr, c: char, maxsplits: Either<usize, M1>) -> Result<VecVal> {
	Ok(VecVal(match maxsplits {
		Either::Left(n) => str.splitn(n + 1, c).map(|s| Val::Str(s.into())).collect(),
		Either::Right(_) => str.split(c).map(|s| Val::Str(s.into())).collect(),
	}))
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
fn builtin_member(arr: IndexableVal, x: Any) -> Result<bool> {
	match arr {
		IndexableVal::Str(s) => {
			let x: IStr = IStr::try_from(x.0)?;
			Ok(!x.is_empty() && s.contains(&*x))
		}
		IndexableVal::Arr(a) => {
			for item in a.iter() {
				let item = item?;
				if equals(&item, &x.0)? {
					return Ok(true);
				}
			}
			Ok(false)
		}
	}
}

#[jrsonnet_macros::builtin]
fn builtin_count(arr: Vec<Any>, v: Any) -> Result<usize> {
	let mut count = 0;
	for item in arr.iter() {
		if equals(&item.0, &v.0)? {
			count += 1;
		}
	}
	Ok(count)
}

pub fn call_builtin(
	context: Context,
	loc: &ExprLocation,
	name: &str,
	args: &ArgsDesc,
) -> Result<Val> {
	BUILTINS
		.with(|builtins| builtins.get(name).copied())
		.ok_or_else(|| IntrinsicNotFound(name.into()))?(context, loc, args)
}
