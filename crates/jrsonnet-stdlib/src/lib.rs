#![allow(clippy::similar_names)]

use std::{
	cell::{Ref, RefCell, RefMut},
	collections::HashMap,
	rc::Rc,
};

pub use arrays::*;
pub use compat::*;
pub use encoding::*;
pub use hash::*;
use jrsonnet_evaluator::{
	error::{ErrorKind::*, Result},
	function::{CallLocation, FuncVal, TlaArg},
	tb,
	trace::PathResolver,
	ContextBuilder, IStr, ObjValue, ObjValueBuilder, State, Thunk, Val,
};
use jrsonnet_gcmodule::Trace;
use jrsonnet_parser::Source;
pub use manifest::*;
pub use math::*;
pub use misc::*;
pub use objects::*;
pub use operator::*;
pub use parse::*;
pub use sets::*;
pub use sort::*;
pub use strings::*;
pub use types::*;

#[cfg(feature = "exp-regex")]
pub use crate::regex::*;

mod arrays;
mod compat;
mod encoding;
mod expr;
mod hash;
mod manifest;
mod math;
mod misc;
mod objects;
mod operator;
mod parse;
#[cfg(feature = "exp-regex")]
mod regex;
mod sets;
mod sort;
mod strings;
mod types;

#[allow(clippy::too_many_lines)]
pub fn stdlib_uncached(settings: Rc<RefCell<Settings>>) -> ObjValue {
	let mut builder = ObjValueBuilder::new();

	let expr = expr::stdlib_expr();
	let eval = jrsonnet_evaluator::evaluate(ContextBuilder::dangerous_empty_state().build(), &expr)
		.expect("stdlib.jsonnet should have no errors")
		.as_obj()
		.expect("stdlib.jsonnet should evaluate to object");

	builder.with_super(eval);

	// FIXME: Use PHF
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
		("mapWithIndex", builtin_map_with_index::INST),
		("flatMap", builtin_flatmap::INST),
		("filter", builtin_filter::INST),
		("foldl", builtin_foldl::INST),
		("foldr", builtin_foldr::INST),
		("range", builtin_range::INST),
		("join", builtin_join::INST),
		("lines", builtin_lines::INST),
		("deepJoin", builtin_deep_join::INST),
		("reverse", builtin_reverse::INST),
		("any", builtin_any::INST),
		("all", builtin_all::INST),
		("member", builtin_member::INST),
		("contains", builtin_contains::INST),
		("count", builtin_count::INST),
		("avg", builtin_avg::INST),
		("removeAt", builtin_remove_at::INST),
		("remove", builtin_remove::INST),
		("flattenArrays", builtin_flatten_arrays::INST),
		("flattenDeepArray", builtin_flatten_deep_array::INST),
		("prune", builtin_prune::INST),
		("filterMap", builtin_filter_map::INST),
		// Math
		("abs", builtin_abs::INST),
		("sign", builtin_sign::INST),
		("max", builtin_max::INST),
		("min", builtin_min::INST),
		("clamp", builtin_clamp::INST),
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
		("atan2", builtin_atan2::INST),
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
		("objectFields", builtin_object_fields::INST),
		("objectFieldsAll", builtin_object_fields_all::INST),
		("objectValues", builtin_object_values::INST),
		("objectValuesAll", builtin_object_values_all::INST),
		("objectKeysValues", builtin_object_keys_values::INST),
		("objectKeysValuesAll", builtin_object_keys_values_all::INST),
		("objectHasEx", builtin_object_has_ex::INST),
		("objectHas", builtin_object_has::INST),
		("objectHasAll", builtin_object_has_all::INST),
		("objectRemoveKey", builtin_object_remove_key::INST),
		// Manifest
		("escapeStringJson", builtin_escape_string_json::INST),
		("escapeStringPython", builtin_escape_string_python::INST),
		("escapeStringXML", builtin_escape_string_xml::INST),
		("manifestJsonEx", builtin_manifest_json_ex::INST),
		("manifestJson", builtin_manifest_json::INST),
		("manifestJsonMinified", builtin_manifest_json_minified::INST),
		("manifestYamlDoc", builtin_manifest_yaml_doc::INST),
		("manifestYamlStream", builtin_manifest_yaml_stream::INST),
		("manifestTomlEx", builtin_manifest_toml_ex::INST),
		("manifestToml", builtin_manifest_toml::INST),
		("toString", builtin_to_string::INST),
		("manifestPython", builtin_manifest_python::INST),
		("manifestPythonVars", builtin_manifest_python_vars::INST),
		("manifestXmlJsonml", builtin_manifest_xml_jsonml::INST),
		("manifestIni", builtin_manifest_ini::INST),
		// Parse
		("parseJson", builtin_parse_json::INST),
		("parseYaml", builtin_parse_yaml::INST),
		// Strings
		("codepoint", builtin_codepoint::INST),
		("substr", builtin_substr::INST),
		("char", builtin_char::INST),
		("strReplace", builtin_str_replace::INST),
		("escapeStringBash", builtin_escape_string_bash::INST),
		("escapeStringDollars", builtin_escape_string_dollars::INST),
		("isEmpty", builtin_is_empty::INST),
		("equalsIgnoreCase", builtin_equals_ignore_case::INST),
		("splitLimit", builtin_splitlimit::INST),
		("splitLimitR", builtin_splitlimitr::INST),
		("split", builtin_split::INST),
		("asciiUpper", builtin_ascii_upper::INST),
		("asciiLower", builtin_ascii_lower::INST),
		("findSubstr", builtin_find_substr::INST),
		("parseInt", builtin_parse_int::INST),
		#[cfg(feature = "exp-bigint")]
		("bigint", builtin_bigint::INST),
		("parseOctal", builtin_parse_octal::INST),
		("parseHex", builtin_parse_hex::INST),
		("stringChars", builtin_string_chars::INST),
		("lstripChars", builtin_lstrip_chars::INST),
		("rstripChars", builtin_rstrip_chars::INST),
		("stripChars", builtin_strip_chars::INST),
		// Misc
		("length", builtin_length::INST),
		("get", builtin_get::INST),
		("startsWith", builtin_starts_with::INST),
		("endsWith", builtin_ends_with::INST),
		// Sets
		("setMember", builtin_set_member::INST),
		("setInter", builtin_set_inter::INST),
		("setDiff", builtin_set_diff::INST),
		("setUnion", builtin_set_union::INST),
		// Regex
		#[cfg(feature = "exp-regex")]
		("regexQuoteMeta", builtin_regex_quote_meta::INST),
		// Compat
		("__compare", builtin___compare::INST),
		("__compare_array", builtin___compare_array::INST),
		("__array_less", builtin___array_less::INST),
		("__array_greater", builtin___array_greater::INST),
		("__array_less_or_equal", builtin___array_less_or_equal::INST),
		(
			"__array_greater_or_equal",
			builtin___array_greater_or_equal::INST,
		),
	]
	.iter()
	.copied()
	{
		builder.method(name, builtin);
	}

	builder.method(
		"extVar",
		builtin_ext_var {
			settings: settings.clone(),
		},
	);
	builder.method(
		"native",
		builtin_native {
			settings: settings.clone(),
		},
	);
	builder.method("trace", builtin_trace { settings });
	builder.method("id", FuncVal::Id);

	#[cfg(feature = "exp-regex")]
	{
		// Regex
		let regex_cache = RegexCache::default();
		builder.method(
			"regexFullMatch",
			builtin_regex_full_match {
				cache: regex_cache.clone(),
			},
		);
		builder.method(
			"regexPartialMatch",
			builtin_regex_partial_match {
				cache: regex_cache.clone(),
			},
		);
		builder.method(
			"regexReplace",
			builtin_regex_replace {
				cache: regex_cache.clone(),
			},
		);
		builder.method(
			"regexGlobalReplace",
			builtin_regex_global_replace { cache: regex_cache },
		);
	};

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
				loc.0.source_path().path().map_or_else(
					|| loc.0.source_path().to_string(),
					|p| self.resolver.resolve(p)
				),
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
	pub ext_natives: HashMap<IStr, FuncVal>,
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
	pub fn new(s: State, resolver: PathResolver) -> Self {
		let settings = Settings {
			ext_vars: HashMap::new(),
			ext_natives: HashMap::new(),
			trace_printer: Box::new(StdTracePrinter::new(resolver.clone())),
			path_resolver: resolver,
		};
		let settings = Rc::new(RefCell::new(settings));
		let stdlib_obj = stdlib_uncached(settings.clone());
		#[cfg(not(feature = "legacy-this-file"))]
		let stdlib_thunk = Thunk::evaluated(Val::Obj(stdlib_obj));
		#[cfg(feature = "legacy-this-file")]
		let _ = s;
		Self {
			#[cfg(not(feature = "legacy-this-file"))]
			context: {
				let mut context = ContextBuilder::with_capacity(s, 1);
				context.bind("std", stdlib_thunk.clone());
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
	pub fn add_native(&self, name: impl Into<IStr>, cb: impl Into<FuncVal>) {
		self.settings_mut()
			.ext_natives
			.insert(name.into(), cb.into());
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
		builder.bind("std", self.stdlib_thunk.clone());
	}
	#[cfg(feature = "legacy-this-file")]
	fn populate(&self, source: Source, builder: &mut ContextBuilder) {
		let mut std = ObjValueBuilder::new();
		std.with_super(self.stdlib_obj.clone());
		std.field("thisFile").hide().value({
			let source_path = source.source_path();
			source_path.path().map_or_else(
				|| source_path.to_string(),
				|p| self.settings().path_resolver.resolve(p),
			)
		});
		let stdlib_with_this_file = std.build();

		builder.bind("std", Thunk::evaluated(Val::Obj(stdlib_with_this_file)));
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
		self.settings_mut().context_initializer = tb!(initializer);
	}
}
