use jrsonnet_evaluator::{
	bail,
	function::{builtin, FuncVal},
	parser::Source,
	ContextBuilder, ContextInitializer as ContextInitializerT, ObjValueBuilder, Result, Thunk, Val,
};
use jrsonnet_gcmodule::Trace;

#[macro_export]
macro_rules! ensure_eq {
	($a:expr, $b:expr $(,)?) => {{
		let a = &$a;
		let b = &$b;
		if a != b {
			::jrsonnet_evaluator::bail!("assertion failed: a != b\na={a:#?}\nb={b:#?}")
		}
	}};
}

#[macro_export]
macro_rules! ensure {
	($v:expr $(,)?) => {
		if !$v {
			::jrsonnet_evaluator::bail!("assertion failed: {}", stringify!($v))
		}
	};
}

#[macro_export]
macro_rules! ensure_val_eq {
	($a:expr, $b:expr) => {{
		if !::jrsonnet_evaluator::val::equals(&$a.clone(), &$b.clone())? {
			use ::jrsonnet_evaluator::manifest::JsonFormat;
			::jrsonnet_evaluator::bail!(
				"assertion failed: a != b\na={:#?}\nb={:#?}",
				$a.manifest(JsonFormat::default())?,
				$b.manifest(JsonFormat::default())?,
			)
		}
	}};
}

#[builtin]
fn assert_throw(lazy: Thunk<Val>, message: String) -> Result<bool> {
	match lazy.evaluate() {
		Ok(_) => {
			bail!("expected argument to throw on evaluation, but it returned instead")
		}
		Err(e) => {
			let error = format!("{}", e.error());
			ensure_eq!(message, error);
		}
	}
	Ok(true)
}

#[builtin]
fn param_names(fun: FuncVal) -> Vec<String> {
	match fun {
		FuncVal::Id => vec!["x".to_string()],
		FuncVal::Normal(func) => func
			.params
			.iter()
			.map(|p| p.0.name().unwrap_or_else(|| "<unnamed>".into()).to_string())
			.collect(),
		FuncVal::StaticBuiltin(b) => b
			.params()
			.iter()
			.map(|p| p.name().as_str().unwrap_or("<unnamed>").to_string())
			.collect(),
		FuncVal::Builtin(b) => b
			.as_ref()
			.params()
			.iter()
			.map(|p| p.name().as_str().unwrap_or("<unnamed>").to_string())
			.collect(),
		FuncVal::Thunk(_) => vec![],
	}
}

#[derive(Trace)]
pub struct ContextInitializer;
impl ContextInitializerT for ContextInitializer {
	fn populate(&self, _for_file: Source, builder: &mut ContextBuilder) {
		let mut bobj = ObjValueBuilder::new();
		bobj.method("assertThrow", assert_throw::INST);
		bobj.method("paramNames", param_names::INST);

		builder.bind("test", Thunk::evaluated(Val::Obj(bobj.build())));
	}
}
