use std::{
	cell::{Ref, RefCell, RefMut},
	collections::HashMap,
	rc::Rc,
};

use jrsonnet_evaluator::{
	error::{ErrorKind::*, Result},
	function::{builtin::Builtin, CallLocation, FuncVal, TlaArg},
	gc::TraceBox,
	tb,
	trace::PathResolver,
	ContextBuilder, IStr, ObjValue, ObjValueBuilder, State, Thunk, Val,
};
use jrsonnet_gcmodule::{Cc, Trace};
use jrsonnet_parser::Source;

mod expr;
mod types;
pub use types::*;
mod arrays;
pub use arrays::*;
mod math;
pub use math::*;
mod operator;
pub use operator::*;
mod sort;
pub use sort::*;
mod hash;
pub use hash::*;
mod encoding;
pub use encoding::*;
mod objects;
pub use objects::*;
mod manifest;
pub use manifest::*;
mod parse;
pub use parse::*;
mod strings;
pub use strings::*;
mod misc;
pub use misc::*;
mod sets;
pub use sets::*;
mod compat;
pub use compat::*;
mod regex;
pub use crate::regex::*;

pub fn stdlib_uncached(settings: Rc<RefCell<Settings>>) -> ObjValue {
	let mut builder = ObjValueBuilder::new();

	let expr = expr::stdlib_expr();
	let eval = jrsonnet_evaluator::evaluate(ContextBuilder::dangerous_empty_state().build(), &expr)
		.expect("stdlib.jsonnet should have no errors")
		.as_obj()
		.expect("stdlib.jsonnet should evaluate to object");

	builder.with_super(eval);

	for (name, builtin) in [
		// Types
		("type", builtin_type::INST),
		("isString", builtin_is_string::INST),
		("isNumber", builtin_is_number::INST),
		("isBoolean", builtin_is_boolean::INST),
		("isObject", builtin_is_object::INST),
		("isArray", builtin_is_array::INST),
		("isFunction", builtin_is_function::INST),
		// Arrays
		("makeArray", builtin_make_array::INST),
		("repeat", builtin_repeat::INST),
		("slice", builtin_slice::INST),
		("map", builtin_map::INST),
		("flatMap", builtin_flatmap::INST),
		("filter", builtin_filter::INST),
		("foldl", builtin_foldl::INST),
		("foldr", builtin_foldr::INST),
		("range", builtin_range::INST),
		("join", builtin_join::INST),
		("reverse", builtin_reverse::INST),
		("any", builtin_any::INST),
		("all", builtin_all::INST),
		("member", builtin_member::INST),
		("contains", builtin_member::INST),
		("count", builtin_count::INST),
		("avg", builtin_avg::INST),
		("removeAt", builtin_remove_at::INST),
		("remove", builtin_remove::INST),
		// Math
		("abs", builtin_abs::INST),
		("sign", builtin_sign::INST),
		("max", builtin_max::INST),
		("min", builtin_min::INST),
		("sum", builtin_sum::INST),
		("modulo", builtin_modulo::INST),
		("floor", builtin_floor::INST),
		("ceil", builtin_ceil::INST),
		("log", builtin_log::INST),
		("pow", builtin_pow::INST),
		("sqrt", builtin_sqrt::INST),
		("sin", builtin_sin::INST),
		("cos", builtin_cos::INST),
		("tan", builtin_tan::INST),
		("asin", builtin_asin::INST),
		("acos", builtin_acos::INST),
		("atan", builtin_atan::INST),
		("exp", builtin_exp::INST),
		("mantissa", builtin_mantissa::INST),
		("exponent", builtin_exponent::INST),
		("round", builtin_round::INST),
		("isEven", builtin_is_even::INST),
		("isOdd", builtin_is_odd::INST),
		("isInteger", builtin_is_integer::INST),
		("isDecimal", builtin_is_decimal::INST),
		// Operator
		("mod", builtin_mod::INST),
		("primitiveEquals", builtin_primitive_equals::INST),
		("equals", builtin_equals::INST),
		("xor", builtin_xor::INST),
		("xnor", builtin_xnor::INST),
		("format", builtin_format::INST),
		// Sort
		("sort", builtin_sort::INST),
		("uniq", builtin_uniq::INST),
		("set", builtin_set::INST),
		("minArray", builtin_min_array::INST),
		("maxArray", builtin_max_array::INST),
		// Hash
		("md5", builtin_md5::INST),
		("sha1", builtin_sha1::INST),
		("sha256", builtin_sha256::INST),
		("sha512", builtin_sha512::INST),
		("sha3", builtin_sha3::INST),
		// Encoding
		("encodeUTF8", builtin_encode_utf8::INST),
		("decodeUTF8", builtin_decode_utf8::INST),
		("base64", builtin_base64::INST),
		("base64Decode", builtin_base64_decode::INST),
		("base64DecodeBytes", builtin_base64_decode_bytes::INST),
		// Objects
		("objectFieldsEx", builtin_object_fields_ex::INST),
		("objectHasEx", builtin_object_has_ex::INST),
		("objectRemoveKey", builtin_object_remove_key::INST),
		// Manifest
		("escapeStringJson", builtin_escape_string_json::INST),
		("manifestJsonEx", builtin_manifest_json_ex::INST),
		("manifestYamlDoc", builtin_manifest_yaml_doc::INST),
		("manifestTomlEx", builtin_manifest_toml_ex::INST),
		// Parsing
		("parseJson", builtin_parse_json::INST),
		("parseYaml", builtin_parse_yaml::INST),
		// Strings
		("codepoint", builtin_codepoint::INST),
		("substr", builtin_substr::INST),
		("char", builtin_char::INST),
		("strReplace", builtin_str_replace::INST),
		("isEmpty", builtin_is_empty::INST),
		("equalsIgnoreCase", builtin_equals_ignore_case::INST),
		("splitLimit", builtin_splitlimit::INST),
		("asciiUpper", builtin_ascii_upper::INST),
		("asciiLower", builtin_ascii_lower::INST),
		("findSubstr", builtin_find_substr::INST),
		("parseInt", builtin_parse_int::INST),
		#[cfg(feature = "exp-bigint")]
		("bigint", builtin_bigint::INST),
		("parseOctal", builtin_parse_octal::INST),
		("parseHex", builtin_parse_hex::INST),
		// Misc
		("length", builtin_length::INST),
		("startsWith", builtin_starts_with::INST),
		("endsWith", builtin_ends_with::INST),
		// Sets
		("setMember", builtin_set_member::INST),
		("setInter", builtin_set_inter::INST),
		// Regex
		("regexQuoteMeta", builtin_regex_quote_meta::INST),
		// Compat
		("__compare", builtin___compare::INST),
	]
	.iter()
	.cloned()
	{
		builder
			.member(name.into())
			.hide()
			.value(Val::Func(FuncVal::StaticBuiltin(builtin)))
			.expect("no conflict");
	}

	builder
		.member("extVar".into())
		.hide()
		.value(Val::Func(FuncVal::builtin(builtin_ext_var {
			settings: settings.clone(),
		})))
		.expect("no conflict");
	builder
		.member("native".into())
		.hide()
		.value(Val::Func(FuncVal::builtin(builtin_native {
			settings: settings.clone(),
		})))
		.expect("no conflict");
	builder
		.member("trace".into())
		.hide()
		.value(Val::Func(FuncVal::builtin(builtin_trace { settings })))
		.expect("no conflict");

	// Regex
	let regex_cache = RegexCache::default();
	builder
		.member("regexFullMatch".into())
		.hide()
		.value(Val::Func(FuncVal::builtin(builtin_regex_full_match {
			cache: regex_cache.clone(),
		})))
		.expect("no conflict");
	builder
		.member("regexPartialMatch".into())
		.hide()
		.value(Val::Func(FuncVal::builtin(builtin_regex_partial_match {
			cache: regex_cache.clone(),
		})))
		.expect("no conflict");
	builder
		.member("regexReplace".into())
		.hide()
		.value(Val::Func(FuncVal::builtin(builtin_regex_replace {
			cache: regex_cache.clone(),
		})))
		.expect("no conflict");
	builder
		.member("regexGlobalReplace".into())
		.hide()
		.value(Val::Func(FuncVal::builtin(builtin_regex_global_replace {
			cache: regex_cache.clone(),
		})))
		.expect("no conflict");

	builder
		.member("id".into())
		.hide()
		.value(Val::Func(FuncVal::Id))
		.expect("no conflict");

	builder.build()
}

pub trait TracePrinter {
	fn print_trace(&self, loc: CallLocation, value: IStr);
}

pub struct StdTracePrinter {
	resolver: PathResolver,
}
impl StdTracePrinter {
	pub fn new(resolver: PathResolver) -> Self {
		Self { resolver }
	}
}
impl TracePrinter for StdTracePrinter {
	fn print_trace(&self, loc: CallLocation, value: IStr) {
		eprint!("TRACE:");
		if let Some(loc) = loc.0 {
			let locs = loc.0.map_source_locations(&[loc.1]);
			eprint!(
				" {}:{}",
				match loc.0.source_path().path() {
					Some(p) => self.resolver.resolve(p),
					None => loc.0.source_path().to_string(),
				},
				locs[0].line
			);
		}
		eprintln!(" {value}");
	}
}

pub struct Settings {
	/// Used for `std.extVar`
	pub ext_vars: HashMap<IStr, TlaArg>,
	/// Used for `std.native`
	pub ext_natives: HashMap<IStr, Cc<TraceBox<dyn Builtin>>>,
	/// Used for `std.trace`
	pub trace_printer: Box<dyn TracePrinter>,
	/// Used for `std.thisFile`
	pub path_resolver: PathResolver,
}

fn extvar_source(name: &str, code: impl Into<IStr>) -> Source {
	let source_name = format!("<extvar:{name}>");
	Source::new_virtual(source_name.into(), code.into())
}

#[derive(Trace, Clone)]
pub struct ContextInitializer {
	/// When we don't need to support legacy-this-file, we can reuse same context for all files
	#[cfg(not(feature = "legacy-this-file"))]
	context: jrsonnet_evaluator::Context,
	/// For `populate`
	#[cfg(not(feature = "legacy-this-file"))]
	stdlib_thunk: Thunk<Val>,
	/// Otherwise, we can only keep first stdlib layer, and then stack thisFile on top of it
	#[cfg(feature = "legacy-this-file")]
	stdlib_obj: ObjValue,
	settings: Rc<RefCell<Settings>>,
}
impl ContextInitializer {
	pub fn new(_s: State, resolver: PathResolver) -> Self {
		let settings = Settings {
			ext_vars: Default::default(),
			ext_natives: Default::default(),
			trace_printer: Box::new(StdTracePrinter::new(resolver.clone())),
			path_resolver: resolver,
		};
		let settings = Rc::new(RefCell::new(settings));
		let stdlib_obj = stdlib_uncached(settings.clone());
		#[cfg(not(feature = "legacy-this-file"))]
		let stdlib_thunk = Thunk::evaluated(Val::Obj(stdlib_obj));
		Self {
			#[cfg(not(feature = "legacy-this-file"))]
			context: {
				let mut context = ContextBuilder::with_capacity(_s, 1);
				context.bind("std".into(), stdlib_thunk.clone());
				context.build()
			},
			#[cfg(not(feature = "legacy-this-file"))]
			stdlib_thunk,
			#[cfg(feature = "legacy-this-file")]
			stdlib_obj,
			settings,
		}
	}
	pub fn settings(&self) -> Ref<Settings> {
		self.settings.borrow()
	}
	pub fn settings_mut(&self) -> RefMut<Settings> {
		self.settings.borrow_mut()
	}
	pub fn add_ext_var(&self, name: IStr, value: Val) {
		self.settings_mut()
			.ext_vars
			.insert(name, TlaArg::Val(value));
	}
	pub fn add_ext_str(&self, name: IStr, value: IStr) {
		self.settings_mut()
			.ext_vars
			.insert(name, TlaArg::String(value));
	}
	pub fn add_ext_code(&self, name: &str, code: impl Into<IStr>) -> Result<()> {
		let code = code.into();
		let source = extvar_source(name, code.clone());
		let parsed = jrsonnet_parser::parse(
			&code,
			&jrsonnet_parser::ParserSettings {
				source: source.clone(),
			},
		)
		.map_err(|e| ImportSyntaxError {
			path: source,
			error: Box::new(e),
		})?;
		// self.data_mut().volatile_files.insert(source_name, code);
		self.settings_mut()
			.ext_vars
			.insert(name.into(), TlaArg::Code(parsed));
		Ok(())
	}
	pub fn add_native(&self, name: IStr, cb: impl Builtin) {
		self.settings_mut()
			.ext_natives
			.insert(name, Cc::new(tb!(cb)));
	}
}
impl jrsonnet_evaluator::ContextInitializer for ContextInitializer {
	fn reserve_vars(&self) -> usize {
		1
	}
	#[cfg(not(feature = "legacy-this-file"))]
	fn initialize(&self, _s: State, _source: Source) -> jrsonnet_evaluator::Context {
		self.context.clone()
	}
	#[cfg(not(feature = "legacy-this-file"))]
	fn populate(&self, _for_file: Source, builder: &mut ContextBuilder) {
		builder.bind("std".into(), self.stdlib_thunk.clone());
	}
	#[cfg(feature = "legacy-this-file")]
	fn populate(&self, source: Source, builder: &mut ContextBuilder) {
		use jrsonnet_evaluator::val::StrValue;

		let mut std = ObjValueBuilder::new();
		std.with_super(self.stdlib_obj.clone());
		std.member("thisFile".into())
			.hide()
			.value(Val::Str(StrValue::Flat(
				match source.source_path().path() {
					Some(p) => self.settings().path_resolver.resolve(p).into(),
					None => source.source_path().to_string().into(),
				},
			)))
			.expect("this object builder is empty");
		let stdlib_with_this_file = std.build();

		builder.bind(
			"std".into(),
			Thunk::evaluated(Val::Obj(stdlib_with_this_file)),
		);
	}
	fn as_any(&self) -> &dyn std::any::Any {
		self
	}
}

pub trait StateExt {
	/// This method was previously implemented in jrsonnet-evaluator itself
	fn with_stdlib(&self);
}

impl StateExt for State {
	fn with_stdlib(&self) {
		let initializer = ContextInitializer::new(self.clone(), PathResolver::new_cwd_fallback());
		self.settings_mut().context_initializer = tb!(initializer)
	}
}
