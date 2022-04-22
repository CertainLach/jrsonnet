use std::{cell::RefCell, fmt::Debug, rc::Rc};

use gcmodule::{Cc, Trace};
use jrsonnet_interner::IStr;
use jrsonnet_parser::{LocExpr, ParamsDesc};
use jrsonnet_types::ValType;

use crate::{
	builtin::manifest::{
		manifest_json_ex, manifest_yaml_ex, ManifestJsonOptions, ManifestType, ManifestYamlOptions,
	},
	cc_ptr_eq,
	error::{Error::*, LocError},
	evaluate,
	function::{
		parse_default_function_call, parse_function_call, ArgsLike, Builtin, CallLocation,
		StaticBuiltin,
	},
	gc::TraceBox,
	throw, Context, ObjValue, Result, State,
};

pub trait LazyValValue: Trace {
	fn get(self: Box<Self>, s: State) -> Result<Val>;
}

#[derive(Trace)]
enum LazyValInternals {
	Computed(Val),
	Errored(LocError),
	Waiting(TraceBox<dyn LazyValValue>),
	Pending,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Trace)]
pub struct LazyVal(Cc<RefCell<LazyValInternals>>);
impl LazyVal {
	pub fn new(f: TraceBox<dyn LazyValValue>) -> Self {
		Self(Cc::new(RefCell::new(LazyValInternals::Waiting(f))))
	}
	pub fn new_resolved(val: Val) -> Self {
		Self(Cc::new(RefCell::new(LazyValInternals::Computed(val))))
	}
	pub fn force(&self, s: State) -> Result<()> {
		self.evaluate(s)?;
		Ok(())
	}
	pub fn evaluate(&self, s: State) -> Result<Val> {
		match &*self.0.borrow() {
			LazyValInternals::Computed(v) => return Ok(v.clone()),
			LazyValInternals::Errored(e) => return Err(e.clone()),
			LazyValInternals::Pending => return Err(InfiniteRecursionDetected.into()),
			LazyValInternals::Waiting(..) => (),
		};
		let value = if let LazyValInternals::Waiting(value) =
			std::mem::replace(&mut *self.0.borrow_mut(), LazyValInternals::Pending)
		{
			value
		} else {
			unreachable!()
		};
		let new_value = match value.0.get(s) {
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
		cc_ptr_eq(&self.0, &other.0)
	}
}

#[derive(Debug, PartialEq, Trace)]
pub struct FuncDesc {
	pub name: IStr,
	pub ctx: Context,
	pub params: ParamsDesc,
	pub body: LocExpr,
}
impl FuncDesc {
	/// Create body context, but fill arguments without defaults with lazy error
	pub fn default_body_context(&self) -> Context {
		parse_default_function_call(self.ctx.clone(), &self.params)
	}

	/// Create context, with which body code will run
	pub fn call_body_context(
		&self,
		s: State,
		call_ctx: Context,
		args: &dyn ArgsLike,
		tailstrict: bool,
	) -> Result<Context> {
		parse_function_call(
			s,
			call_ctx,
			self.ctx.clone(),
			&self.params,
			args,
			tailstrict,
		)
	}
}

#[allow(clippy::module_name_repetitions)]
#[derive(Trace, Clone)]
pub enum FuncVal {
	/// Plain function implemented in jsonnet
	Normal(Cc<FuncDesc>),
	/// Standard library function
	StaticBuiltin(#[skip_trace] &'static dyn StaticBuiltin),
	/// User-provided function
	Builtin(Cc<TraceBox<dyn Builtin>>),
}

impl Debug for FuncVal {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Normal(arg0) => f.debug_tuple("Normal").field(arg0).finish(),
			Self::StaticBuiltin(arg0) => {
				f.debug_tuple("StaticBuiltin").field(&arg0.name()).finish()
			}
			Self::Builtin(arg0) => f.debug_tuple("Builtin").field(&arg0.name()).finish(),
		}
	}
}

impl FuncVal {
	pub fn args_len(&self) -> usize {
		match self {
			Self::Normal(n) => n.params.iter().filter(|p| p.1.is_none()).count(),
			Self::StaticBuiltin(i) => i.params().iter().filter(|p| !p.has_default).count(),
			Self::Builtin(i) => i.params().iter().filter(|p| !p.has_default).count(),
		}
	}
	pub fn name(&self) -> IStr {
		match self {
			Self::Normal(normal) => normal.name.clone(),
			Self::StaticBuiltin(builtin) => builtin.name().into(),
			Self::Builtin(builtin) => builtin.name().into(),
		}
	}
	pub fn evaluate(
		&self,
		s: State,
		call_ctx: Context,
		loc: CallLocation,
		args: &dyn ArgsLike,
		tailstrict: bool,
	) -> Result<Val> {
		match self {
			Self::Normal(func) => {
				let body_ctx = func.call_body_context(s.clone(), call_ctx, args, tailstrict)?;
				evaluate(s, body_ctx, &func.body)
			}
			Self::StaticBuiltin(b) => b.call(s, call_ctx, loc, args),
			Self::Builtin(b) => b.call(s, call_ctx, loc, args),
		}
	}
	pub fn evaluate_simple(&self, s: State, args: &dyn ArgsLike) -> Result<Val> {
		self.evaluate(s, Context::default(), CallLocation::native(), args, true)
	}
}

#[derive(Clone)]
pub enum ManifestFormat {
	YamlStream(Box<ManifestFormat>),
	Yaml {
		padding: usize,
		#[cfg(feature = "exp-preserve-order")]
		preserve_order: bool,
	},
	Json {
		padding: usize,
		#[cfg(feature = "exp-preserve-order")]
		preserve_order: bool,
	},
	ToString,
	String,
}
impl ManifestFormat {
	#[cfg(feature = "exp-preserve-order")]
	fn preserve_order(&self) -> bool {
		match self {
			ManifestFormat::YamlStream(s) => s.preserve_order(),
			ManifestFormat::Yaml { preserve_order, .. } => *preserve_order,
			ManifestFormat::Json { preserve_order, .. } => *preserve_order,
			ManifestFormat::ToString => false,
			ManifestFormat::String => false,
		}
	}
}

#[derive(Debug, Clone, Trace)]
pub struct Slice {
	pub(crate) inner: ArrValue,
	pub(crate) from: u32,
	pub(crate) to: u32,
	pub(crate) step: u32,
}
impl Slice {
	const fn from(&self) -> usize {
		self.from as usize
	}
	const fn to(&self) -> usize {
		self.to as usize
	}
	const fn step(&self) -> usize {
		self.step as usize
	}
	const fn len(&self) -> usize {
		// TODO: use div_ceil
		let diff = self.to() - self.from();
		let rem = diff % self.step();
		let div = diff / self.step();

		if rem == 0 {
			div
		} else {
			div + 1
		}
	}
}

#[derive(Debug, Clone, Trace)]
#[force_tracking]
pub enum ArrValue {
	Bytes(#[skip_trace] Rc<[u8]>),
	Lazy(Cc<Vec<LazyVal>>),
	Eager(Cc<Vec<Val>>),
	Extended(Box<(Self, Self)>),
	Range(i32, i32),
	Slice(Box<Slice>),
	Reversed(Box<Self>),
}
impl ArrValue {
	pub fn new_eager() -> Self {
		Self::Eager(Cc::new(Vec::new()))
	}

	/// # Panics
	/// If a > b
	pub fn new_range(a: i32, b: i32) -> Self {
		assert!(a <= b);
		Self::Range(a, b)
	}

	/// # Panics
	/// If passed numbers are incorrect
	#[must_use]
	pub fn slice(self, from: Option<usize>, to: Option<usize>, step: Option<usize>) -> Self {
		let len = self.len();
		let from = from.unwrap_or(0);
		let to = to.unwrap_or(len).min(len);
		let step = step.unwrap_or(1);
		assert!(from < to);
		assert!(step > 0);

		Self::Slice(Box::new(Slice {
			inner: self,
			from: from as u32,
			to: to as u32,
			step: step as u32,
		}))
	}

	pub fn len(&self) -> usize {
		match self {
			Self::Bytes(i) => i.len(),
			Self::Lazy(l) => l.len(),
			Self::Eager(e) => e.len(),
			Self::Extended(v) => v.0.len() + v.1.len(),
			Self::Range(a, b) => a.abs_diff(*b) as usize + 1,
			Self::Reversed(i) => i.len(),
			Self::Slice(s) => s.len(),
		}
	}

	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	pub fn get(&self, s: State, index: usize) -> Result<Option<Val>> {
		match self {
			Self::Bytes(i) => i
				.get(index)
				.map_or(Ok(None), |v| Ok(Some(Val::Num(f64::from(*v))))),
			Self::Lazy(vec) => {
				if let Some(v) = vec.get(index) {
					Ok(Some(v.evaluate(s)?))
				} else {
					Ok(None)
				}
			}
			Self::Eager(vec) => Ok(vec.get(index).cloned()),
			Self::Extended(v) => {
				let a_len = v.0.len();
				if a_len > index {
					v.0.get(s, index)
				} else {
					v.1.get(s, index - a_len)
				}
			}
			Self::Range(a, _) => {
				if index >= self.len() {
					return Ok(None);
				}
				Ok(Some(Val::Num(((*a as isize) + index as isize) as f64)))
			}
			Self::Reversed(v) => {
				let len = v.len();
				if index >= len {
					return Ok(None);
				}
				v.get(s, len - index - 1)
			}
			Self::Slice(v) => {
				let index = v.from() + index * v.step();
				if index >= v.to() {
					return Ok(None);
				}
				v.inner.get(s, index as usize)
			}
		}
	}

	pub fn get_lazy(&self, index: usize) -> Option<LazyVal> {
		match self {
			Self::Bytes(i) => i
				.get(index)
				.map(|b| LazyVal::new_resolved(Val::Num(f64::from(*b)))),
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
			Self::Range(a, _) => {
				if index >= self.len() {
					return None;
				}
				Some(LazyVal::new_resolved(Val::Num(
					((*a as isize) + index as isize) as f64,
				)))
			}
			Self::Reversed(v) => {
				let len = v.len();
				if index >= len {
					return None;
				}
				v.get_lazy(len - index - 1)
			}
			Self::Slice(s) => {
				let index = s.from() + index * s.step();
				if index >= s.to() {
					return None;
				}
				s.inner.get_lazy(index as usize)
			}
		}
	}

	pub fn evaluated(&self, s: State) -> Result<Cc<Vec<Val>>> {
		Ok(match self {
			Self::Bytes(i) => {
				let mut out = Vec::with_capacity(i.len());
				for v in i.iter() {
					out.push(Val::Num(f64::from(*v)));
				}
				Cc::new(out)
			}
			Self::Lazy(vec) => {
				let mut out = Vec::with_capacity(vec.len());
				for item in vec.iter() {
					out.push(item.evaluate(s.clone())?);
				}
				Cc::new(out)
			}
			Self::Eager(vec) => vec.clone(),
			Self::Extended(_v) => {
				let mut out = Vec::with_capacity(self.len());
				for item in self.iter(s) {
					out.push(item?);
				}
				Cc::new(out)
			}
			Self::Range(a, b) => {
				let mut out = Vec::with_capacity(self.len());
				for i in *a..*b {
					out.push(Val::Num(f64::from(i)));
				}
				Cc::new(out)
			}
			Self::Reversed(r) => {
				let mut r = r.evaluated(s)?;
				Cc::update_with(&mut r, |v| v.reverse());
				r
			}
			Self::Slice(v) => {
				let mut out = Vec::with_capacity(v.inner.len());
				for v in v
					.inner
					.iter_lazy()
					.skip(v.from())
					.take(v.to() - v.from())
					.step_by(v.step())
				{
					out.push(v.evaluate(s.clone())?);
				}
				Cc::new(out)
			}
		})
	}

	pub fn iter(&self, s: State) -> impl DoubleEndedIterator<Item = Result<Val>> + '_ {
		(0..self.len()).map(move |idx| match self {
			Self::Bytes(b) => Ok(Val::Num(f64::from(b[idx]))),
			Self::Lazy(l) => l[idx].evaluate(s.clone()),
			Self::Eager(e) => Ok(e[idx].clone()),
			Self::Extended(..) | Self::Range(..) | Self::Reversed(..) | Self::Slice(..) => {
				self.get(s.clone(), idx).map(|e| e.expect("idx < len"))
			}
		})
	}

	pub fn iter_lazy(&self) -> impl DoubleEndedIterator<Item = LazyVal> + '_ {
		(0..self.len()).map(move |idx| match self {
			Self::Bytes(b) => LazyVal::new_resolved(Val::Num(f64::from(b[idx]))),
			Self::Lazy(l) => l[idx].clone(),
			Self::Eager(e) => LazyVal::new_resolved(e[idx].clone()),
			Self::Slice(..) | Self::Extended(..) | Self::Range(..) | Self::Reversed(..) => {
				self.get_lazy(idx).expect("idx < len")
			}
		})
	}

	#[must_use]
	pub fn reversed(self) -> Self {
		Self::Reversed(Box::new(self))
	}

	pub fn map(self, s: State, mapper: impl Fn(Val) -> Result<Val>) -> Result<Self> {
		let mut out = Vec::with_capacity(self.len());

		for value in self.iter(s) {
			out.push(mapper(value?)?);
		}

		Ok(Self::Eager(Cc::new(out)))
	}

	pub fn filter(self, s: State, filter: impl Fn(&Val) -> Result<bool>) -> Result<Self> {
		let mut out = Vec::with_capacity(self.len());

		for value in self.iter(s) {
			let value = value?;
			if filter(&value)? {
				out.push(value);
			}
		}

		Ok(Self::Eager(Cc::new(out)))
	}

	pub fn ptr_eq(a: &Self, b: &Self) -> bool {
		match (a, b) {
			(Self::Lazy(a), Self::Lazy(b)) => cc_ptr_eq(a, b),
			(Self::Eager(a), Self::Eager(b)) => cc_ptr_eq(a, b),
			_ => false,
		}
	}
}

impl From<Vec<LazyVal>> for ArrValue {
	fn from(v: Vec<LazyVal>) -> Self {
		Self::Lazy(Cc::new(v))
	}
}

impl From<Vec<Val>> for ArrValue {
	fn from(v: Vec<Val>) -> Self {
		Self::Eager(Cc::new(v))
	}
}

#[allow(clippy::module_name_repetitions)]
pub enum IndexableVal {
	Str(IStr),
	Arr(ArrValue),
}

#[derive(Debug, Clone, Trace)]
pub enum Val {
	Bool(bool),
	Null,
	Str(IStr),
	Num(f64),
	Arr(ArrValue),
	Obj(ObjValue),
	Func(FuncVal),
}

impl Val {
	pub const fn as_bool(&self) -> Option<bool> {
		match self {
			Val::Bool(v) => Some(*v),
			_ => None,
		}
	}
	pub const fn as_null(&self) -> Option<()> {
		match self {
			Val::Null => Some(()),
			_ => None,
		}
	}
	pub fn as_str(&self) -> Option<IStr> {
		match self {
			Val::Str(s) => Some(s.clone()),
			_ => None,
		}
	}
	pub const fn as_num(&self) -> Option<f64> {
		match self {
			Val::Num(n) => Some(*n),
			_ => None,
		}
	}
	pub fn as_arr(&self) -> Option<ArrValue> {
		match self {
			Val::Arr(a) => Some(a.clone()),
			_ => None,
		}
	}
	pub fn as_obj(&self) -> Option<ObjValue> {
		match self {
			Val::Obj(o) => Some(o.clone()),
			_ => None,
		}
	}
	pub fn as_func(&self) -> Option<FuncVal> {
		match self {
			Val::Func(f) => Some(f.clone()),
			_ => None,
		}
	}

	/// Creates `Val::Num` after checking for numeric overflow.
	/// As numbers are `f64`, we can just check for their finity.
	pub fn new_checked_num(num: f64) -> Result<Self> {
		if num.is_finite() {
			Ok(Self::Num(num))
		} else {
			throw!(RuntimeError("overflow".into()))
		}
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

	pub fn to_string(&self, s: State) -> Result<IStr> {
		Ok(match self {
			Self::Bool(true) => "true".into(),
			Self::Bool(false) => "false".into(),
			Self::Null => "null".into(),
			Self::Str(s) => s.clone(),
			v => manifest_json_ex(
				s,
				v,
				&ManifestJsonOptions {
					padding: "",
					mtype: ManifestType::ToString,
					newline: "\n",
					key_val_sep: ": ",
					#[cfg(feature = "exp-preserve-order")]
					preserve_order: false,
				},
			)?
			.into(),
		})
	}

	/// Expects value to be object, outputs (key, manifested value) pairs
	pub fn manifest_multi(&self, s: State, ty: &ManifestFormat) -> Result<Vec<(IStr, IStr)>> {
		let obj = match self {
			Self::Obj(obj) => obj,
			_ => throw!(MultiManifestOutputIsNotAObject),
		};
		let keys = obj.fields(
			#[cfg(feature = "exp-preserve-order")]
			ty.preserve_order(),
		);
		let mut out = Vec::with_capacity(keys.len());
		for key in keys {
			let value = obj
				.get(s.clone(), key.clone())?
				.expect("item in object")
				.manifest(s.clone(), ty)?;
			out.push((key, value));
		}
		Ok(out)
	}

	/// Expects value to be array, outputs manifested values
	pub fn manifest_stream(&self, s: State, ty: &ManifestFormat) -> Result<Vec<IStr>> {
		let arr = match self {
			Self::Arr(a) => a,
			_ => throw!(StreamManifestOutputIsNotAArray),
		};
		let mut out = Vec::with_capacity(arr.len());
		for i in arr.iter(s.clone()) {
			out.push(i?.manifest(s.clone(), ty)?);
		}
		Ok(out)
	}

	pub fn manifest(&self, s: State, ty: &ManifestFormat) -> Result<IStr> {
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
					for v in arr.iter(s.clone()) {
						out.push_str("---\n");
						out.push_str(&v?.manifest(s.clone(), format)?);
						out.push('\n');
					}
					out.push_str("...");
				}

				out.into()
			}
			ManifestFormat::Yaml {
				padding,
				#[cfg(feature = "exp-preserve-order")]
				preserve_order,
			} => self.to_yaml(
				s,
				*padding,
				#[cfg(feature = "exp-preserve-order")]
				*preserve_order,
			)?,
			ManifestFormat::Json {
				padding,
				#[cfg(feature = "exp-preserve-order")]
				preserve_order,
			} => self.to_json(
				s,
				*padding,
				#[cfg(feature = "exp-preserve-order")]
				*preserve_order,
			)?,
			ManifestFormat::ToString => self.to_string(s)?,
			ManifestFormat::String => match self {
				Self::Str(s) => s.clone(),
				_ => throw!(StringManifestOutputIsNotAString),
			},
		})
	}

	/// For manifestification
	pub fn to_json(
		&self,
		s: State,
		padding: usize,
		#[cfg(feature = "exp-preserve-order")] preserve_order: bool,
	) -> Result<IStr> {
		manifest_json_ex(
			s,
			self,
			&ManifestJsonOptions {
				padding: &" ".repeat(padding),
				mtype: if padding == 0 {
					ManifestType::Minify
				} else {
					ManifestType::Manifest
				},
				newline: "\n",
				key_val_sep: ": ",
				#[cfg(feature = "exp-preserve-order")]
				preserve_order,
			},
		)
		.map(Into::into)
	}

	/// Calls `std.manifestJson`
	pub fn to_std_json(
		&self,
		s: State,
		padding: usize,
		#[cfg(feature = "exp-preserve-order")] preserve_order: bool,
	) -> Result<Rc<str>> {
		manifest_json_ex(
			s,
			self,
			&ManifestJsonOptions {
				padding: &" ".repeat(padding),
				mtype: ManifestType::Std,
				newline: "\n",
				key_val_sep: ": ",
				#[cfg(feature = "exp-preserve-order")]
				preserve_order,
			},
		)
		.map(Into::into)
	}

	pub fn to_yaml(
		&self,
		s: State,
		padding: usize,
		#[cfg(feature = "exp-preserve-order")] preserve_order: bool,
	) -> Result<IStr> {
		let padding = &" ".repeat(padding);
		manifest_yaml_ex(
			s,
			self,
			&ManifestYamlOptions {
				padding,
				arr_element_padding: padding,
				quote_keys: false,
				#[cfg(feature = "exp-preserve-order")]
				preserve_order,
			},
		)
		.map(Into::into)
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
pub fn equals(s: State, val_a: &Val, val_b: &Val) -> Result<bool> {
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
			for (a, b) in a.iter(s.clone()).zip(b.iter(s.clone())) {
				if !equals(s.clone(), &a?, &b?)? {
					return Ok(false);
				}
			}
			Ok(true)
		}
		(Val::Obj(a), Val::Obj(b)) => {
			if ObjValue::ptr_eq(a, b) {
				return Ok(true);
			}
			let fields = a.fields(
				#[cfg(feature = "exp-preserve-order")]
				false,
			);
			if fields
				!= b.fields(
					#[cfg(feature = "exp-preserve-order")]
					false,
				) {
				return Ok(false);
			}
			for field in fields {
				if !equals(
					s.clone(),
					&a.get(s.clone(), field.clone())?.expect("field exists"),
					&b.get(s.clone(), field)?.expect("field exists"),
				)? {
					return Ok(false);
				}
			}
			Ok(true)
		}
		(a, b) => Ok(primitive_equals(a, b)?),
	}
}
