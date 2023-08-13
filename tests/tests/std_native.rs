use jrsonnet_evaluator::{function::builtin, trace::PathResolver, State};
use jrsonnet_stdlib::ContextInitializer;

#[builtin]
fn example_native(a: u32, b: u32) -> u32 {
	a + b
}

#[test]
fn std_native() {
	let state = State::default();
	let std = ContextInitializer::new(state.clone(), PathResolver::Absolute);
	std.add_native("example", example_native::INST);
	state.set_context_initializer(std);

	assert!(state
		.evaluate_snippet("test", "std.native('example')(1, 3) == 4")
		.unwrap()
		.as_bool()
		.expect("boolean output"));
}
