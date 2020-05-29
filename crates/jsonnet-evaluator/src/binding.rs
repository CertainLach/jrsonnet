use crate::{
	ArgsBindingLazyVal, BoxedContextCreator, BoxedLazyVal, NoArgsBindingLazyVal, ObjValue, Val,
};
use jsonnet_parser::{Expr, ParamsDesc};
use std::{fmt::Debug, rc::Rc};

pub trait Binding: Debug {
	fn evaluate(&self, this: Option<ObjValue>, super_obj: Option<ObjValue>) -> Val;
}
pub type BoxedBinding = Rc<dyn Binding>;

#[derive(Debug)]
pub struct NoArgsBinding {
	pub expr: Expr,
	pub context_creator: BoxedContextCreator,
}
impl Binding for NoArgsBinding {
	fn evaluate(&self, this: Option<ObjValue>, super_obj: Option<ObjValue>) -> Val {
		Val::Lazy(BoxedLazyVal(Rc::new(NoArgsBindingLazyVal {
			context_creator: self.context_creator.clone(),
			expr: self.expr.clone(),
			this,
			super_obj,
		})))
	}
}
#[derive(Debug)]
pub struct ArgsBinding {
	pub expr: Expr,
	pub args: ParamsDesc,
	pub context_creator: BoxedContextCreator,
}
impl Binding for ArgsBinding {
	fn evaluate(&self, this: Option<ObjValue>, super_obj: Option<ObjValue>) -> Val {
		Val::Lazy(BoxedLazyVal(Rc::new(ArgsBindingLazyVal {
			context_creator: self.context_creator.clone(),
			expr: self.expr.clone(),
			args: self.args.clone(),
			this,
			super_obj,
		})))
	}
}
