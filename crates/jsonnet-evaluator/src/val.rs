use crate::{
	create_error, evaluate,
	function::{inline_parse_function_call, place_args},
	with_state, Context, Error, ObjValue, Result,
};
use jsonnet_parser::{el, Arg, ArgsDesc, Expr, LocExpr, ParamsDesc};
use std::{
	cell::RefCell,
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

#[derive(Debug, PartialEq, Clone)]
pub struct FuncDesc {
	pub ctx: Context,
	pub params: ParamsDesc,
	pub body: LocExpr,
}
impl FuncDesc {
	/// This function is always inlined to make tailstrict work
	pub fn evaluate(&self, call_ctx: Context, args: &ArgsDesc, tailstrict: bool) -> Result<Val> {
		let ctx = inline_parse_function_call(
			call_ctx,
			Some(self.ctx.clone()),
			&self.params,
			args,
			tailstrict,
		)?;
		evaluate(ctx, &self.body)
	}

	pub fn evaluate_values(&self, call_ctx: Context, args: &[Val]) -> Result<Val> {
		let ctx = place_args(call_ctx, Some(self.ctx.clone()), &self.params, args)?;
		evaluate(ctx, &self.body)
	}
}

#[derive(Debug, Clone)]
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

#[derive(Debug, PartialEq, Clone)]
pub enum Val {
	Bool(bool),
	Null,
	Str(String),
	Num(f64),
	Lazy(LazyVal),
	Arr(Vec<Val>),
	Obj(ObjValue),
	Func(FuncDesc),

	// Library functions implemented in native
	Intristic(String, String),
}
impl Val {
	pub fn try_cast_bool(self, context: &'static str) -> Result<bool> {
		match self.unwrap_if_lazy()? {
			Val::Bool(v) => Ok(v),
			v => create_error(Error::TypeMismatch(
				context,
				vec![ValType::Bool],
				v.value_type()?,
			)),
		}
	}
	pub fn try_cast_str(self, context: &'static str) -> Result<String> {
		match self.unwrap_if_lazy()? {
			Val::Str(v) => Ok(v),
			v => create_error(Error::TypeMismatch(
				context,
				vec![ValType::Str],
				v.value_type()?,
			)),
		}
	}
	pub fn unwrap_if_lazy(self) -> Result<Self> {
		Ok(if let Val::Lazy(v) = self {
			v.evaluate()?.unwrap_if_lazy()?
		} else {
			self
		})
	}
	pub fn value_type(&self) -> Result<ValType> {
		Ok(match self {
			Val::Str(..) => ValType::Str,
			Val::Num(..) => ValType::Num,
			Val::Arr(..) => ValType::Arr,
			Val::Obj(..) => ValType::Obj,
			Val::Func(..) => ValType::Func,
			Val::Bool(_) => ValType::Bool,
			Val::Null => ValType::Null,
			Val::Intristic(_, _) => ValType::Func,
			Val::Lazy(_) => self.clone().unwrap_if_lazy()?.value_type()?,
		})
	}
	pub fn into_json(self, padding: usize) -> Result<String> {
		with_state(|s| {
			let ctx = s
				.create_default_context()?
				.with_var("__tmp__to_json__".to_owned(), self)?;
			if let Val::Str(result) = evaluate(
				ctx,
				&el!(Expr::Apply(
					el!(Expr::Index(
						el!(Expr::Var("std".to_owned())),
						el!(Expr::Str("manifestJsonEx".to_owned()))
					)),
					ArgsDesc(vec![
						Arg(None, el!(Expr::Var("__tmp__to_json__".to_owned()))),
						Arg(None, el!(Expr::Str(" ".repeat(padding))))
					]),
					false
				)),
			)? {
				Ok(result)
			} else {
				unreachable!()
			}
		})
	}
}
