use crate::{
	equals,
	error::{Error::*, Result},
	evaluate, parse_args, primitive_equals, push, throw, with_state, Context, FuncVal, Val,
	ValType,
};
use format::{format_arr, format_obj};
use gc::Gc;
use jrsonnet_parser::{ArgsDesc, ExprLocation};
use manifest::{escape_string_json, manifest_json_ex, ManifestJsonOptions, ManifestType};
use std::{path::PathBuf, rc::Rc};

pub mod stdlib;
pub use stdlib::*;

pub mod format;
pub mod manifest;
pub mod sort;

#[allow(clippy::cognitive_complexity)]
pub fn call_builtin(
	context: Context,
	loc: &Option<ExprLocation>,
	ns: &str,
	name: &str,
	args: &ArgsDesc,
) -> Result<Val> {
	Ok(match (ns, name as &str) {
		// arr/string/function
		("std", "length") => parse_args!(context, "std.length", args, 1, [
			0, x: [Val::Str|Val::Arr|Val::Obj], vec![ValType::Str, ValType::Arr, ValType::Obj];
		], {
			Ok(match &x {
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
		("std", "type") => parse_args!(context, "std.type", args, 1, [
			0, x, vec![];
		], {
			Ok(Val::Str(x.value_type()?.name().into()))
		})?,
		// length, idx=>any
		("std", "makeArray") => parse_args!(context, "std.makeArray", args, 2, [
			0, sz: [Val::Num]!!Val::Num, vec![ValType::Num];
			1, func: [Val::Func]!!Val::Func, vec![ValType::Func];
		], {
			if *sz < 0.0 {
				throw!(RuntimeError(format!("makeArray requires size >= 0, got {}", sz).into()));
			}
			let mut out = Vec::with_capacity(*sz as usize);
			for i in 0..*sz as usize {
				out.push(func.evaluate_values(
					Context::new(),
					&[Val::Num(i as f64)]
				)?)
			}
			Ok(Val::Arr(Gc::new(out)))
		})?,
		// string
		("std", "codepoint") => parse_args!(context, "std.codepoint", args, 1, [
			0, str: [Val::Str]!!Val::Str, vec![ValType::Str];
		], {
			assert!(
				str.chars().count() == 1,
				"std.codepoint should receive single char string"
			);
			Ok(Val::Num(str.chars().take(1).next().unwrap() as u32 as f64))
		})?,
		// object, includeHidden
		("std", "objectFieldsEx") => parse_args!(context, "std.objectFieldsEx",args, 2, [
			0, obj: [Val::Obj]!!Val::Obj, vec![ValType::Obj];
			1, inc_hidden: [Val::Bool]!!Val::Bool, vec![ValType::Bool];
		], {
			let mut out = obj.fields_visibility()
				.into_iter()
				.filter(|(_k, v)| *v || *inc_hidden)
				.map(|(k, _v)|k)
				.collect::<Vec<_>>();
			out.sort();
			Ok(Val::Arr(Gc::new(out.into_iter().map(Val::Str).collect())))
		})?,
		// object, field, includeHidden
		("std", "objectHasEx") => parse_args!(context, "std.objectHasEx", args, 3, [
			0, obj: [Val::Obj]!!Val::Obj, vec![ValType::Obj];
			1, f: [Val::Str]!!Val::Str, vec![ValType::Str];
			2, inc_hidden: [Val::Bool]!!Val::Bool, vec![ValType::Bool];
		], {
			Ok(Val::Bool(
				obj.fields_visibility()
					.into_iter()
					.filter(|(_k, v)| *v || *inc_hidden)
					.any(|(k, _v)| k == *f),
			))
		})?,
		("std", "primitiveEquals") => parse_args!(context, "std.primitiveEquals", args, 2, [
			0, a, vec![];
			1, b, vec![];
		], {
			Ok(Val::Bool(primitive_equals(&a, &b)?))
		})?,
		// faster
		("std", "equals") => parse_args!(context, "std.equals", args, 2, [
			0, a, vec![];
			1, b, vec![];
		], {
			Ok(Val::Bool(equals(&a, &b)?))
		})?,
		("std", "modulo") => parse_args!(context, "std.modulo", args, 2, [
			0, a: [Val::Num]!!Val::Num, vec![ValType::Num];
			1, b: [Val::Num]!!Val::Num, vec![ValType::Num];
		], {
			Ok(Val::Num(a % b))
		})?,
		("std", "floor") => parse_args!(context, "std.floor", args, 1, [
			0, x: [Val::Num]!!Val::Num, vec![ValType::Num];
		], {
			Ok(Val::Num(x.floor()))
		})?,
		("std", "log") => parse_args!(context, "std.log", args, 2, [
			0, n: [Val::Num]!!Val::Num, vec![ValType::Num];
		], {
			Ok(Val::Num(n.ln()))
		})?,
		("std", "trace") => parse_args!(context, "std.trace", args, 2, [
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
		("std", "pow") => parse_args!(context, "std.modulo", args, 2, [
			0, x: [Val::Num]!!Val::Num, vec![ValType::Num];
			1, n: [Val::Num]!!Val::Num, vec![ValType::Num];
		], {
			Ok(Val::Num(x.powf(n.clone())))
		})?,
		("std", "extVar") => parse_args!(context, "std.extVar", args, 1, [
			0, x: [Val::Str]!!Val::Str, vec![ValType::Str];
		], {
			Ok(with_state(|s| s.settings().ext_vars.get(&x.clone()).cloned()).ok_or_else(
				|| UndefinedExternalVariable(x.clone()),
			)?)
		})?,
		("std", "native") => parse_args!(context, "std.native", args, 1, [
			0, x: [Val::Str]!!Val::Str, vec![ValType::Str];
		], {
			Ok(with_state(|s| s.settings().ext_natives.get(&x.clone()).cloned()).map(|v| Val::Func(Gc::new(FuncVal::NativeExt(x.clone(), v)))).ok_or_else(
				|| UndefinedExternalFunction(x.clone()),
			)?)
		})?,
		("std", "filter") => parse_args!(context, "std.filter", args, 2, [
			0, func: [Val::Func]!!Val::Func, vec![ValType::Func];
			1, arr: [Val::Arr]!!Val::Arr, vec![ValType::Arr];
		], {
			Ok(Val::Arr(Gc::new(
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
		("std", "foldl") => parse_args!(context, "std.foldl", args, 3, [
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
		("std", "foldr") => parse_args!(context, "std.foldr", args, 3, [
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
		("std", "sortImpl") => parse_args!(context, "std.sort", args, 2, [
			0, arr: [Val::Arr]!!Val::Arr, vec![ValType::Arr];
			1, keyF: [Val::Func]!!Val::Func, vec![ValType::Func];
		], {
			if arr.len() <= 1 {
				return Ok(Val::Arr(arr.clone()))
			}
			Ok(Val::Arr(sort::sort(context, arr.clone(), &keyF)?))
		})?,
		// faster
		("std", "format") => parse_args!(context, "std.format", args, 2, [
			0, str: [Val::Str]!!Val::Str, vec![ValType::Str];
			1, vals, vec![]
		], {
			push(&Some(ExprLocation(Rc::from(PathBuf::from("std.jsonnet")), 0, 0)), ||format!("std.format of {}", str), ||{
				Ok(match &vals {
					Val::Arr(vals) => Val::Str(format_arr(&str, &vals)?.into()),
					Val::Obj(obj) => Val::Str(format_obj(&str, &obj)?.into()),
					o => Val::Str(format_arr(&str, &[o.clone()])?.into()),
				})
			})
		})?,
		// faster
		("std", "range") => parse_args!(context, "std.range", args, 2, [
			0, from: [Val::Num]!!Val::Num, vec![ValType::Num];
			1, to: [Val::Num]!!Val::Num, vec![ValType::Num];
		], {
			let mut out = Vec::with_capacity((1+*to as usize-*from as usize).max(0));
			for i in *from as usize..=*to as usize {
				out.push(Val::Num(i as f64));
			}
			Ok(Val::Arr(Gc::new(out)))
		})?,
		("std", "char") => parse_args!(context, "std.char", args, 1, [
			0, n: [Val::Num]!!Val::Num, vec![ValType::Num];
		], {
			let mut out = String::new();
			out.push(std::char::from_u32(*n as u32).ok_or_else(||
				InvalidUnicodeCodepointGot(*n as u32)
			)?);
			Ok(Val::Str(out.into()))
		})?,
		("std", "encodeUTF8") => parse_args!(context, "std.encodeUtf8", args, 1, [
			0, str: [Val::Str]!!Val::Str, vec![ValType::Str];
		], {
			Ok(Val::Arr(Gc::new(str.bytes().map(|b| Val::Num(b as f64)).collect())))
		})?,
		("std", "md5") => parse_args!(context, "std.md5", args, 1, [
			0, str: [Val::Str]!!Val::Str, vec![ValType::Str];
		], {
			Ok(Val::Str(format!("{:x}", md5::compute(&str.as_bytes())).into()))
		})?,
		// faster
		("std", "base64") => parse_args!(context, "std.base64", args, 1, [
			0, input: [Val::Str | Val::Arr], vec![ValType::Arr, ValType::Str];
		], {
			Ok(Val::Str(match &input {
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
		("std", "join") => parse_args!(context, "std.join", args, 2, [
			0, sep: [Val::Str|Val::Arr], vec![ValType::Str, ValType::Arr];
			1, arr: [Val::Arr]!!Val::Arr, vec![ValType::Arr];
		], {
			Ok(match &sep {
				Val::Arr(joiner_items) => {
					let mut out = Vec::new();

					let mut first = true;
					for item in arr.iter().cloned() {
						if let Val::Arr(items) = &item.unwrap_if_lazy()? {
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

					Val::Arr(Gc::new(out))
				},
				Val::Str(sep) => {
					let mut out = String::new();

					let mut first = true;
					for item in arr.iter().cloned() {
						if let Val::Str(item) = &item.unwrap_if_lazy()? {
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
		("std", "escapeStringJson") => parse_args!(context, "std.escapeStringJson", args, 1, [
			0, str_: [Val::Str]!!Val::Str, vec![ValType::Str];
		], {
			Ok(Val::Str(escape_string_json(&str_).into()))
		})?,
		// Faster
		("std", "manifestJsonEx") => parse_args!(context, "std.manifestJsonEx", args, 2, [
			0, value, vec![];
			1, indent: [Val::Str]!!Val::Str, vec![ValType::Str];
		], {
			Ok(Val::Str(manifest_json_ex(&value, &ManifestJsonOptions {
				padding: &indent,
				mtype: ManifestType::Std,
			})?.into()))
		})?,
		// Faster
		("std", "reverse") => parse_args!(context, "std.reverse", args, 1, [
			0, arr: [Val::Arr]!!Val::Arr, vec![ValType::Arr];
		], {
			let mut marr = (&arr as &Vec<_>).clone();
			marr.reverse();
			Ok(Val::Arr(Gc::new(marr)))
		})?,
		("std", "id") => parse_args!(context, "std.id", args, 1, [
			0, v, vec![];
		], {
			Ok(v)
		})?,
		(ns, name) => throw!(IntrinsicNotFound(ns.into(), name.into())),
	})
}
