use crate::{
	equals,
	error::{Error::*, Result},
	evaluate, parse_args, primitive_equals, push, throw, with_state, Context, FuncVal, Val,
	ValType,
};
use format::{format_arr, format_obj};
use jrsonnet_parser::{ArgsDesc, ExprLocation};
use manifest::{escape_string_json, manifest_json_ex, ManifestJsonOptions, ManifestType};
use std::{path::PathBuf, rc::Rc};

pub mod stdlib;
pub use stdlib::*;

pub mod format;
pub mod manifest;
pub mod sort;

fn std_format(str: Rc<str>, vals: Val) -> Result<Val> {
	push(
		&Some(ExprLocation(Rc::from(PathBuf::from("std.jsonnet")), 0, 0)),
		|| format!("std.format of {}", str),
		|| {
			Ok(match vals {
				Val::Arr(vals) => Val::Str(format_arr(&str, &vals)?.into()),
				Val::Obj(obj) => Val::Str(format_obj(&str, &obj)?.into()),
				o => Val::Str(format_arr(&str, &[o])?.into()),
			})
		},
	)
}

#[allow(clippy::cognitive_complexity)]
pub fn call_builtin(
	context: Context,
	loc: &Option<ExprLocation>,
	name: &str,
	args: &ArgsDesc,
) -> Result<Val> {
	Ok(match name as &str {
		// arr/string/function
		"length" => parse_args!(context, "std.length", args, 1, [
			0, x: [Val::Str|Val::Arr|Val::Obj], vec![ValType::Str, ValType::Arr, ValType::Obj];
		], {
			Ok(match x {
				Val::Str(n) => Val::Num(n.chars().count() as f64),
				Val::Arr(i) => Val::Num(i.len() as f64),
				Val::Obj(o) => Val::Num(
					o.fields_visibility()
						.into_iter()
						.filter(|(_k, v)| *v)
						.count() as f64,
				),
				_ => unreachable!(),
			})
		})?,
		// any
		"type" => parse_args!(context, "std.type", args, 1, [
			0, x, vec![];
		], {
			Ok(Val::Str(x.value_type()?.name().into()))
		})?,
		// length, idx=>any
		"makeArray" => parse_args!(context, "std.makeArray", args, 2, [
			0, sz: [Val::Num]!!Val::Num, vec![ValType::Num];
			1, func: [Val::Func]!!Val::Func, vec![ValType::Func];
		], {
			if sz < 0.0 {
				throw!(RuntimeError(format!("makeArray requires size >= 0, got {}", sz).into()));
			}
			let mut out = Vec::with_capacity(sz as usize);
			for i in 0..sz as usize {
				out.push(func.evaluate_values(
					Context::new(),
					&[Val::Num(i as f64)]
				)?)
			}
			Ok(Val::Arr(Rc::new(out)))
		})?,
		// string
		"codepoint" => parse_args!(context, "std.codepoint", args, 1, [
			0, str: [Val::Str]!!Val::Str, vec![ValType::Str];
		], {
			assert!(
				str.chars().count() == 1,
				"std.codepoint should receive single char string"
			);
			Ok(Val::Num(str.chars().take(1).next().unwrap() as u32 as f64))
		})?,
		// object, includeHidden
		"objectFieldsEx" => parse_args!(context, "std.objectFieldsEx",args, 2, [
			0, obj: [Val::Obj]!!Val::Obj, vec![ValType::Obj];
			1, inc_hidden: [Val::Bool]!!Val::Bool, vec![ValType::Bool];
		], {
			let mut out = obj.fields_visibility()
				.into_iter()
				.filter(|(_k, v)| *v || inc_hidden)
				.map(|(k, _v)|k)
				.collect::<Vec<_>>();
			out.sort();
			Ok(Val::Arr(Rc::new(out.into_iter().map(Val::Str).collect())))
		})?,
		// object, field, includeHidden
		"objectHasEx" => parse_args!(context, "std.objectHasEx", args, 3, [
			0, obj: [Val::Obj]!!Val::Obj, vec![ValType::Obj];
			1, f: [Val::Str]!!Val::Str, vec![ValType::Str];
			2, inc_hidden: [Val::Bool]!!Val::Bool, vec![ValType::Bool];
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
			0, indexable: [Val::Str | Val::Arr], vec![ValType::Str, ValType::Arr];
			1, index, vec![ValType::Num, ValType::Null];
			2, end, vec![ValType::Num, ValType::Null];
			3, step, vec![ValType::Num, ValType::Null];
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
					Ok(Val::Arr((arr.iter().skip(index).take(end-index).step_by(step).cloned().collect::<Vec<Val>>()).into()))
				}
				_ => unreachable!()
			}
		})?,
		"primitiveEquals" => parse_args!(context, "std.primitiveEquals", args, 2, [
			0, a, vec![];
			1, b, vec![];
		], {
			Ok(Val::Bool(primitive_equals(&a, &b)?))
		})?,
		// faster
		"equals" => parse_args!(context, "std.equals", args, 2, [
			0, a, vec![];
			1, b, vec![];
		], {
			Ok(Val::Bool(equals(&a, &b)?))
		})?,
		"mod" => parse_args!(context, "std.mod", args, 2, [
			0, a: [Val::Num | Val::Str], vec![ValType::Num, ValType::Str];
			1, b, vec![];
		], {
			match (a, b) {
				(Val::Num(a), Val::Num(b)) => Ok(Val::Num(a % b)),
				(Val::Str(str), vals) => std_format(str, vals),
				(a, b) => throw!(BinaryOperatorDoesNotOperateOnValues(jrsonnet_parser::BinaryOpType::Mod, a.value_type()?, b.value_type()?))
			}
		})?,
		"modulo" => parse_args!(context, "std.modulo", args, 2, [
			0, a: [Val::Num]!!Val::Num, vec![ValType::Num];
			1, b: [Val::Num]!!Val::Num, vec![ValType::Num];
		], {
			Ok(Val::Num(a % b))
		})?,
		"floor" => parse_args!(context, "std.floor", args, 1, [
			0, x: [Val::Num]!!Val::Num, vec![ValType::Num];
		], {
			Ok(Val::Num(x.floor()))
		})?,
		"log" => parse_args!(context, "std.log", args, 2, [
			0, n: [Val::Num]!!Val::Num, vec![ValType::Num];
		], {
			Ok(Val::Num(n.ln()))
		})?,
		"trace" => parse_args!(context, "std.trace", args, 2, [
			0, str: [Val::Str]!!Val::Str, vec![ValType::Str];
			1, rest, vec![];
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
		"pow" => parse_args!(context, "std.modulo", args, 2, [
			0, x: [Val::Num]!!Val::Num, vec![ValType::Num];
			1, n: [Val::Num]!!Val::Num, vec![ValType::Num];
		], {
			Ok(Val::Num(x.powf(n)))
		})?,
		"extVar" => parse_args!(context, "std.extVar", args, 1, [
			0, x: [Val::Str]!!Val::Str, vec![ValType::Str];
		], {
			Ok(with_state(|s| s.settings().ext_vars.get(&x).cloned()).ok_or(UndefinedExternalVariable(x))?)
		})?,
		"native" => parse_args!(context, "std.native", args, 1, [
			0, x: [Val::Str]!!Val::Str, vec![ValType::Str];
		], {
			Ok(with_state(|s| s.settings().ext_natives.get(&x).cloned()).map(|v| Val::Func(Rc::new(FuncVal::NativeExt(x.clone(), v)))).ok_or(UndefinedExternalFunction(x))?)
		})?,
		"filter" => parse_args!(context, "std.filter", args, 2, [
			0, func: [Val::Func]!!Val::Func, vec![ValType::Func];
			1, arr: [Val::Arr]!!Val::Arr, vec![ValType::Arr];
		], {
			Ok(Val::Arr(Rc::new(
				arr.iter()
					.cloned()
					.filter(|e| {
						func
							.evaluate_values(context.clone(), &[e.clone()])
							.unwrap()
							.try_cast_bool("filter predicate")
							.unwrap()
					})
					.collect(),
			)))
		})?,
		// faster
		"foldl" => parse_args!(context, "std.foldl", args, 3, [
			0, func: [Val::Func]!!Val::Func, vec![ValType::Func];
			1, arr: [Val::Arr]!!Val::Arr, vec![ValType::Arr];
			2, init, vec![];
		], {
			let mut acc = init;
			for i in arr.iter().cloned() {
				acc = func.evaluate_values(context.clone(), &[acc, i])?;
			}
			Ok(acc)
		})?,
		// faster
		"foldr" => parse_args!(context, "std.foldr", args, 3, [
			0, func: [Val::Func]!!Val::Func, vec![ValType::Func];
			1, arr: [Val::Arr]!!Val::Arr, vec![ValType::Arr];
			2, init, vec![];
		], {
			let mut acc = init;
			for i in arr.iter().rev().cloned() {
				acc = func.evaluate_values(context.clone(), &[acc, i])?;
			}
			Ok(acc)
		})?,
		// faster
		#[allow(non_snake_case)]
		"sortImpl" => parse_args!(context, "std.sort", args, 2, [
			0, arr: [Val::Arr]!!Val::Arr, vec![ValType::Arr];
			1, keyF: [Val::Func]!!Val::Func, vec![ValType::Func];
		], {
			if arr.len() <= 1 {
				return Ok(Val::Arr(arr))
			}
			Ok(Val::Arr(sort::sort(context, arr, &keyF)?))
		})?,
		// faster
		"format" => parse_args!(context, "std.format", args, 2, [
			0, str: [Val::Str]!!Val::Str, vec![ValType::Str];
			1, vals, vec![]
		], {
			std_format(str, vals)
		})?,
		// faster
		"range" => parse_args!(context, "std.range", args, 2, [
			0, from: [Val::Num]!!Val::Num, vec![ValType::Num];
			1, to: [Val::Num]!!Val::Num, vec![ValType::Num];
		], {
			if to < from {
				return Ok(Val::Arr(Rc::new(Vec::new())))
			}
			let mut out = Vec::with_capacity((1+to as usize-from as usize).max(0));
			for i in from as usize..=to as usize {
				out.push(Val::Num(i as f64));
			}
			Ok(Val::Arr(Rc::new(out)))
		})?,
		"char" => parse_args!(context, "std.char", args, 1, [
			0, n: [Val::Num]!!Val::Num, vec![ValType::Num];
		], {
			let mut out = String::new();
			out.push(std::char::from_u32(n as u32).ok_or_else(||
				InvalidUnicodeCodepointGot(n as u32)
			)?);
			Ok(Val::Str(out.into()))
		})?,
		"encodeUTF8" => parse_args!(context, "std.encodeUtf8", args, 1, [
			0, str: [Val::Str]!!Val::Str, vec![ValType::Str];
		], {
			Ok(Val::Arr(Rc::new(str.bytes().map(|b| Val::Num(b as f64)).collect())))
		})?,
		"md5" => parse_args!(context, "std.md5", args, 1, [
			0, str: [Val::Str]!!Val::Str, vec![ValType::Str];
		], {
			Ok(Val::Str(format!("{:x}", md5::compute(&str.as_bytes())).into()))
		})?,
		// faster
		"base64" => parse_args!(context, "std.base64", args, 1, [
			0, input: [Val::Str | Val::Arr], vec![ValType::Arr, ValType::Str];
		], {
			Ok(Val::Str(match input {
				Val::Str(s) => {
					base64::encode(s.bytes().collect::<Vec<_>>()).into()
				},
				Val::Arr(a) => {
					base64::encode(a.iter().map(|v| {
						Ok(v.clone().try_cast_num("base64 array")? as u8)
					}).collect::<Result<Vec<_>>>()?).into()
				},
				_ => unreachable!()
			}))
		})?,
		// faster
		"join" => parse_args!(context, "std.join", args, 2, [
			0, sep: [Val::Str|Val::Arr], vec![ValType::Str, ValType::Arr];
			1, arr: [Val::Arr]!!Val::Arr, vec![ValType::Arr];
		], {
			Ok(match sep {
				Val::Arr(joiner_items) => {
					let mut out = Vec::new();

					let mut first = true;
					for item in arr.iter().cloned() {
						if let Val::Arr(items) = item.unwrap_if_lazy()? {
							if !first {
								out.reserve(joiner_items.len());
								out.extend(joiner_items.iter().cloned());
							}
							first = false;
							out.reserve(items.len());
							out.extend(items.iter().cloned());
						} else {
							throw!(RuntimeError("in std.join all items should be arrays".into()));
						}
					}

					Val::Arr(Rc::new(out))
				},
				Val::Str(sep) => {
					let mut out = String::new();

					let mut first = true;
					for item in arr.iter().cloned() {
						if let Val::Str(item) = item.unwrap_if_lazy()? {
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
		// Faster
		"escapeStringJson" => parse_args!(context, "std.escapeStringJson", args, 1, [
			0, str_: [Val::Str]!!Val::Str, vec![ValType::Str];
		], {
			Ok(Val::Str(escape_string_json(&str_).into()))
		})?,
		// Faster
		"manifestJsonEx" => parse_args!(context, "std.manifestJsonEx", args, 2, [
			0, value, vec![];
			1, indent: [Val::Str]!!Val::Str, vec![ValType::Str];
		], {
			Ok(Val::Str(manifest_json_ex(&value, &ManifestJsonOptions {
				padding: &indent,
				mtype: ManifestType::Std,
			})?.into()))
		})?,
		// Faster
		"reverse" => parse_args!(context, "std.reverse", args, 1, [
			0, arr: [Val::Arr]!!Val::Arr, vec![ValType::Arr];
		], {
			let mut marr = arr;
			Rc::make_mut(&mut marr).reverse();
			Ok(Val::Arr(marr))
		})?,
		"id" => parse_args!(context, "std.id", args, 1, [
			0, v, vec![];
		], {
			Ok(v)
		})?,
		name => throw!(IntrinsicNotFound(name.into())),
	})
}
