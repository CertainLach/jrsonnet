use crate::{
	create_error, lazy_binding, Context, Error, FunctionDefault, FunctionRhs, LazyBinding,
	ObjValue, Result,
};
use closure::closure;
use jsonnet_parser::ParamsDesc;
use std::{
	cell::RefCell,
	collections::HashMap,
	fmt::{Debug, Display},
	rc::Rc,
};

struct LazyValInternals {
	pub f: Box<dyn Fn() -> Result<Val>>,
	pub cached: RefCell<Option<Val>>,
}
#[derive(Clone)]
pub struct LazyVal(Rc<LazyValInternals>);
impl LazyVal {
	pub fn new(f: Box<dyn Fn() -> Result<Val>>) -> Self {
		LazyVal(Rc::new(LazyValInternals {
			f,
			cached: RefCell::new(None),
		}))
	}
	pub fn evaluate(&self) -> Result<Val> {
		{
			let cached = self.0.cached.borrow();
			if cached.is_some() {
				return Ok(cached.clone().unwrap());
			}
		}
		let result = (self.0.f)()?;
		self.0.cached.borrow_mut().replace(result.clone());
		Ok(result)
	}
}
#[macro_export]
macro_rules! lazy_val {
	($f: expr) => {
		$crate::LazyVal::new(Box::new($f))
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
	pub eval_rhs: FunctionRhs,
	pub eval_default: FunctionDefault,
}
impl FuncDesc {
	// TODO: Check for unset variables
	pub fn evaluate(&self, args: Vec<(Option<String>, Val)>) -> Result<Val> {
		let mut new_bindings: HashMap<String, LazyBinding> = HashMap::new();
		let future_ctx = Context::new_future();

		// self.params
		// 	.with_defaults()
		// 	.into_iter()
		// 	.for_each(|Param(name, default)| {
		// 		let default = Rc::new(*default.unwrap());
		// 		new_bindings.insert(
		// 			name,
		// 			binding!(move |_, _| Val::Lazy(lazy_val!(|| self
		// 				.eval_default
		// 				.0
		// 				.default(future_ctx.unwrap(), *default.clone())))),
		// 		);
		// 	});
		for (name, val) in args.clone().into_iter().filter(|e| e.0.is_some()) {
			new_bindings.insert(
				name.as_ref().unwrap().clone(),
				lazy_binding!(
					closure!(clone val, |_, _| Ok(lazy_val!(closure!(clone val, || Ok(val.clone())))))
				),
			);
		}
		for (i, param) in self.params.0.iter().enumerate() {
			if let Some((None, val)) = args.get(i) {
				new_bindings.insert(
					param.0.clone(),
					lazy_binding!(
						closure!(clone val, |_, _| Ok(lazy_val!(closure!(clone val, || Ok(val.clone())))))
					),
				);
			}
		}
		let ctx = self
			.ctx
			.extend(new_bindings, None, None, None)?
			.into_future(future_ctx);
		self.eval_rhs.0(ctx)
	}
}

#[derive(Debug)]
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
}
