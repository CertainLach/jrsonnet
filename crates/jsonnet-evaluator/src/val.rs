use crate::{
	dynamic_wrapper, evaluate, evaluate_method, BoxedContextCreator, Context, FunctionDefault,
	FunctionRhs, ObjValue,
};
use crate::{Binding, BoxedBinding, BoxedFunctionDefault, BoxedFunctionRhs, FutureContext};
use jsonnet_parser::{ArgsDesc, Expr, LiteralType, Param, ParamsDesc};
use std::{
	collections::HashMap,
	fmt::{Debug, Display},
	ops::Deref,
	rc::Rc,
};

pub trait LazyVal: Debug {
	fn evaluate(&self) -> Val;
}
dynamic_wrapper!(LazyVal, BoxedLazyVal);

#[derive(Debug)]
pub struct PlainLazyVal {
	pub expr: Expr,
	pub context: Context,
}
impl LazyVal for PlainLazyVal {
	fn evaluate(&self) -> Val {
		evaluate(self.context.clone(), &self.expr)
	}
}

#[derive(Debug)]
pub struct NoArgsBindingLazyVal {
	pub expr: Expr,
	pub context_creator: BoxedContextCreator,

	pub this: Option<ObjValue>,
	pub super_obj: Option<ObjValue>,
}
impl LazyVal for NoArgsBindingLazyVal {
	fn evaluate(&self) -> Val {
		evaluate(
			self.context_creator
				.create_context(&self.this, &self.super_obj),
			&self.expr,
		)
	}
}

#[derive(Debug)]
pub struct ArgsBindingLazyVal {
	pub expr: Expr,
	pub args: ParamsDesc,
	pub context_creator: BoxedContextCreator,

	pub this: Option<ObjValue>,
	pub super_obj: Option<ObjValue>,
}
impl LazyVal for ArgsBindingLazyVal {
	fn evaluate(&self) -> Val {
		evaluate_method(
			self.context_creator
				.create_context(&self.this, &self.super_obj),
			&self.expr,
			self.args.clone(),
		)
	}
}

#[derive(Debug)]
pub struct FunctionDefaultBinding {
	eval: BoxedFunctionDefault,
	default: Expr,
	ctx: FutureContext,
}
impl Binding for FunctionDefaultBinding {
	fn evaluate(&self, _this: Option<ObjValue>, _super_obj: Option<ObjValue>) -> Val {
		self.eval
			.default(self.ctx.clone().unwrap(), self.default.clone())
	}
}

#[derive(Debug)]
pub struct ValBinding {
	val: Val,
}
impl Binding for ValBinding {
	fn evaluate(&self, this: Option<ObjValue>, super_obj: Option<ObjValue>) -> Val {
		self.val.clone()
	}
}

#[derive(Debug, PartialEq, Clone)]
pub struct FuncDesc {
	pub ctx: Context,
	pub params: ParamsDesc,
	pub eval_rhs: BoxedFunctionRhs,
	pub eval_default: BoxedFunctionDefault,
}
impl FuncDesc {
	// TODO: Check for unset variables
	pub fn evaluate(&self, args: Vec<(Option<String>, Val)>) -> Val {
		let mut new_bindings: HashMap<String, BoxedBinding> = HashMap::new();
		let future_ctx = Context::new_future();

		self.params
			.with_defaults()
			.into_iter()
			.for_each(|Param(name, default)| {
				new_bindings.insert(
					name,
					Rc::new(FunctionDefaultBinding {
						eval: self.eval_default.clone(),
						default: *default.unwrap().clone(),
						ctx: future_ctx.clone(),
					}),
				);
			});
		for (name, val) in args.iter().filter(|e| e.0.is_some()) {
			new_bindings.insert(
				name.as_ref().unwrap().clone(),
				Rc::new(ValBinding { val: val.clone() }),
			);
		}
		for (i, param) in self.params.0.iter().enumerate() {
			if let Some((None, val)) = args.get(i) {
				new_bindings.insert(param.0.clone(), Rc::new(ValBinding { val: val.clone() }));
			}
		}
		let ctx = self
			.ctx
			.extend(new_bindings, None, None, None)
			.into_future(future_ctx);
		self.eval_rhs.evaluate(ctx)
	}
}

#[derive(Debug, PartialEq, Clone)]
pub enum Val {
	Literal(LiteralType),
	Str(String),
	Num(f64),
	Lazy(BoxedLazyVal),
	Arr(Vec<Val>),
	Obj(ObjValue),
	Func(FuncDesc),
}
impl Val {
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
			Val::Literal(v) => write!(f, "{}", v)?,
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
					write!(f, "{}", value.get_raw(&field, None).unwrap())?;
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
	if v {
		Val::Literal(LiteralType::True)
	} else {
		Val::Literal(LiteralType::False)
	}
}
