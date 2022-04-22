#[macro_export]
macro_rules! ensure_eq {
	($a:expr, $b:expr $(,)?) => {{
		if $a != $b {
			::jrsonnet_evaluator::throw_runtime!(
				"assertion failed: a != b\na={:#?}\nb={:#?}",
				$a,
				$b,
			)
		}
	}};
}

#[macro_export]
macro_rules! ensure_val_eq {
	($s:expr, $a:expr, $b:expr) => {{
		if !::jrsonnet_evaluator::val::equals($s.clone(), &$a.clone(), &$b.clone())? {
			::jrsonnet_evaluator::throw_runtime!(
				"assertion failed: a != b\na={:#?}\nb={:#?}",
				$a.to_json($s.clone(), 2)?,
				$b.to_json($s.clone(), 2)?,
			)
		}
	}};
}
