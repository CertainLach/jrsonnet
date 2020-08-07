use crate::{
	builtin::{
		call_builtin,
		manifest::{manifest_json_ex, ManifestJsonOptions, ManifestType},
	},
	error::Error::*,
	evaluate,
	function::{parse_function_call, parse_function_call_map, place_args},
	native::NativeCallback,
	throw, with_state, Context, ObjValue, Result,
};
use jrsonnet_parser::{el, Arg, ArgsDesc, Expr, ExprLocation, LocExpr, ParamsDesc};
use std::{
	cell::RefCell,
	collections::HashMap,
	fmt::{Debug, Display},
	rc::Rc,
};

enum LazyValInternals {
	Computed(Val),
	Waiting(Box<dyn Fn() -> Result<Val>>),
}
#[derive(Clone)]
pub struct LazyVal(Rc<RefCell<LazyValInternals>>);
impl LazyVal {
	pub fn new(f: Box<dyn Fn() -> Result<Val>>) -> Self {
		LazyVal(Rc::new(RefCell::new(LazyValInternals::Waiting(f))))
	}
	pub fn new_resolved(val: Val) -> Self {
		LazyVal(Rc::new(RefCell::new(LazyValInternals::Computed(val))))
	}
	pub fn evaluate(&self) -> Result<Val> {
		let new_value = match &*self.0.borrow() {
			LazyValInternals::Computed(v) => return Ok(v.clone()),
			LazyValInternals::Waiting(f) => f()?,
		};
		*self.0.borrow_mut() = LazyValInternals::Computed(new_value.clone());
		Ok(new_value)
	}
}

#[macro_export]
macro_rules! lazy_val {
	($f: expr) => {
		$crate::LazyVal::new(Box::new($f))
	};
}
#[macro_export]
macro_rules! resolved_lazy_val {
	($f: expr) => {
		$crate::LazyVal::new_resolved($f)
	};
}
impl Debug for LazyVal {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Lazy")
	}
}
impl PartialEq for LazyVal {
	fn eq(&self, other: &Self) -> bool {
		Rc::ptr_eq(&self.0, &other.0)
	}
}

#[derive(Debug, PartialEq)]
pub struct FuncDesc {
	pub name: Rc<str>,
	pub ctx: Context,
	pub params: ParamsDesc,
	pub body: LocExpr,
}

#[derive(Debug)]
pub enum FuncVal {
	/// Plain function implemented in jsonnet
	Normal(FuncDesc),
	/// Standard library function
	Intristic(Rc<str>, Rc<str>),
	/// Library functions implemented in native
	NativeExt(Rc<str>, Rc<NativeCallback>),
}

impl PartialEq for FuncVal {
	fn eq(&self, other: &Self) -> bool {
		match (self, other) {
			(FuncVal::Normal(a), FuncVal::Normal(b)) => a == b,
			(FuncVal::Intristic(ans, an), FuncVal::Intristic(bns, bn)) => ans == bns && an == bn,
			(FuncVal::NativeExt(an, _), FuncVal::NativeExt(bn, _)) => an == bn,
			(..) => false,
		}
	}
}
impl FuncVal {
	pub fn is_ident(&self) -> bool {
		matches!(&self, FuncVal::Intristic(ns, n) if ns as &str == "std" && n as &str == "id")
	}
	pub fn name(&self) -> Rc<str> {
		match self {
			FuncVal::Normal(normal) => normal.name.clone(),
			FuncVal::Intristic(ns, name) => format!("intristic.{}.{}", ns, name).into(),
			FuncVal::NativeExt(n, _) => format!("native.{}", n).into(),
		}
	}
	pub fn evaluate(
		&self,
		call_ctx: Context,
		loc: &Option<ExprLocation>,
		args: &ArgsDesc,
		tailstrict: bool,
	) -> Result<Val> {
		match self {
			FuncVal::Normal(func) => {
				let ctx = parse_function_call(
					call_ctx,
					Some(func.ctx.clone()),
					&func.params,
					args,
					tailstrict,
				)?;
				evaluate(ctx, &func.body)
			}
			FuncVal::Intristic(ns, name) => call_builtin(call_ctx, loc, &ns, &name, args),
			FuncVal::NativeExt(_name, handler) => {
				let args = parse_function_call(call_ctx, None, &handler.params, args, true)?;
				let mut out_args = Vec::with_capacity(handler.params.len());
				for p in handler.params.0.iter() {
					out_args.push(args.binding(p.0.clone())?.evaluate()?);
				}
				Ok(handler.call(&out_args)?)
			}
		}
	}

	pub fn evaluate_map(
		&self,
		call_ctx: Context,
		args: &HashMap<Rc<str>, Val>,
		tailstrict: bool,
	) -> Result<Val> {
		match self {
			FuncVal::Normal(func) => {
				let ctx = parse_function_call_map(
					call_ctx,
					Some(func.ctx.clone()),
					&func.params,
					args,
					tailstrict,
				)?;
				evaluate(ctx, &func.body)
			}
			FuncVal::Intristic(_, _) => todo!(),
			FuncVal::NativeExt(_, _) => todo!(),
		}
	}

	pub fn evaluate_values(&self, call_ctx: Context, args: &[Val]) -> Result<Val> {
		match self {
			FuncVal::Normal(func) => {
				let ctx = place_args(call_ctx, Some(func.ctx.clone()), &func.params, args)?;
				evaluate(ctx, &func.body)
			}
			FuncVal::Intristic(_, _) => todo!(),
			FuncVal::NativeExt(_, _) => todo!(),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValType {
	Bool,
	Null,
	Str,
	Num,
	Arr,
	Obj,
	Func,
}
impl ValType {
	pub fn name(&self) -> &'static str {
		use ValType::*;
		match self {
			Bool => "boolean",
			Null => "null",
			Str => "string",
			Num => "number",
			Arr => "array",
			Obj => "object",
			Func => "function",
		}
	}
}
impl Display for ValType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.name())
	}
}

#[derive(Clone)]
pub enum ManifestFormat {
	YamlStream(Box<ManifestFormat>),
	Yaml(usize),
	Json(usize),
	String,
}

#[derive(Debug, Clone)]
pub enum Val {
	Bool(bool),
	Null,
	Str(Rc<str>),
	Num(f64),
	Lazy(LazyVal),
	Arr(Rc<Vec<Val>>),
	Obj(ObjValue),
	Func(Rc<FuncVal>),
}

macro_rules! matches_unwrap {
	($e: expr, $p: pat, $r: expr) => {
		match $e {
			$p => $r,
			_ => panic!("no match"),
			}
	};
}
impl Val {
	/// Creates `Val::Num` after checking for numeric overflow.
	/// As numbers are `f64`, we can just check for their finity.
	pub fn new_checked_num(num: f64) -> Result<Val> {
		if num.is_finite() {
			Ok(Val::Num(num))
		} else {
			throw!(RuntimeError("overflow".into()))
		}
	}

	pub fn assert_type(&self, context: &'static str, val_type: ValType) -> Result<()> {
		let this_type = self.value_type()?;
		if this_type != val_type {
			throw!(TypeMismatch(context, vec![val_type], this_type))
		} else {
			Ok(())
		}
	}
	pub fn try_cast_bool(self, context: &'static str) -> Result<bool> {
		self.assert_type(context, ValType::Bool)?;
		Ok(matches_unwrap!(self.unwrap_if_lazy()?, Val::Bool(v), v))
	}
	pub fn try_cast_str(self, context: &'static str) -> Result<Rc<str>> {
		self.assert_type(context, ValType::Str)?;
		Ok(matches_unwrap!(self.unwrap_if_lazy()?, Val::Str(v), v))
	}
	pub fn try_cast_num(self, context: &'static str) -> Result<f64> {
		self.assert_type(context, ValType::Num)?;
		Ok(matches_unwrap!(self.unwrap_if_lazy()?, Val::Num(v), v))
	}
	pub fn inplace_unwrap(&mut self) -> Result<()> {
		while let Val::Lazy(lazy) = self {
			*self = lazy.evaluate()?;
		}
		Ok(())
	}
	pub fn unwrap_if_lazy(&self) -> Result<Self> {
		Ok(if let Val::Lazy(v) = self {
			v.evaluate()?.unwrap_if_lazy()?
		} else {
			self.clone()
		})
	}
	pub fn value_type(&self) -> Result<ValType> {
		Ok(match self {
			Val::Str(..) => ValType::Str,
			Val::Num(..) => ValType::Num,
			Val::Arr(..) => ValType::Arr,
			Val::Obj(..) => ValType::Obj,
			Val::Bool(_) => ValType::Bool,
			Val::Null => ValType::Null,
			Val::Func(..) => ValType::Func,
			Val::Lazy(_) => self.clone().unwrap_if_lazy()?.value_type()?,
		})
	}

	pub fn to_string(&self) -> Result<Rc<str>> {
		Ok(match self.unwrap_if_lazy()? {
			Val::Bool(true) => "true".into(),
			Val::Bool(false) => "false".into(),
			Val::Null => "null".into(),
			Val::Str(s) => s,
			v => manifest_json_ex(
				&v,
				&ManifestJsonOptions {
					padding: &"",
					mtype: ManifestType::ToString,
				},
			)?
			.into(),
		})
	}

	/// Expects value to be object, outputs (key, manifested value) pairs
	pub fn manifest_multi(&self, ty: &ManifestFormat) -> Result<Vec<(Rc<str>, Rc<str>)>> {
		let obj = match self {
			Val::Obj(obj) => obj,
			_ => throw!(MultiManifestOutputIsNotAObject),
		};
		let keys = obj.visible_fields();
		let mut out = Vec::with_capacity(keys.len());
		for key in keys {
			let value = obj
				.get(key.clone())?
				.expect("item in object")
				.manifest(ty)?;
			out.push((key, value));
		}
		Ok(out)
	}

	/// Expects value to be array, outputs manifested values
	pub fn manifest_stream(&self, ty: &ManifestFormat) -> Result<Vec<Rc<str>>> {
		let arr = match self {
			Val::Arr(a) => a,
			_ => throw!(StreamManifestOutputIsNotAArray),
		};
		let mut out = Vec::with_capacity(arr.len());
		for i in arr.iter() {
			out.push(i.manifest(ty)?);
		}
		Ok(out)
	}

	pub fn manifest(&self, ty: &ManifestFormat) -> Result<Rc<str>> {
		Ok(match ty {
			ManifestFormat::YamlStream(format) => {
				let arr = match self {
					Val::Arr(a) => a,
					_ => throw!(StreamManifestOutputIsNotAArray),
				};
				let mut out = String::new();

				match format as &ManifestFormat {
					ManifestFormat::YamlStream(_) => throw!(StreamManifestOutputCannotBeRecursed),
					ManifestFormat::String => throw!(StreamManifestCannotNestString),
					_ => {}
				};

				if !arr.is_empty() {
					for v in arr.iter() {
						out.push_str("---\n");
						out.push_str(&v.manifest(format)?);
						out.push_str("\n");
					}
					out.push_str("...");
				}

				out.into()
			}
			ManifestFormat::Yaml(padding) => self.to_yaml(*padding)?,
			ManifestFormat::Json(padding) => self.to_json(*padding)?,
			ManifestFormat::String => match self {
				Val::Str(s) => s.clone(),
				_ => throw!(StringManifestOutputIsNotAString),
			},
		})
	}

	/// For manifestification
	pub fn to_json(&self, padding: usize) -> Result<Rc<str>> {
		manifest_json_ex(
			self,
			&ManifestJsonOptions {
				padding: &" ".repeat(padding),
				mtype: if padding == 0 {
					ManifestType::Minify
				} else {
					ManifestType::Manifest
				},
			},
		)
		.map(|s| s.into())
	}

	/// Calls `std.manifestJson`
	#[cfg(feature = "faster")]
	pub fn to_std_json(&self, padding: usize) -> Result<Rc<str>> {
		manifest_json_ex(
			&self,
			&ManifestJsonOptions {
				padding: &" ".repeat(padding),
				mtype: ManifestType::Std,
			},
		)
		.map(|s| s.into())
	}

	/// Calls `std.manifestJson`
	#[cfg(not(feature = "faster"))]
	pub fn to_std_json(&self, padding: usize) -> Result<Rc<str>> {
		with_state(|s| {
			let ctx = s
				.create_default_context()?
				.with_var("__tmp__to_json__".into(), self.clone())?;
			Ok(evaluate(
				ctx,
				&el!(Expr::Apply(
					el!(Expr::Index(
						el!(Expr::Var("std".into())),
						el!(Expr::Str("manifestJsonEx".into()))
					)),
					ArgsDesc(vec![
						Arg(None, el!(Expr::Var("__tmp__to_json__".into()))),
						Arg(None, el!(Expr::Str(" ".repeat(padding).into())))
					]),
					false
				)),
			)?
			.try_cast_str("to json")?)
		})
	}
	pub fn to_yaml(&self, padding: usize) -> Result<Rc<str>> {
		with_state(|s| {
			let ctx = s
				.create_default_context()?
				.with_var("__tmp__to_json__".into(), self.clone());
			Ok(evaluate(
				ctx,
				&el!(Expr::Apply(
					el!(Expr::Index(
						el!(Expr::Var("std".into())),
						el!(Expr::Str("manifestYamlDoc".into()))
					)),
					ArgsDesc(vec![
						Arg(None, el!(Expr::Var("__tmp__to_json__".into()))),
						Arg(None, el!(Expr::Str(" ".repeat(padding).into())))
					]),
					false
				)),
			)?
			.try_cast_str("to json")?)
		})
	}
}

fn is_function_like(val: &Val) -> bool {
	matches!(val, Val::Func(_))
}

/// Native implementation of `std.primitiveEquals`
pub fn primitive_equals(val_a: &Val, val_b: &Val) -> Result<bool> {
	Ok(match (val_a.unwrap_if_lazy()?, val_b.unwrap_if_lazy()?) {
		(Val::Bool(a), Val::Bool(b)) => a == b,
		(Val::Null, Val::Null) => true,
		(Val::Str(a), Val::Str(b)) => a == b,
		(Val::Num(a), Val::Num(b)) => (a - b).abs() <= f64::EPSILON,
		(Val::Arr(_), Val::Arr(_)) => throw!(RuntimeError(
			"primitiveEquals operates on primitive types, got array".into(),
		)),
		(Val::Obj(_), Val::Obj(_)) => throw!(RuntimeError(
			"primitiveEquals operates on primitive types, got object".into(),
		)),
		(a, b) if is_function_like(&a) && is_function_like(&b) => {
			throw!(RuntimeError("cannot test equality of functions".into()))
		}
		(_, _) => false,
	})
}

/// Native implementation of `std.equals`
pub fn equals(val_a: &Val, val_b: &Val) -> Result<bool> {
	let val_a = val_a.unwrap_if_lazy()?;
	let val_b = val_b.unwrap_if_lazy()?;

	if val_a.value_type()? != val_b.value_type()? {
		return Ok(false);
	}
	match (val_a, val_b) {
		// Cant test for ptr equality, because all fields needs to be evaluated
		(Val::Arr(a), Val::Arr(b)) => {
			if a.len() != b.len() {
				return Ok(false);
			}
			for (a, b) in a.iter().zip(b.iter()) {
				if !equals(&a.unwrap_if_lazy()?, &b.unwrap_if_lazy()?)? {
					return Ok(false);
				}
			}
			Ok(true)
		}
		(Val::Obj(a), Val::Obj(b)) => {
			let fields = a.visible_fields();
			if fields != b.visible_fields() {
				return Ok(false);
			}
			for field in fields {
				if !equals(&a.get(field.clone())?.unwrap(), &b.get(field)?.unwrap())? {
					return Ok(false);
				}
			}
			Ok(true)
		}
		(a, b) => Ok(primitive_equals(&a, &b)?),
	}
}
