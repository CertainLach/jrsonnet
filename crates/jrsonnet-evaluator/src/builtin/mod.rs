use crate::{
	equals,
	error::{Error::*, Result},
	evaluate, parse_args, primitive_equals, push, throw,
	typed::CheckType,
	with_state, ArrValue, Context, FuncVal, LazyVal, Val,
};
use format::{format_arr, format_obj};
use jrsonnet_parser::{ArgsDesc, BinaryOpType, ExprLocation};
use jrsonnet_types::{ty, ComplexValType, ValType};
use std::{collections::HashMap, path::PathBuf, rc::Rc};

pub mod stdlib;
pub use stdlib::*;

use self::manifest::{escape_string_json, manifest_json_ex, ManifestJsonOptions, ManifestType};

pub mod format;
pub mod manifest;
pub mod sort;

fn std_format(str: Rc<str>, vals: Val) -> Result<Val> {
	push(
		&Some(ExprLocation(Rc::from(PathBuf::from("std.jsonnet")), 0, 0)),
		|| format!("std.format of {}", str),
		|| {
			Ok(match vals {
				Val::Arr(vals) => Val::Str(format_arr(&str, &vals.evaluated()?)?.into()),
				Val::Obj(obj) => Val::Str(format_obj(&str, &obj)?.into()),
				o => Val::Str(format_arr(&str, &[o])?.into()),
			})
		},
	)
}

thread_local! {
	pub static INTRINSICS: HashMap<&'static str, fn(Context, &Option<ExprLocation>, &ArgsDesc) -> Result<Val>> = {
		let mut out: HashMap<&'static str, _> = HashMap::new();
		out.insert("length", intrinsic_length);
		out
	};
}

fn intrinsic_length(context: Context, _loc: &Option<ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	Ok(parse_args!(context, "length", args, 1, [
		0, x: ty!((str | obj | [any]));
	], {
		Ok(match x {
			Val::Str(n) => Val::Num(n.chars().count() as f64),
			Val::Arr(a) => Val::Num(a.len() as f64),
			Val::Obj(o) => Val::Num(
				o.fields_visibility()
					.into_iter()
					.filter(|(_k, v)| *v)
					.count() as f64,
			),
			_ => unreachable!(),
		})
	})?)
}

#[allow(clippy::cognitive_complexity)]
pub fn call_builtin(
	context: Context,
	loc: &Option<ExprLocation>,
	name: &str,
	args: &ArgsDesc,
) -> Result<Val> {
	Ok(match name as &str {
		"length" => parse_args!(context, "length", args, 1, [
			0, x: ty!((str | obj | [any]));
		], {
			Ok(match x {
				Val::Str(n) => Val::Num(n.chars().count() as f64),
				Val::Arr(a) => Val::Num(a.len() as f64),
				Val::Obj(o) => Val::Num(
					o.fields_visibility()
						.into_iter()
						.filter(|(_k, v)| *v)
						.count() as f64,
				),
				_ => unreachable!(),
			})
		})?,
		"type" => parse_args!(context, "type", args, 1, [
			0, x: ty!(any);
		], {
			Ok(Val::Str(x.value_type().name().into()))
		})?,
		"makeArray" => parse_args!(context, "makeArray", args, 2, [
			0, sz: ty!(number((Some(0.0))..(None))) => Val::Num;
			1, func: ty!(fn.any) => Val::Func;
		], {
			let mut out = Vec::with_capacity(sz as usize);
			for i in 0..sz as usize {
				out.push(LazyVal::new_resolved(func.evaluate_values(
					Context::new(),
					&[Val::Num(i as f64)]
				)?))
			}
			Ok(Val::Arr(out.into()))
		})?,
		"codepoint" => parse_args!(context, "codepoint", args, 1, [
			0, str: ty!(char) => Val::Str;
		], {
			Ok(Val::Num(str.chars().take(1).next().unwrap() as u32 as f64))
		})?,
		"objectFieldsEx" => parse_args!(context, "objectFieldsEx", args, 2, [
			0, obj: ty!(obj) => Val::Obj;
			1, inc_hidden: ty!(bool) => Val::Bool;
		], {
			let mut out = obj.fields_visibility()
				.into_iter()
				.filter(|(_k, v)| *v || inc_hidden)
				.map(|(k, _v)|k)
				.collect::<Vec<_>>();
			out.sort();
			Ok(Val::Arr(out.into_iter().map(Val::Str).collect::<Vec<_>>().into()))
		})?,
		"objectHasEx" => parse_args!(context, "objectHasEx", args, 3, [
			0, obj: ty!(obj) => Val::Obj;
			1, f: ty!(str) => Val::Str;
			2, inc_hidden: ty!(bool) => Val::Bool;
		], {
			Ok(Val::Bool(
				obj.fields_visibility()
					.into_iter()
					.filter(|(_k, v)| *v || inc_hidden)
					.any(|(k, _v)| *k == *f),
			))
		})?,
		// faster
		"slice" => parse_args!(context, "slice", args, 4, [
			0, indexable: ty!((str | [any]));
			1, index: ty!((num | null));
			2, end: ty!((num | null));
			3, step: ty!((num | null));
		], {
			let index = match index {
				Val::Num(v) => v as usize,
				Val::Null => 0,
				_ => unreachable!(),
			};
			let end = match end {
				Val::Num(v) => v as usize,
				Val::Null => match &indexable {
					Val::Str(s) => s.chars().count(),
					Val::Arr(v) => v.len(),
					_ => unreachable!()
				},
				_ => unreachable!()
			};
			let step = match step {
				Val::Num(v) => v as usize,
				Val::Null => 1,
				_ => unreachable!()
			};
			match &indexable {
				Val::Str(s) => {
					Ok(Val::Str((s.chars().skip(index).take(end-index).step_by(step).collect::<String>()).into()))
				}
				Val::Arr(arr) => {
					Ok(Val::Arr((arr.iter().skip(index).take(end-index).step_by(step).collect::<Result<Vec<Val>>>()?).into()))
				}
				_ => unreachable!()
			}
		})?,
		"primitiveEquals" => parse_args!(context, "primitiveEquals", args, 2, [
			0, a: ty!(any);
			1, b: ty!(any);
		], {
			Ok(Val::Bool(primitive_equals(&a, &b)?))
		})?,
		// faster
		"equals" => parse_args!(context, "equals", args, 2, [
			0, a: ty!(any);
			1, b: ty!(any);
		], {
			Ok(Val::Bool(equals(&a, &b)?))
		})?,
		"modulo" => parse_args!(context, "modulo", args, 2, [
			0, a: ty!(num) => Val::Num;
			1, b: ty!(num) => Val::Num;
		], {
			Ok(Val::Num(a % b))
		})?,
		"mod" => parse_args!(context, "mod", args, 2, [
			0, a: ty!((num | str));
			1, b: ty!(any);
		], {
			match (a, b) {
				(Val::Num(a), Val::Num(b)) => Ok(Val::Num(a % b)),
				(Val::Str(str), vals) => std_format(str, vals),
				(a, b) => throw!(BinaryOperatorDoesNotOperateOnValues(BinaryOpType::Mod, a.value_type(), b.value_type()))
			}
		})?,
		"floor" => parse_args!(context, "floor", args, 1, [
			0, x: ty!(num) => Val::Num;
		], {
			Ok(Val::Num(x.floor()))
		})?,
		"log" => parse_args!(context, "log", args, 1, [
			0, n: ty!(num) => Val::Num;
		], {
			Ok(Val::Num(n.ln()))
		})?,
		"trace" => parse_args!(context, "trace", args, 2, [
			0, str: ty!(str) => Val::Str;
			1, rest: ty!(any);
		], {
			eprint!("TRACE:");
			if let Some(loc) = loc {
				with_state(|s|{
					let locs = s.map_source_locations(&loc.0, &[loc.1]);
					eprint!(" {}:{}", loc.0.file_name().unwrap().to_str().unwrap(), locs[0].line);
				});
			}
			eprintln!(" {}", str);
			Ok(rest)
		})?,
		"pow" => parse_args!(context, "pow", args, 2, [
			0, x: ty!(num) => Val::Num;
			1, n: ty!(num) => Val::Num;
		], {
			Ok(Val::Num(x.powf(n)))
		})?,
		"extVar" => parse_args!(context, "extVar", args, 1, [
			0, x: ty!(str) => Val::Str;
		], {
			Ok(with_state(|s| s.settings().ext_vars.get(&x).cloned()).ok_or(UndefinedExternalVariable(x))?)
		})?,
		"native" => parse_args!(context, "native", args, 1, [
			0, x: ty!(str) => Val::Str;
		], {
			Ok(with_state(|s| s.settings().ext_natives.get(&x).cloned()).map(|v| Val::Func(Rc::new(FuncVal::NativeExt(x.clone(), v)))).ok_or(UndefinedExternalFunction(x))?)
		})?,
		"filter" => parse_args!(context, "filter", args, 2, [
			0, func: ty!(fn.any) => Val::Func;
			1, arr: ty!([any]) => Val::Arr;
		], {
			let mut out = Vec::new();
			for item in arr.iter() {
				let item = item?;
				if func
							.evaluate_values(context.clone(), &[item.clone()])?
							.try_cast_bool("filter predicate")? {
								out.push(item);
							}
			}
			Ok(Val::Arr(out.into()))
		})?,
		"foldl" => parse_args!(context, "foldl", args, 3, [
			0, func: ty!(fn.any) => Val::Func;
			1, arr: ty!([any]) => Val::Arr;
			2, init: ty!(any);
		], {
			let mut acc = init;
			for i in arr.iter() {
				acc = func.evaluate_values(context.clone(), &[acc, i?])?;
			}
			Ok(acc)
		})?,
		"foldr" => parse_args!(context, "foldr", args, 3, [
			0, func: ty!(fn.any) => Val::Func;
			1, arr: ty!([any]) => Val::Arr;
			2, init: ty!(any);
		], {
			let mut acc = init;
			for i in arr.iter().rev() {
				acc = func.evaluate_values(context.clone(), &[acc, i?])?;
			}
			Ok(acc)
		})?,
		#[allow(non_snake_case)]
		"sortImpl" => parse_args!(context, "sort", args, 2, [
			0, arr: ty!([any]) => Val::Arr;
			1, keyF: ty!(fn.any) => Val::Func;
		], {
			if arr.len() <= 1 {
				return Ok(Val::Arr(arr))
			}
			Ok(Val::Arr(ArrValue::Eager(sort::sort(context, arr.evaluated()?, &keyF)?)))
		})?,
		// faster
		"format" => parse_args!(context, "format", args, 2, [
			0, str: ty!(str) => Val::Str;
			1, vals: ty!(any)
		], {
			std_format(str, vals)
		})?,
		"range" => parse_args!(context, "range", args, 2, [
			0, from: ty!(num) => Val::Num;
			1, to: ty!(num) => Val::Num;
		], {
			let mut out = Vec::with_capacity((1+to as usize-from as usize).max(0));
			for i in from as usize..=to as usize {
				out.push(Val::Num(i as f64));
			}
			Ok(Val::Arr(out.into()))
		})?,
		"char" => parse_args!(context, "char", args, 1, [
			0, n: ty!(num) => Val::Num;
		], {
			let mut out = String::new();
			out.push(std::char::from_u32(n as u32).ok_or_else(||
				InvalidUnicodeCodepointGot(n as u32)
			)?);
			Ok(Val::Str(out.into()))
		})?,
		"encodeUTF8" => parse_args!(context, "encodeUTF8", args, 1, [
			0, str: ty!(str) => Val::Str;
		], {
			Ok(Val::Arr((str.bytes().map(|b| Val::Num(b as f64)).collect::<Vec<Val>>()).into()))
		})?,
		"md5" => parse_args!(context, "md5", args, 1, [
			0, str: ty!(str) => Val::Str;
		], {
			Ok(Val::Str(format!("{:x}", md5::compute(&str.as_bytes())).into()))
		})?,
		"base64" => parse_args!(context, "base64", args, 1, [
			0, input: ty!((str | [num]));
		], {
			Ok(Val::Str(match input {
				Val::Str(s) => {
					base64::encode(s.bytes().collect::<Vec<_>>()).into()
				},
				Val::Arr(a) => {
					base64::encode(a.iter().map(|v| {
						Ok(v?.clone().unwrap_num()? as u8)
					}).collect::<Result<Vec<_>>>()?).into()
				},
				_ => unreachable!()
			}))
		})?,
		"join" => parse_args!(context, "join", args, 2, [
			0, sep: ty!((str | [any]));
			1, arr: ty!([any]) => Val::Arr;
		], {
			Ok(match sep {
				Val::Arr(joiner_items) => {
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
							throw!(RuntimeError("in std.join all items should be arrays".into()));
						}
					}

					Val::Arr(out.into())
				},
				Val::Str(sep) => {
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
							throw!(RuntimeError("in std.join all items should be strings".into()));
						}
					}

					Val::Str(out.into())
				},
				_ => unreachable!()
			})
		})?,
		// faster
		"escapeStringJson" => parse_args!(context, "escapeStringJson", args, 1, [
			0, str_: ty!(str) => Val::Str;
		], {
			Ok(Val::Str(escape_string_json(&str_).into()))
		})?,
		// faster
		"manifestJsonEx" => parse_args!(context, "manifestJsonEx", args, 2, [
			0, value: ty!(any);
			1, indent: ty!(str) => Val::Str;
		], {
			Ok(Val::Str(manifest_json_ex(&value, &ManifestJsonOptions {
				padding: &indent,
				mtype: ManifestType::Std,
			})?.into()))
		})?,
		// faster
		"reverse" => parse_args!(context, "reverse", args, 1, [
			0, value: ty!([any]) => Val::Arr;
		], {
			Ok(Val::Arr(value.reversed()))
		})?,
		"id" => parse_args!(context, "id", args, 1, [
			0, v: ty!(any);
		], {
			Ok(v)
		})?,
		name => throw!(IntrinsicNotFound(name.into())),
	})
}
