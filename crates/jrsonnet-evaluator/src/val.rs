use crate::{
	builtin::{
		call_builtin,
		manifest::{manifest_json_ex, ManifestJsonOptions, ManifestType},
	},
	error::{Error::*, LocError},
	evaluate,
	function::{parse_function_call, parse_function_call_map, place_args},
	native::NativeCallback,
	throw, with_state, Context, ObjValue, Result,
};
use jrsonnet_gc::{Gc, GcCell, Trace};
use jrsonnet_interner::IStr;
use jrsonnet_parser::{el, Arg, ArgsDesc, Expr, ExprLocation, LiteralType, LocExpr, ParamsDesc};
use jrsonnet_types::ValType;
use std::{collections::HashMap, fmt::Debug, rc::Rc};

pub trait LazyValValue: Trace {
	fn get(self: Box<Self>) -> Result<Val>;
}

#[derive(Trace)]
#[trivially_drop]
enum LazyValInternals {
	Computed(Val),
	Errored(LocError),
	Waiting(Box<dyn LazyValValue>),
	Pending,
}

#[derive(Clone, Trace)]
#[trivially_drop]
pub struct LazyVal(Gc<GcCell<LazyValInternals>>);
impl LazyVal {
	pub fn new(f: Box<dyn LazyValValue>) -> Self {
		Self(Gc::new(GcCell::new(LazyValInternals::Waiting(f))))
	}
	pub fn new_resolved(val: Val) -> Self {
		Self(Gc::new(GcCell::new(LazyValInternals::Computed(val))))
	}
	pub fn evaluate(&self) -> Result<Val> {
		match &*self.0.borrow() {
			LazyValInternals::Computed(v) => return Ok(v.clone()),
			LazyValInternals::Errored(e) => return Err(e.clone()),
			LazyValInternals::Pending => return Err(RecursiveLazyValueEvaluation.into()),
			_ => (),
		};
		let value = if let LazyValInternals::Waiting(value) =
			std::mem::replace(&mut *self.0.borrow_mut(), LazyValInternals::Pending)
		{
			value
		} else {
			unreachable!()
		};
		let new_value = match value.get() {
			Ok(v) => v,
			Err(e) => {
				*self.0.borrow_mut() = LazyValInternals::Errored(e.clone());
				return Err(e);
			}
		};
		*self.0.borrow_mut() = LazyValInternals::Computed(new_value.clone());
		Ok(new_value)
	}
}

impl Debug for LazyVal {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Lazy")
	}
}
impl PartialEq for LazyVal {
	fn eq(&self, other: &Self) -> bool {
		Gc::ptr_eq(&self.0, &other.0)
	}
}

#[derive(Debug, PartialEq, Trace)]
#[trivially_drop]
pub struct FuncDesc {
	pub name: IStr,
	pub ctx: Context,
	pub params: ParamsDesc,
	pub body: LocExpr,
}

#[derive(Debug, Trace)]
#[trivially_drop]
pub enum FuncVal {
	/// Plain function implemented in jsonnet
	Normal(FuncDesc),
	/// Standard library function
	Intrinsic(IStr),
	/// Library functions implemented in native
	NativeExt(IStr, Gc<NativeCallback>),
}

impl PartialEq for FuncVal {
	fn eq(&self, other: &Self) -> bool {
		match (self, other) {
			(Self::Normal(a), Self::Normal(b)) => a == b,
			(Self::Intrinsic(an), Self::Intrinsic(bn)) => an == bn,
			(Self::NativeExt(an, _), Self::NativeExt(bn, _)) => an == bn,
			(..) => false,
		}
	}
}
impl FuncVal {
	pub fn is_ident(&self) -> bool {
		matches!(&self, Self::Intrinsic(n) if n as &str == "id")
	}
	pub fn name(&self) -> IStr {
		match self {
			Self::Normal(normal) => normal.name.clone(),
			Self::Intrinsic(name) => format!("std.{}", name).into(),
			Self::NativeExt(n, _) => format!("native.{}", n).into(),
		}
	}
	pub fn evaluate(
		&self,
		call_ctx: Context,
		loc: Option<&ExprLocation>,
		args: &ArgsDesc,
		tailstrict: bool,
	) -> Result<Val> {
		match self {
			Self::Normal(func) => {
				let ctx = parse_function_call(
					call_ctx,
					Some(func.ctx.clone()),
					&func.params,
					args,
					tailstrict,
				)?;
				evaluate(ctx, &func.body)
			}
			Self::Intrinsic(name) => call_builtin(call_ctx, loc, name, args),
			Self::NativeExt(_name, handler) => {
				let args = parse_function_call(call_ctx, None, &handler.params, args, true)?;
				let mut out_args = Vec::with_capacity(handler.params.len());
				for p in handler.params.0.iter() {
					out_args.push(args.binding(p.0.clone())?.evaluate()?);
				}
				Ok(handler.call(loc.map(|l| l.0.clone()), &out_args)?)
			}
		}
	}

	pub fn evaluate_map(
		&self,
		call_ctx: Context,
		args: &HashMap<IStr, Val>,
		tailstrict: bool,
	) -> Result<Val> {
		match self {
			Self::Normal(func) => {
				let ctx = parse_function_call_map(
					call_ctx,
					Some(func.ctx.clone()),
					&func.params,
					args,
					tailstrict,
				)?;
				evaluate(ctx, &func.body)
			}
			Self::Intrinsic(_) => todo!(),
			Self::NativeExt(_, _) => todo!(),
		}
	}

	pub fn evaluate_values(&self, call_ctx: Context, args: &[Val]) -> Result<Val> {
		match self {
			Self::Normal(func) => {
				let ctx = place_args(call_ctx, Some(func.ctx.clone()), &func.params, args)?;
				evaluate(ctx, &func.body)
			}
			Self::Intrinsic(_) => todo!(),
			Self::NativeExt(_, _) => todo!(),
		}
	}
}

#[derive(Clone)]
pub enum ManifestFormat {
	YamlStream(Box<ManifestFormat>),
	Yaml(usize),
	Json(usize),
	ToString,
	String,
}

#[derive(Debug, Clone, Trace)]
#[trivially_drop]
pub enum ArrValue {
	Lazy(Gc<Vec<LazyVal>>),
	Eager(Gc<Vec<Val>>),
	Extended(Box<(Self, Self)>),
}
impl ArrValue {
	pub fn new_eager() -> Self {
		Self::Eager(Gc::new(Vec::new()))
	}

	pub fn len(&self) -> usize {
		match self {
			Self::Lazy(l) => l.len(),
			Self::Eager(e) => e.len(),
			Self::Extended(v) => v.0.len() + v.1.len(),
		}
	}

	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	pub fn get(&self, index: usize) -> Result<Option<Val>> {
		match self {
			Self::Lazy(vec) => {
				if let Some(v) = vec.get(index) {
					Ok(Some(v.evaluate()?))
				} else {
					Ok(None)
				}
			}
			Self::Eager(vec) => Ok(vec.get(index).cloned()),
			Self::Extended(v) => {
				let a_len = v.0.len();
				if a_len > index {
					v.0.get(index)
				} else {
					v.1.get(index - a_len)
				}
			}
		}
	}

	pub fn get_lazy(&self, index: usize) -> Option<LazyVal> {
		match self {
			Self::Lazy(vec) => vec.get(index).cloned(),
			Self::Eager(vec) => vec.get(index).cloned().map(LazyVal::new_resolved),
			Self::Extended(v) => {
				let a_len = v.0.len();
				if a_len > index {
					v.0.get_lazy(index)
				} else {
					v.1.get_lazy(index - a_len)
				}
			}
		}
	}

	pub fn evaluated(&self) -> Result<Gc<Vec<Val>>> {
		Ok(match self {
			Self::Lazy(vec) => {
				let mut out = Vec::with_capacity(vec.len());
				for item in vec.iter() {
					out.push(item.evaluate()?);
				}
				Gc::new(out)
			}
			Self::Eager(vec) => vec.clone(),
			Self::Extended(_v) => {
				let mut out = Vec::with_capacity(self.len());
				for item in self.iter() {
					out.push(item?);
				}
				Gc::new(out)
			}
		})
	}

	pub fn iter(&self) -> impl DoubleEndedIterator<Item = Result<Val>> + '_ {
		(0..self.len()).map(move |idx| match self {
			Self::Lazy(l) => l[idx].evaluate(),
			Self::Eager(e) => Ok(e[idx].clone()),
			Self::Extended(_) => self.get(idx).map(|e| e.unwrap()),
		})
	}

	pub fn iter_lazy(&self) -> impl DoubleEndedIterator<Item = LazyVal> + '_ {
		(0..self.len()).map(move |idx| match self {
			Self::Lazy(l) => l[idx].clone(),
			Self::Eager(e) => LazyVal::new_resolved(e[idx].clone()),
			Self::Extended(_) => self.get_lazy(idx).unwrap(),
		})
	}

	pub fn reversed(self) -> Self {
		match self {
			Self::Lazy(vec) => {
				let mut out = (&vec as &Vec<_>).clone();
				out.reverse();
				Self::Lazy(Gc::new(out))
			}
			Self::Eager(vec) => {
				let mut out = (&vec as &Vec<_>).clone();
				out.reverse();
				Self::Eager(Gc::new(out))
			}
			Self::Extended(b) => Self::Extended(Box::new((b.1.reversed(), b.0.reversed()))),
		}
	}

	pub fn map(self, mapper: impl Fn(Val) -> Result<Val>) -> Result<Self> {
		let mut out = Vec::with_capacity(self.len());

		for value in self.iter() {
			out.push(mapper(value?)?);
		}

		Ok(Self::Eager(Gc::new(out)))
	}

	pub fn filter(self, filter: impl Fn(&Val) -> Result<bool>) -> Result<Self> {
		let mut out = Vec::with_capacity(self.len());

		for value in self.iter() {
			let value = value?;
			if filter(&value)? {
				out.push(value);
			}
		}

		Ok(Self::Eager(Gc::new(out)))
	}

	pub fn ptr_eq(a: &Self, b: &Self) -> bool {
		match (a, b) {
			(Self::Lazy(a), Self::Lazy(b)) => Gc::ptr_eq(a, b),
			(Self::Eager(a), Self::Eager(b)) => Gc::ptr_eq(a, b),
			_ => false,
		}
	}
}

impl From<Vec<LazyVal>> for ArrValue {
	fn from(v: Vec<LazyVal>) -> Self {
		Self::Lazy(Gc::new(v))
	}
}

impl From<Vec<Val>> for ArrValue {
	fn from(v: Vec<Val>) -> Self {
		Self::Eager(Gc::new(v))
	}
}

pub enum IndexableVal {
	Str(IStr),
	Arr(ArrValue),
}

#[derive(Debug, Clone, Trace)]
#[trivially_drop]
pub enum Val {
	Bool(bool),
	Null,
	Str(IStr),
	Num(f64),
	Arr(ArrValue),
	Obj(ObjValue),
	Func(Gc<FuncVal>),
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
	pub fn new_checked_num(num: f64) -> Result<Self> {
		if num.is_finite() {
			Ok(Self::Num(num))
		} else {
			throw!(RuntimeError("overflow".into()))
		}
	}

	pub fn assert_type(&self, context: &'static str, val_type: ValType) -> Result<()> {
		let this_type = self.value_type();
		if this_type != val_type {
			throw!(TypeMismatch(context, vec![val_type], this_type))
		} else {
			Ok(())
		}
	}
	pub fn unwrap_num(self) -> Result<f64> {
		Ok(matches_unwrap!(self, Self::Num(v), v))
	}
	pub fn unwrap_func(self) -> Result<Gc<FuncVal>> {
		Ok(matches_unwrap!(self, Self::Func(v), v))
	}
	pub fn try_cast_bool(self, context: &'static str) -> Result<bool> {
		self.assert_type(context, ValType::Bool)?;
		Ok(matches_unwrap!(self, Self::Bool(v), v))
	}
	pub fn try_cast_str(self, context: &'static str) -> Result<IStr> {
		self.assert_type(context, ValType::Str)?;
		Ok(matches_unwrap!(self, Self::Str(v), v))
	}
	pub fn try_cast_num(self, context: &'static str) -> Result<f64> {
		self.assert_type(context, ValType::Num)?;
		self.unwrap_num()
	}
	pub fn try_cast_nullable_num(self, context: &'static str) -> Result<Option<f64>> {
		Ok(match self {
			Val::Null => None,
			Val::Num(num) => Some(num),
			_ => throw!(TypeMismatch(
				context,
				vec![ValType::Null, ValType::Num],
				self.value_type()
			)),
		})
	}
	pub const fn value_type(&self) -> ValType {
		match self {
			Self::Str(..) => ValType::Str,
			Self::Num(..) => ValType::Num,
			Self::Arr(..) => ValType::Arr,
			Self::Obj(..) => ValType::Obj,
			Self::Bool(_) => ValType::Bool,
			Self::Null => ValType::Null,
			Self::Func(..) => ValType::Func,
		}
	}

	pub fn to_string(&self) -> Result<IStr> {
		Ok(match self {
			Self::Bool(true) => "true".into(),
			Self::Bool(false) => "false".into(),
			Self::Null => "null".into(),
			Self::Str(s) => s.clone(),
			v => manifest_json_ex(
				v,
				&ManifestJsonOptions {
					padding: "",
					mtype: ManifestType::ToString,
				},
			)?
			.into(),
		})
	}

	/// Expects value to be object, outputs (key, manifested value) pairs
	pub fn manifest_multi(&self, ty: &ManifestFormat) -> Result<Vec<(IStr, IStr)>> {
		let obj = match self {
			Self::Obj(obj) => obj,
			_ => throw!(MultiManifestOutputIsNotAObject),
		};
		let keys = obj.fields();
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
	pub fn manifest_stream(&self, ty: &ManifestFormat) -> Result<Vec<IStr>> {
		let arr = match self {
			Self::Arr(a) => a,
			_ => throw!(StreamManifestOutputIsNotAArray),
		};
		let mut out = Vec::with_capacity(arr.len());
		for i in arr.iter() {
			out.push(i?.manifest(ty)?);
		}
		Ok(out)
	}

	pub fn manifest(&self, ty: &ManifestFormat) -> Result<IStr> {
		Ok(match ty {
			ManifestFormat::YamlStream(format) => {
				let arr = match self {
					Self::Arr(a) => a,
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
						out.push_str(&v?.manifest(format)?);
						out.push('\n');
					}
					out.push_str("...");
				}

				out.into()
			}
			ManifestFormat::Yaml(padding) => self.to_yaml(*padding)?,
			ManifestFormat::Json(padding) => self.to_json(*padding)?,
			ManifestFormat::ToString => self.to_string()?,
			ManifestFormat::String => match self {
				Self::Str(s) => s.clone(),
				_ => throw!(StringManifestOutputIsNotAString),
			},
		})
	}

	/// For manifestification
	pub fn to_json(&self, padding: usize) -> Result<IStr> {
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
	pub fn to_std_json(&self, padding: usize) -> Result<Rc<str>> {
		manifest_json_ex(
			self,
			&ManifestJsonOptions {
				padding: &" ".repeat(padding),
				mtype: ManifestType::Std,
			},
		)
		.map(|s| s.into())
	}

	pub fn to_yaml(&self, padding: usize) -> Result<IStr> {
		with_state(|s| {
			let ctx = s
				.create_default_context()
				.with_var("__tmp__to_json__".into(), self.clone());
			evaluate(
				ctx,
				&el!(Expr::Apply(
					el!(Expr::Index(
						el!(Expr::Var("std".into())),
						el!(Expr::Str("manifestYamlDoc".into()))
					)),
					ArgsDesc(vec![
						Arg(None, el!(Expr::Var("__tmp__to_json__".into()))),
						Arg(
							None,
							el!(Expr::Literal(if padding != 0 {
								LiteralType::True
							} else {
								LiteralType::False
							}))
						)
					]),
					false
				)),
			)?
			.try_cast_str("to json")
		})
	}
	pub fn into_indexable(self) -> Result<IndexableVal> {
		Ok(match self {
			Val::Str(s) => IndexableVal::Str(s),
			Val::Arr(arr) => IndexableVal::Arr(arr),
			_ => throw!(ValueIsNotIndexable(self.value_type())),
		})
	}
}

const fn is_function_like(val: &Val) -> bool {
	matches!(val, Val::Func(_))
}

/// Native implementation of `std.primitiveEquals`
pub fn primitive_equals(val_a: &Val, val_b: &Val) -> Result<bool> {
	Ok(match (val_a, val_b) {
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
		(a, b) if is_function_like(a) && is_function_like(b) => {
			throw!(RuntimeError("cannot test equality of functions".into()))
		}
		(_, _) => false,
	})
}

/// Native implementation of `std.equals`
pub fn equals(val_a: &Val, val_b: &Val) -> Result<bool> {
	if val_a.value_type() != val_b.value_type() {
		return Ok(false);
	}
	match (val_a, val_b) {
		(Val::Arr(a), Val::Arr(b)) => {
			if ArrValue::ptr_eq(a, b) {
				return Ok(true);
			}
			if a.len() != b.len() {
				return Ok(false);
			}
			for (a, b) in a.iter().zip(b.iter()) {
				if !equals(&a?, &b?)? {
					return Ok(false);
				}
			}
			Ok(true)
		}
		(Val::Obj(a), Val::Obj(b)) => {
			if ObjValue::ptr_eq(a, b) {
				return Ok(true);
			}
			let fields = a.fields();
			if fields != b.fields() {
				return Ok(false);
			}
			for field in fields {
				if !equals(&a.get(field.clone())?.unwrap(), &b.get(field)?.unwrap())? {
					return Ok(false);
				}
			}
			Ok(true)
		}
		(a, b) => Ok(primitive_equals(a, b)?),
	}
}
