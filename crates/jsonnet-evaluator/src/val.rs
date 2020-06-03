use crate::{lazy_binding, Context, FunctionDefault, FunctionRhs, LazyBinding, ObjValue};
use closure::closure;
use jsonnet_parser::ParamsDesc;
use std::{
	cell::RefCell,
	collections::HashMap,
	fmt::{Debug, Display},
	rc::Rc,
};

struct LazyValInternals {
	pub f: Box<dyn Fn() -> Val>,
	pub cached: RefCell<Option<Val>>,
}
#[derive(Clone)]
pub struct LazyVal(Rc<LazyValInternals>);
impl LazyVal {
	pub fn new(f: Box<dyn Fn() -> Val>) -> Self {
		LazyVal(Rc::new(LazyValInternals {
			f,
			cached: RefCell::new(None),
		}))
	}
	pub fn evaluate(&self) -> Val {
		{
			let cached = self.0.cached.borrow();
			if cached.is_some() {
				return cached.clone().unwrap();
			}
		}
		let result = (self.0.f)();
		self.0.cached.borrow_mut().replace(result.clone());
		result
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
	pub fn evaluate(&self, args: Vec<(Option<String>, Val)>) -> Val {
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
					closure!(clone val, |_, _| lazy_val!(closure!(clone val, || val.clone())))
				),
			);
		}
		for (i, param) in self.params.0.iter().enumerate() {
			if let Some((None, val)) = args.get(i) {
				new_bindings.insert(
					param.0.clone(),
					lazy_binding!(
						closure!(clone val, |_, _| lazy_val!(closure!(clone val, || val.clone())))
					),
				);
			}
		}
		let ctx = self
			.ctx
			.extend(new_bindings, None, None, None)
			.into_future(future_ctx);
		self.eval_rhs.0(ctx)
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
	pub fn try_cast_bool(self) -> bool {
		match self.unwrap_if_lazy() {
			Val::Bool(v) => v,
			v => panic!("expected bool, got {:?}", v),
		}
	}
	pub fn try_cast_str(self) -> String {
		match self.unwrap_if_lazy() {
			Val::Str(v) => v,
			v => panic!("expected bool, got {:?}", v),
		}
	}
	pub fn unwrap_if_lazy(self) -> Self {
		if let Val::Lazy(v) = self {
			v.evaluate().unwrap_if_lazy()
		} else {
			self
		}
	}
	pub fn type_of(&self) -> &'static str {
		match self {
			Val::Str(..) => "string",
			Val::Num(..) => "number",
			Val::Arr(..) => "array",
			Val::Obj(..) => "object",
			Val::Func(..) => "function",
			_ => panic!("no native type found"),
		}
	}
}
impl Display for Val {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Val::Str(str) => write!(f, "\"{}\"", str)?,
			Val::Num(n) => write!(f, "{}", n)?,
			Val::Arr(values) => {
				write!(f, "[")?;
				let mut first = true;
				for value in values {
					if first {
						first = false;
					} else {
						write!(f, ",")?;
					}
					write!(f, "{}", value)?;
				}
				write!(f, "]")?;
			}
			Val::Obj(value) => {
				write!(f, "{{")?;
				let mut first = true;
				for field in value.fields() {
					if first {
						first = false;
					} else {
						write!(f, ",")?;
					}
					write!(f, "\"{}\":", field)?;
					write!(f, "{}", value.get(&field).unwrap())?;
				}
				write!(f, "}}")?;
			}
			Val::Lazy(lazy) => {
				write!(f, "{}", lazy.evaluate())?;
			}
			Val::Func(_) => {
				write!(f, "<<FUNC>>")?;
			}
			v => panic!("no json equivalent for {:?}", v),
		};
		Ok(())
	}
}

pub fn bool_val(v: bool) -> Val {
	Val::Bool(v)
}
