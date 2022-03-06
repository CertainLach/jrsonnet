use crate::function::StaticBuiltin;
use crate::typed::{Any, PositiveF64, VecVal, M1};
use crate::{
	builtin::manifest::{manifest_yaml_ex, ManifestYamlOptions},
	equals,
	error::{Error::*, Result},
	operator::evaluate_mod_op,
	primitive_equals, push_frame, throw,
	typed::{Either2, Either4},
	with_state, ArrValue, FuncVal, IndexableVal, Val,
};
use crate::{Either, ObjValue};
use format::{format_arr, format_obj};
use jrsonnet_interner::IStr;
use jrsonnet_parser::ExprLocation;
use serde::Deserialize;
use serde_yaml::DeserializingQuirks;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};

pub mod stdlib;
pub use stdlib::*;

use self::manifest::{escape_string_json, manifest_json_ex, ManifestJsonOptions, ManifestType};

pub mod format;
pub mod manifest;
pub mod sort;

pub fn std_format(str: IStr, vals: Val) -> Result<String> {
	push_frame(
		None,
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
			("id".into(), builtin_id::INST),
			("strReplace".into(), builtin_str_replace::INST),
			("splitLimit".into(), builtin_splitlimit::INST),
			("parseJson".into(), builtin_parse_json::INST),
			("parseYaml".into(), builtin_parse_yaml::INST),
			("asciiUpper".into(), builtin_ascii_upper::INST),
			("asciiLower".into(), builtin_ascii_lower::INST),
			("member".into(), builtin_member::INST),
			("count".into(), builtin_count::INST),
		].iter().cloned().collect()
	};
}

#[jrsonnet_macros::builtin]
fn builtin_length(x: Either![IStr, VecVal, ObjValue, FuncVal]) -> Result<usize> {
	use Either4::*;
	Ok(match x {
		A(x) => x.chars().count(),
		B(x) => x.0.len(),
		C(x) => x
			.fields_visibility()
			.into_iter()
			.filter(|(_k, v)| *v)
			.count(),
		D(f) => f.args_len(),
	})
}

#[jrsonnet_macros::builtin]
fn builtin_type(x: Any) -> Result<IStr> {
	Ok(x.0.value_type().name().into())
}

#[jrsonnet_macros::builtin]
fn builtin_make_array(sz: usize, func: FuncVal) -> Result<VecVal> {
	let mut out = Vec::with_capacity(sz);
	for i in 0..sz {
		out.push(func.evaluate_simple(&[i as f64].as_slice())?)
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
	index: Option<usize>,
	end: Option<usize>,
	step: Option<usize>,
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
fn builtin_equals(a: Any, b: Any) -> Result<bool> {
	equals(&a.0, &b.0)
}

#[jrsonnet_macros::builtin]
fn builtin_modulo(a: f64, b: f64) -> Result<f64> {
	Ok(a % b)
}

#[jrsonnet_macros::builtin]
fn builtin_mod(a: Either![f64, IStr], b: Any) -> Result<Any> {
	use Either2::*;
	Ok(Any(evaluate_mod_op(
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
fn builtin_ext_var(x: IStr) -> Result<Any> {
	Ok(Any(with_state(|s| s.settings().ext_vars.get(&x).cloned())
		.ok_or(UndefinedExternalVariable(x))?))
}

#[jrsonnet_macros::builtin]
fn builtin_native(name: IStr) -> Result<FuncVal> {
	Ok(with_state(|s| s.settings().ext_natives.get(&name).cloned())
		.map(|v| FuncVal::Builtin(v.clone()))
		.ok_or(UndefinedExternalFunction(name))?)
}

#[jrsonnet_macros::builtin]
fn builtin_filter(func: FuncVal, arr: ArrValue) -> Result<ArrValue> {
	arr.filter(|val| bool::try_from(func.evaluate_simple(&[Any(val.clone())].as_slice())?))
}

#[jrsonnet_macros::builtin]
fn builtin_map(func: FuncVal, arr: ArrValue) -> Result<ArrValue> {
	arr.map(|val| func.evaluate_simple(&[Any(val)].as_slice()))
}

#[jrsonnet_macros::builtin]
fn builtin_flatmap(func: FuncVal, arr: IndexableVal) -> Result<IndexableVal> {
	match arr {
		IndexableVal::Str(s) => {
			let mut out = String::new();
			for c in s.chars() {
				match func.evaluate_simple(&[c.to_string()].as_slice())? {
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
				match func.evaluate_simple(&[Any(el)].as_slice())? {
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
fn builtin_foldl(func: FuncVal, arr: ArrValue, init: Any) -> Result<Any> {
	let mut acc = init.0;
	for i in arr.iter() {
		acc = func.evaluate_simple(&[Any(acc), Any(i?)].as_slice())?;
	}
	Ok(Any(acc))
}

#[jrsonnet_macros::builtin]
fn builtin_foldr(func: FuncVal, arr: ArrValue, init: Any) -> Result<Any> {
	let mut acc = init.0;
	for i in arr.iter().rev() {
		acc = func.evaluate_simple(&[Any(i?), Any(acc)].as_slice())?;
	}
	Ok(Any(acc))
}

#[jrsonnet_macros::builtin]
#[allow(non_snake_case)]
fn builtin_sort(arr: ArrValue, keyF: Option<FuncVal>) -> Result<ArrValue> {
	if arr.len() <= 1 {
		return Ok(arr);
	}
	Ok(ArrValue::Eager(sort::sort(
		arr.evaluated()?,
		keyF.as_ref(),
	)?))
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
	Ok(std::char::from_u32(n as u32).ok_or(InvalidUnicodeCodepointGot(n as u32))?)
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
fn builtin_trace(#[location] loc: Option<&ExprLocation>, str: IStr, rest: Any) -> Result<Any> {
	eprint!("TRACE:");
	if let Some(loc) = loc {
		with_state(|s| {
			let locs = s.map_source_locations(&loc.0, &[loc.1]);
			eprint!(
				" {}:{}",
				loc.0.file_name().unwrap().to_str().unwrap(),
				locs[0].line
			);
		});
	}
	eprintln!(" {}", str);
	Ok(rest) as Result<Any>
}

#[jrsonnet_macros::builtin]
fn builtin_base64(input: Either![Vec<u8>, IStr]) -> Result<String> {
	use Either2::*;
	Ok(match input {
		A(a) => base64::encode(a),
		B(l) => base64::encode(l.bytes().collect::<Vec<_>>()),
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
fn builtin_manifest_json_ex(
	value: Any,
	indent: IStr,
	newline: Option<IStr>,
	key_val_sep: Option<IStr>,
) -> Result<String> {
	let newline = newline.as_deref().unwrap_or("\n");
	let key_val_sep = key_val_sep.as_deref().unwrap_or(": ");
	manifest_json_ex(
		&value.0,
		&ManifestJsonOptions {
			padding: &indent,
			mtype: ManifestType::Std,
			newline,
			key_val_sep,
		},
	)
}

#[jrsonnet_macros::builtin]
fn builtin_manifest_yaml_doc(
	value: Any,
	indent_array_in_object: Option<bool>,
	quote_keys: Option<bool>,
) -> Result<String> {
	manifest_yaml_ex(
		&value.0,
		&ManifestYamlOptions {
			padding: "  ",
			arr_element_padding: if indent_array_in_object.unwrap_or(false) {
				"  "
			} else {
				""
			},
			quote_keys: quote_keys.unwrap_or(true),
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
fn builtin_splitlimit(str: IStr, c: char, maxsplits: Either![usize, M1]) -> Result<VecVal> {
	use Either2::*;
	Ok(VecVal(match maxsplits {
		A(n) => str.splitn(n + 1, c).map(|s| Val::Str(s.into())).collect(),
		B(_) => str.split(c).map(|s| Val::Str(s.into())).collect(),
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
