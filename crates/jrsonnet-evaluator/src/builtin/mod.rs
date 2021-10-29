use crate::{
	builtin::manifest::{manifest_yaml_ex, ManifestYamlOptions},
	equals,
	error::{Error::*, Result},
	operator::evaluate_mod_op,
	parse_args, primitive_equals, push, throw, with_state, ArrValue, Context, EvaluationState,
	FuncVal, IndexableVal, LazyVal, Val,
};
use format::{format_arr, format_obj};
use jrsonnet_gc::Gc;
use jrsonnet_interner::IStr;
use jrsonnet_parser::{ArgsDesc, ExprLocation};
use jrsonnet_types::ty;
use std::{collections::HashMap, path::PathBuf, rc::Rc};

pub mod stdlib;
pub use stdlib::*;

use self::manifest::{escape_string_json, manifest_json_ex, ManifestJsonOptions, ManifestType};

pub mod format;
pub mod manifest;
pub mod sort;

pub fn std_format(str: IStr, vals: Val) -> Result<Val> {
	push(
		Some(&ExprLocation(Rc::from(PathBuf::from("std.jsonnet")), 0, 0)),
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

type Builtin = fn(context: Context, loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val>;

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
			("asciiUpper".into(), builtin_ascii_upper),
			("asciiLower".into(), builtin_ascii_lower),
			("member".into(), builtin_member),
			("count".into(), builtin_count),
		].iter().cloned().collect()
	};
}

fn builtin_length(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "length", args, 1, [
		0, x: ty!((string | object | array));
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
	})
}

fn builtin_type(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "type", args, 1, [
		0, x: ty!(any);
	], {
		Ok(Val::Str(x.value_type().name().into()))
	})
}

fn builtin_make_array(
	context: Context,
	_loc: Option<&ExprLocation>,
	args: &ArgsDesc,
) -> Result<Val> {
	parse_args!(context, "makeArray", args, 2, [
		0, sz: ty!(BoundedNumber<(Some(0.0)), (None)>) => Val::Num;
		1, func: ty!(function) => Val::Func;
	], {
		let mut out = Vec::with_capacity(sz as usize);
		for i in 0..sz as usize {
			out.push(LazyVal::new_resolved(func.evaluate_values(
				context.clone(),
				&[Val::Num(i as f64)]
			)?))
		}
		Ok(Val::Arr(out.into()))
	})
}

fn builtin_codepoint(
	context: Context,
	_loc: Option<&ExprLocation>,
	args: &ArgsDesc,
) -> Result<Val> {
	parse_args!(context, "codepoint", args, 1, [
		0, str: ty!(char) => Val::Str;
	], {
		Ok(Val::Num(str.chars().next().unwrap() as u32 as f64))
	})
}

fn builtin_object_fields_ex(
	context: Context,
	_loc: Option<&ExprLocation>,
	args: &ArgsDesc,
) -> Result<Val> {
	parse_args!(context, "objectFieldsEx", args, 2, [
		0, obj: ty!(object) => Val::Obj;
		1, inc_hidden: ty!(boolean) => Val::Bool;
	], {
		let out = obj.fields_ex(inc_hidden);
		Ok(Val::Arr(out.into_iter().map(Val::Str).collect::<Vec<_>>().into()))
	})
}

fn builtin_object_has_ex(
	context: Context,
	_loc: Option<&ExprLocation>,
	args: &ArgsDesc,
) -> Result<Val> {
	parse_args!(context, "objectHasEx", args, 3, [
		0, obj: ty!(object) => Val::Obj;
		1, f: ty!(string) => Val::Str;
		2, inc_hidden: ty!(boolean) => Val::Bool;
	], {
		Ok(Val::Bool(obj.has_field_ex(f, inc_hidden)))
	})
}

fn builtin_parse_json(
	context: Context,
	_loc: Option<&ExprLocation>,
	args: &ArgsDesc,
) -> Result<Val> {
	parse_args!(context, "parseJson", args, 1, [
		0, s: ty!(string) => Val::Str;
	], {
		let state = EvaluationState::default();
		let path = PathBuf::from("std.parseJson").into();
		state.evaluate_snippet_raw(path ,s)
	})
}

fn builtin_slice(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "slice", args, 4, [
		0, indexable: ty!((string | array));
		1, index: ty!((number | null));
		2, end: ty!((number | null));
		3, step: ty!((number | null));
	], {
		std_slice(
			indexable.into_indexable()?,
			index.try_cast_nullable_num("index")?.map(|v| v as usize),
			end.try_cast_nullable_num("end")?.map(|v| v as usize),
			step.try_cast_nullable_num("step")?.map(|v| v as usize),
		)
	})
}

fn builtin_substr(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "substr", args, 3, [
		0, str: ty!(string) => Val::Str;
		1, from: ty!(BoundedNumber<(Some(0.0)), (None)>) => Val::Num;
		2, len: ty!(BoundedNumber<(Some(0.0)), (None)>) => Val::Num;
	], {
		let out: String = str.chars().skip(from as usize).take(len as usize).collect();
		Ok(Val::Str(out.into()))
	})
}

fn builtin_primitive_equals(
	context: Context,
	_loc: Option<&ExprLocation>,
	args: &ArgsDesc,
) -> Result<Val> {
	parse_args!(context, "primitiveEquals", args, 2, [
		0, a: ty!(any);
		1, b: ty!(any);
	], {
		Ok(Val::Bool(primitive_equals(&a, &b)?))
	})
}

fn builtin_equals(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "equals", args, 2, [
		0, a: ty!(any);
		1, b: ty!(any);
	], {
		Ok(Val::Bool(equals(&a, &b)?))
	})
}

fn builtin_modulo(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "modulo", args, 2, [
		0, a: ty!(number) => Val::Num;
		1, b: ty!(number) => Val::Num;
	], {
		Ok(Val::Num(a % b))
	})
}

fn builtin_mod(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "mod", args, 2, [
		0, a: ty!((number | string));
		1, b: ty!(any);
	], {
		evaluate_mod_op(&a, &b)
	})
}

fn builtin_floor(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "floor", args, 1, [
		0, x: ty!(number) => Val::Num;
	], {
		Ok(Val::Num(x.floor()))
	})
}

fn builtin_ceil(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "ceil", args, 1, [
		0, x: ty!(number) => Val::Num;
	], {
		Ok(Val::Num(x.ceil()))
	})
}

fn builtin_log(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "log", args, 1, [
		0, n: ty!(number) => Val::Num;
	], {
		Ok(Val::Num(n.ln()))
	})
}

fn builtin_pow(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "pow", args, 2, [
		0, x: ty!(number) => Val::Num;
		1, n: ty!(number) => Val::Num;
	], {
		Ok(Val::Num(x.powf(n)))
	})
}

fn builtin_sqrt(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "sqrt", args, 1, [
		0, x: ty!(BoundedNumber<(Some(0.0)), (None)>) => Val::Num;
	], {
		Ok(Val::Num(x.sqrt()))
	})
}

fn builtin_sin(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "sin", args, 1, [
		0, x: ty!(number) => Val::Num;
	], {
		Ok(Val::Num(x.sin()))
	})
}

fn builtin_cos(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "cos", args, 1, [
		0, x: ty!(number) => Val::Num;
	], {
		Ok(Val::Num(x.cos()))
	})
}

fn builtin_tan(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "tan", args, 1, [
		0, x: ty!(number) => Val::Num;
	], {
		Ok(Val::Num(x.tan()))
	})
}

fn builtin_asin(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "asin", args, 1, [
		0, x: ty!(number) => Val::Num;
	], {
		Ok(Val::Num(x.asin()))
	})
}

fn builtin_acos(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "acos", args, 1, [
		0, x: ty!(number) => Val::Num;
	], {
		Ok(Val::Num(x.acos()))
	})
}

fn builtin_atan(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "atan", args, 1, [
		0, x: ty!(number) => Val::Num;
	], {
		Ok(Val::Num(x.atan()))
	})
}

fn builtin_exp(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "exp", args, 1, [
		0, x: ty!(number) => Val::Num;
	], {
		Ok(Val::Num(x.exp()))
	})
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

fn builtin_mantissa(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "mantissa", args, 1, [
		0, x: ty!(number) => Val::Num;
	], {
		Ok(Val::Num(frexp(x).0))
	})
}

fn builtin_exponent(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "exponent", args, 1, [
		0, x: ty!(number) => Val::Num;
	], {
		Ok(Val::Num(frexp(x).1.into()))
	})
}

fn builtin_ext_var(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "extVar", args, 1, [
		0, x: ty!(string) => Val::Str;
	], {
		Ok(with_state(|s| s.settings().ext_vars.get(&x).cloned()).ok_or(UndefinedExternalVariable(x))?)
	})
}

fn builtin_native(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "native", args, 1, [
		0, x: ty!(string) => Val::Str;
	], {
		Ok(with_state(|s| s.settings().ext_natives.get(&x).cloned()).map(|v| Val::Func(Gc::new(FuncVal::NativeExt(x.clone(), v)))).ok_or(UndefinedExternalFunction(x))?)
	})
}

fn builtin_filter(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "filter", args, 2, [
		0, func: ty!(function) => Val::Func;
		1, arr: ty!(array) => Val::Arr;
	], {
		Ok(Val::Arr(arr.filter(|val| func
			.evaluate_values(context.clone(), &[val.clone()])?
			.try_cast_bool("filter predicate"))?))
	})
}

fn builtin_map(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "map", args, 2, [
		0, func: ty!(function) => Val::Func;
		1, arr: ty!(array) => Val::Arr;
	], {
		Ok(Val::Arr(arr.map(|val| func
			.evaluate_values(context.clone(), &[val]))?))
	})
}

fn builtin_flatmap(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "flatMap", args, 2, [
		0, func: ty!(function) => Val::Func;
		1, arr: ty!((array | string));
	], {
		match arr {
			Val::Str(s) => {
				let mut out = String::new();
				for c in s.chars() {
					match func.evaluate_values(context.clone(), &[Val::Str(c.to_string().into())])? {
						Val::Str(o) => out.push_str(&o),
						_ => throw!(RuntimeError("in std.join all items should be strings".into())),
					};
				}
				Ok(Val::Str(out.into()))
			},
			Val::Arr(a) => {
				let mut out = Vec::new();
				for el in a.iter() {
					let el = el?;
					match func.evaluate_values(context.clone(), &[el])? {
						Val::Arr(o) => for oe in o.iter() {
							out.push(oe?)
						},
						_ => throw!(RuntimeError("in std.join all items should be arrays".into())),
					};
				}
				Ok(Val::Arr(out.into()))
			},
			_ => unreachable!(),
		}
	})
}

fn builtin_foldl(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "foldl", args, 3, [
		0, func: ty!(function) => Val::Func;
		1, arr: ty!(array) => Val::Arr;
		2, init: ty!(any);
	], {
		let mut acc = init;
		for i in arr.iter() {
			acc = func.evaluate_values(context.clone(), &[acc, i?])?;
		}
		Ok(acc)
	})
}

fn builtin_foldr(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "foldr", args, 3, [
		0, func: ty!(function) => Val::Func;
		1, arr: ty!(array) => Val::Arr;
		2, init: ty!(any);
	], {
		let mut acc = init;
		for i in arr.iter().rev() {
			acc = func.evaluate_values(context.clone(), &[i?, acc])?;
		}
		Ok(acc)
	})
}

#[allow(non_snake_case)]
fn builtin_sort_impl(
	context: Context,
	_loc: Option<&ExprLocation>,
	args: &ArgsDesc,
) -> Result<Val> {
	parse_args!(context, "sort", args, 2, [
		0, arr: ty!(array) => Val::Arr;
		1, keyF: ty!(function) => Val::Func;
	], {
		if arr.len() <= 1 {
			return Ok(Val::Arr(arr))
		}
		Ok(Val::Arr(ArrValue::Eager(sort::sort(context, arr.evaluated()?, &keyF)?)))
	})
}

fn builtin_format(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "format", args, 2, [
		0, str: ty!(string) => Val::Str;
		1, vals: ty!(any)
	], {
		std_format(str, vals)
	})
}

fn builtin_range(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "range", args, 2, [
		0, from: ty!(number) => Val::Num;
		1, to: ty!(number) => Val::Num;
	], {
		if to < from {
			return Ok(Val::Arr(ArrValue::new_eager()))
		}
		let mut out = Vec::with_capacity((1+to as usize-from as usize).max(0));
		for i in from as usize..=to as usize {
			out.push(Val::Num(i as f64));
		}
		Ok(Val::Arr(out.into()))
	})
}

fn builtin_char(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "char", args, 1, [
		0, n: ty!(number) => Val::Num;
	], {
		let mut out = String::new();
		out.push(std::char::from_u32(n as u32).ok_or_else(||
			InvalidUnicodeCodepointGot(n as u32)
		)?);
		Ok(Val::Str(out.into()))
	})
}

fn builtin_encode_utf8(
	context: Context,
	_loc: Option<&ExprLocation>,
	args: &ArgsDesc,
) -> Result<Val> {
	parse_args!(context, "encodeUTF8", args, 1, [
		0, str: ty!(string) => Val::Str;
	], {
		Ok(Val::Arr((str.bytes().map(|b| Val::Num(b as f64)).collect::<Vec<Val>>()).into()))
	})
}

fn builtin_decode_utf8(
	context: Context,
	_loc: Option<&ExprLocation>,
	args: &ArgsDesc,
) -> Result<Val> {
	parse_args!(context, "decodeUTF8", args, 1, [
		0, arr: ty!((Array<ubyte>)) => Val::Arr;
	], {
		let data: Result<Vec<u8>> = arr.iter().map(|v| v.map(|v| match v{
			Val::Num(n) => n as u8,
			_ => unreachable!(),
		})).collect();
		let data = data?;
		Ok(Val::Str(String::from_utf8(data).map_err(|_| RuntimeError("bad utf8".into()))?.into()))
	})
}

fn builtin_md5(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "md5", args, 1, [
		0, str: ty!(string) => Val::Str;
	], {
		Ok(Val::Str(format!("{:x}", md5::compute(&str.as_bytes())).into()))
	})
}

fn builtin_trace(context: Context, loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "trace", args, 2, [
		0, str: ty!(string) => Val::Str;
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
	})
}

fn builtin_base64(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "base64", args, 1, [
		0, input: ty!((string | (Array<number>)));
	], {
		Ok(Val::Str(match input {
			Val::Str(s) => {
				base64::encode(s.bytes().collect::<Vec<_>>()).into()
			},
			Val::Arr(a) => {
				base64::encode(a.iter().map(|v| {
					Ok(v?.unwrap_num()? as u8)
				}).collect::<Result<Vec<_>>>()?).into()
			},
			_ => unreachable!()
		}))
	})
}

fn builtin_base64_decode_bytes(
	context: Context,
	_loc: Option<&ExprLocation>,
	args: &ArgsDesc,
) -> Result<Val> {
	parse_args!(context, "base64DecodeBytes", args, 1, [
		0, input: ty!(string) => Val::Str;
	], {
		Ok(Val::Arr(
			base64::decode(&input.as_bytes())
				.map_err(|_| RuntimeError("bad base64".into()))?
				.iter()
				.map(|v| Val::Num(*v as f64)).collect::<Vec<_>>().into()
		))
	})
}

fn builtin_base64_decode(
	context: Context,
	_loc: Option<&ExprLocation>,
	args: &ArgsDesc,
) -> Result<Val> {
	parse_args!(context, "base64Decode", args, 1, [
		0, input: ty!(string) => Val::Str;
	], {
		Ok(Val::Str(
			String::from_utf8(base64::decode(&input.as_bytes())
				.map_err(|_| RuntimeError("bad base64".into()))?)
				.map_err(|_| RuntimeError("bad utf8".into()))?.into()
		))
	})
}

fn builtin_join(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "join", args, 2, [
		0, sep: ty!((string | array));
		1, arr: ty!(array) => Val::Arr;
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
	})
}

fn builtin_escape_string_json(
	context: Context,
	_loc: Option<&ExprLocation>,
	args: &ArgsDesc,
) -> Result<Val> {
	parse_args!(context, "escapeStringJson", args, 1, [
		0, str_: ty!(string) => Val::Str;
	], {
		Ok(Val::Str(escape_string_json(&str_).into()))
	})
}

fn builtin_manifest_json_ex(
	context: Context,
	_loc: Option<&ExprLocation>,
	args: &ArgsDesc,
) -> Result<Val> {
	parse_args!(context, "manifestJsonEx", args, 2, [
		0, value: ty!(any);
		1, indent: ty!(string) => Val::Str;
	], {
		Ok(Val::Str(manifest_json_ex(&value, &ManifestJsonOptions {
			padding: &indent,
			mtype: ManifestType::Std,
		})?.into()))
	})
}

fn builtin_manifest_yaml_doc(
	context: Context,
	_loc: Option<&ExprLocation>,
	args: &ArgsDesc,
) -> Result<Val> {
	parse_args!(context, "manifestYamlDoc", args, 2, [
		0, value: ty!(any);
		1, indent_array_in_object: ty!(boolean) => Val::Bool;
	], {
		Ok(Val::Str(manifest_yaml_ex(&value, &ManifestYamlOptions {
			padding: "  ",
			arr_element_padding: if indent_array_in_object { "  " } else { "" },
		})?.into()))
	})
}

fn builtin_reverse(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "reverse", args, 1, [
		0, value: ty!(array) => Val::Arr;
	], {
		Ok(Val::Arr(value.reversed()))
	})
}

fn builtin_id(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "id", args, 1, [
		0, v: ty!(any);
	], {
		Ok(v)
	})
}

fn builtin_str_replace(
	context: Context,
	_loc: Option<&ExprLocation>,
	args: &ArgsDesc,
) -> Result<Val> {
	parse_args!(context, "strReplace", args, 3, [
		0, str: ty!(string) => Val::Str;
		1, from: ty!(string) => Val::Str;
		2, to: ty!(string) => Val::Str;
	], {
		Ok(Val::Str(str.replace(&from as &str, &to as &str).into()))
	})
}

fn builtin_splitlimit(
	context: Context,
	_loc: Option<&ExprLocation>,
	args: &ArgsDesc,
) -> Result<Val> {
	parse_args!(context, "splitLimit", args, 3, [
		0, str: ty!(string) => Val::Str;
		1, c: ty!(char) => Val::Str;
		2, maxsplits: ty!(number) => Val::Num;
	], {
		let maxsplits = maxsplits as isize;
		let c = c.chars().next().unwrap();

		let out: Vec<Val> = if maxsplits == -1 {
			str.split(c).map(|s| Val::Str(s.into())).collect()
		} else {
			str.splitn(maxsplits as usize + 1, c).map(|s| Val::Str(s.into())).collect()
		};

		Ok(Val::Arr(out.into()))
	})
}

fn builtin_ascii_upper(
	context: Context,
	_loc: Option<&ExprLocation>,
	args: &ArgsDesc,
) -> Result<Val> {
	parse_args!(context, "asciiUpper", args, 1, [
		0, str: ty!(string) => Val::Str;
	], {
		Ok(Val::Str(str.to_ascii_uppercase().into()))
	})
}

fn builtin_ascii_lower(
	context: Context,
	_loc: Option<&ExprLocation>,
	args: &ArgsDesc,
) -> Result<Val> {
	parse_args!(context, "asciiLower", args, 1, [
		0, str: ty!(string) => Val::Str;
	], {
		Ok(Val::Str(str.to_ascii_lowercase().into()))
	})
}

fn builtin_member(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "member", args, 2, [
		0, arr: ty!((array | string));
		1, x: ty!(any);
	], {
		match arr {
			Val::Str(s) => {
				let x = x.try_cast_str("x should be string")?;
				Ok(Val::Bool(!x.is_empty() && s.contains(&*x)))
			}
			Val::Arr(a) => {
				for item in a.iter() {
					let item = item?;
					if equals(&item, &x)? {
						return Ok(Val::Bool(true));
					}
				}
				Ok(Val::Bool(false))
			}
			_ => unreachable!(),
		}
	})
}

fn builtin_count(context: Context, _loc: Option<&ExprLocation>, args: &ArgsDesc) -> Result<Val> {
	parse_args!(context, "count", args, 2, [
		0, arr: ty!(array) => Val::Arr;
		1, x: ty!(any);
	], {
		let mut count = 0;
		for item in arr.iter() {
			let item = item?;
			if equals(&item, &x)? {
				count += 1;
			}
		}
		Ok(Val::Num(count as f64))
	})
}

pub fn call_builtin(
	context: Context,
	loc: Option<&ExprLocation>,
	name: &str,
	args: &ArgsDesc,
) -> Result<Val> {
	BUILTINS
		.with(|builtins| builtins.get(name).copied())
		.ok_or_else(|| IntrinsicNotFound(name.into()))?(context, loc, args)
}
