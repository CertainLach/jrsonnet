use jrsonnet_evaluator::{
	error::Result,
	function::{builtin, FuncVal},
	throw, ObjValueBuilder, State, Thunk, Val,
};
use jrsonnet_stdlib::StateExt;

#[macro_export]
macro_rules! ensure_eq {
	($a:expr, $b:expr $(,)?) => {{
		let a = &$a;
		let b = &$b;
		if a != b {
			::jrsonnet_evaluator::throw!("assertion failed: a != b\na={:#?}\nb={:#?}", a, b)
		}
	}};
}

#[macro_export]
macro_rules! ensure {
	($v:expr $(,)?) => {
		if !$v {
			::jrsonnet_evaluator::throw!("assertion failed: {}", stringify!($v))
		}
	};
}

#[macro_export]
macro_rules! ensure_val_eq {
	($s:expr, $a:expr, $b:expr) => {{
		if !::jrsonnet_evaluator::val::equals($s.clone(), &$a.clone(), &$b.clone())? {
			::jrsonnet_evaluator::throw!(
				"assertion failed: a != b\na={:#?}\nb={:#?}",
				$a.to_json(
					$s.clone(),
					2,
					#[cfg(feature = "exp-preserve-order")]
					false
				)?,
				$b.to_json(
					$s.clone(),
					2,
					#[cfg(feature = "exp-preserve-order")]
					false
				)?,
			)
		}
	}};
}

#[builtin]
fn assert_throw(s: State, lazy: Thunk<Val>, message: String) -> Result<bool> {
	match lazy.evaluate(s) {
		Ok(_) => {
			throw!("expected argument to throw on evaluation, but it returned instead")
		}
		Err(e) => {
			let error = format!("{}", e.error());
			ensure_eq!(message, error);
		}
	}
	Ok(true)
}

#[allow(dead_code)]
pub fn with_test(s: &State) {
	let mut bobj = ObjValueBuilder::new();
	bobj.member("assertThrow".into())
		.hide()
		.value(
			s.clone(),
			Val::Func(FuncVal::StaticBuiltin(assert_throw::INST)),
		)
		.expect("no error");

	s.add_global("test".into(), Thunk::evaluated(Val::Obj(bobj.build())))
}
