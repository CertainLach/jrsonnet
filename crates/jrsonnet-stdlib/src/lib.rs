use std::{
	borrow::Cow,
	cell::{Ref, RefCell, RefMut},
	collections::HashMap,
	rc::Rc,
};

use jrsonnet_evaluator::{
	error::{Error::*, Result},
	function::{builtin::Builtin, ArgLike, CallLocation, FuncVal, TlaArg},
	gc::{GcHashMap, TraceBox},
	tb, throw_runtime,
	typed::{Any, Either, Either2, Either4, VecVal, M1},
	val::{equals, ArrValue},
	Context, ContextBuilder, IStr, ObjValue, ObjValueBuilder, State, Thunk, Val,
};
use jrsonnet_gcmodule::Cc;
use jrsonnet_macros::builtin;
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

pub fn stdlib_uncached(s: State, settings: Rc<RefCell<Settings>>) -> ObjValue {
	let mut builder = ObjValueBuilder::new();

	let expr = expr::stdlib_expr();
	let eval = jrsonnet_evaluator::evaluate(s.clone(), Context::default(), &expr)
		.expect("stdlib.jsonnet should have no errors")
		.as_obj()
		.expect("stdlib.jsonnet should evaluate to object");

	builder.with_super(eval);

	for (name, builtin) in [
		("length".into(), builtin_length::INST),
		// Types
		("type".into(), builtin_type::INST),
		("isString".into(), builtin_is_string::INST),
		("isNumber".into(), builtin_is_number::INST),
		("isBoolean".into(), builtin_is_boolean::INST),
		("isObject".into(), builtin_is_object::INST),
		("isArray".into(), builtin_is_array::INST),
		("isFunction".into(), builtin_is_function::INST),
		// Arrays
		("makeArray".into(), builtin_make_array::INST),
		("slice".into(), builtin_slice::INST),
		("map".into(), builtin_map::INST),
		("flatMap".into(), builtin_flatmap::INST),
		("filter".into(), builtin_filter::INST),
		("foldl".into(), builtin_foldl::INST),
		("foldr".into(), builtin_foldr::INST),
		("range".into(), builtin_range::INST),
		("join".into(), builtin_join::INST),
		("reverse".into(), builtin_reverse::INST),
		("any".into(), builtin_any::INST),
		("all".into(), builtin_all::INST),
		("member".into(), builtin_member::INST),
		("count".into(), builtin_count::INST),
		// Math
		("modulo".into(), builtin_modulo::INST),
		("floor".into(), builtin_floor::INST),
		("ceil".into(), builtin_ceil::INST),
		("log".into(), builtin_log::INST),
		("pow".into(), builtin_pow::INST),
		("sqrt".into(), builtin_sqrt::INST),
		("sin".into(), builtin_sin::INST),
		("cos".into(), builtin_cos::INST),
		("tan".into(), builtin_tan::INST),
		("asin".into(), builtin_asin::INST),
		("acos".into(), builtin_acos::INST),
		("atan".into(), builtin_atan::INST),
		("exp".into(), builtin_exp::INST),
		("mantissa".into(), builtin_mantissa::INST),
		("exponent".into(), builtin_exponent::INST),
		// Operator
		("mod".into(), builtin_mod::INST),
		("primitiveEquals".into(), builtin_primitive_equals::INST),
		("equals".into(), builtin_equals::INST),
		("format".into(), builtin_format::INST),
		// Sort
		("sort".into(), builtin_sort::INST),
		// Hash
		("md5".into(), builtin_md5::INST),
		// Encoding
		("encodeUTF8".into(), builtin_encode_utf8::INST),
		("decodeUTF8".into(), builtin_decode_utf8::INST),
		("base64".into(), builtin_base64::INST),
		("base64Decode".into(), builtin_base64_decode::INST),
		(
			"base64DecodeBytes".into(),
			builtin_base64_decode_bytes::INST,
		),
		// Objects
		("objectFieldsEx".into(), builtin_object_fields_ex::INST),
		("objectHasEx".into(), builtin_object_has_ex::INST),
		// Manifest
		("escapeStringJson".into(), builtin_escape_string_json::INST),
		("manifestJsonEx".into(), builtin_manifest_json_ex::INST),
		("manifestYamlDoc".into(), builtin_manifest_yaml_doc::INST),
		// Parsing
		("parseJson".into(), builtin_parse_json::INST),
		("parseYaml".into(), builtin_parse_yaml::INST),
		// Misc
		("codepoint".into(), builtin_codepoint::INST),
		("substr".into(), builtin_substr::INST),
		("char".into(), builtin_char::INST),
		("strReplace".into(), builtin_str_replace::INST),
		("splitLimit".into(), builtin_splitlimit::INST),
		("asciiUpper".into(), builtin_ascii_upper::INST),
		("asciiLower".into(), builtin_ascii_lower::INST),
		("findSubstr".into(), builtin_find_substr::INST),
		("startsWith".into(), builtin_starts_with::INST),
		("endsWith".into(), builtin_ends_with::INST),
	]
	.iter()
	.cloned()
	{
		builder
			.member(name)
			.hide()
			.value(s.clone(), Val::Func(FuncVal::StaticBuiltin(builtin)))
			.expect("no conflict");
	}

	builder
		.member("extVar".into())
		.hide()
		.value(
			s.clone(),
			Val::Func(FuncVal::Builtin(Cc::new(tb!(builtin_ext_var {
				settings: settings.clone()
			})))),
		)
		.expect("no conflict");
	builder
		.member("native".into())
		.hide()
		.value(
			s.clone(),
			Val::Func(FuncVal::Builtin(Cc::new(tb!(builtin_native {
				settings: settings.clone()
			})))),
		)
		.expect("no conflict");
	builder
		.member("trace".into())
		.hide()
		.value(
			s.clone(),
			Val::Func(FuncVal::Builtin(Cc::new(tb!(builtin_trace { settings })))),
		)
		.expect("no conflict");

	builder
		.member("id".into())
		.hide()
		.value(s, Val::Func(FuncVal::Id))
		.expect("no conflict");

	builder.build()
}

pub trait TracePrinter {
	fn print_trace(&self, s: State, loc: CallLocation, value: IStr);
}

pub struct StdTracePrinter;
impl TracePrinter for StdTracePrinter {
	fn print_trace(&self, _s: State, loc: CallLocation, value: IStr) {
		eprint!("TRACE:");
		if let Some(loc) = loc.0 {
			let locs = loc.0.map_source_locations(&[loc.1]);
			eprint!(" {}:{}", loc.0.short_display(), locs[0].line);
		}
		eprintln!(" {}", value);
	}
}

pub struct Settings {
	/// Used for `std.extVar`
	pub ext_vars: HashMap<IStr, TlaArg>,
	/// Used for `std.native`
	pub ext_natives: HashMap<IStr, Cc<TraceBox<dyn Builtin>>>,
	/// Helper to add globals without implementing custom ContextInitializer
	pub globals: GcHashMap<IStr, Thunk<Val>>,
	/// Used for `std.trace`
	pub trace_printer: Box<dyn TracePrinter>,
}

impl Default for Settings {
	fn default() -> Self {
		Self {
			ext_vars: Default::default(),
			ext_natives: Default::default(),
			globals: Default::default(),
			trace_printer: Box::new(StdTracePrinter),
		}
	}
}

pub fn extvar_source(name: &str, code: impl Into<IStr>) -> Source {
	let source_name = format!("<extvar:{}>", name);
	Source::new_virtual(Cow::Owned(source_name), code.into())
}

pub struct ContextInitializer {
	// When we don't need to support legacy-this-file, we can reuse same context for all files
	#[cfg(not(feature = "legacy-this-file"))]
	context: Context,
	// Otherwise, we can only keep first stdlib layer, and then stack thisFile on top of it
	#[cfg(feature = "legacy-this-file")]
	stdlib_obj: ObjValue,
	settings: Rc<RefCell<Settings>>,
}
impl ContextInitializer {
	pub fn new(s: State) -> Self {
		let settings = Rc::new(RefCell::new(Settings::default()));
		Self {
			#[cfg(not(feature = "legacy-this-file"))]
			context: {
				let mut context = ContextBuilder::with_capacity(1);
				context.bind(
					"std".into(),
					Thunk::evaluated(Val::Obj(stdlib_uncached(s, settings.clone()))),
				);
				context.build()
			},
			#[cfg(feature = "legacy-this-file")]
			stdlib_obj: stdlib_uncached(s, settings.clone()),
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
				file_name: source.clone(),
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
	pub fn add_native(&self, name: IStr, cb: Cc<TraceBox<dyn Builtin>>) {
		self.settings_mut().ext_natives.insert(name, cb);
	}
}
impl jrsonnet_evaluator::ContextInitializer for ContextInitializer {
	#[cfg(not(feature = "legacy-this-file"))]
	fn initialize(&self, _s: State, _source: Source) -> jrsonnet_evaluator::Context {
		let out = self.context.clone();
		let globals = &self.settings().globals;
		if globals.is_empty() {
			return out;
		}

		let mut out = ContextBuilder::extend(out);
		for (k, v) in globals.iter() {
			out.bind(k.clone(), v.clone());
		}
		out.build()
	}
	#[cfg(feature = "legacy-this-file")]
	fn initialize(&self, s: State, source: Source) -> jrsonnet_evaluator::Context {
		let mut builder = ObjValueBuilder::new();
		builder.with_super(self.stdlib_obj.clone());
		builder
			.member("thisFile".into())
			.hide()
			.value(
				s,
				Val::Str(
					source
						.path()
						.map(|p| p.display().to_string())
						.unwrap_or_else(String::new)
						.into(),
				),
			)
			.expect("this object builder is empty");
		let stdlib_with_this_file = builder.build();

		let mut context = ContextBuilder::with_capacity(1);
		context.bind(
			"std".into(),
			Thunk::evaluated(Val::Obj(stdlib_with_this_file)),
		);
		for (k, v) in &self.settings().globals {
			context.bind(k.clone(), v.clone())
		}
		context.build()
	}
	unsafe fn as_any(&self) -> &dyn std::any::Any {
		self
	}
}

#[builtin]
fn builtin_length(x: Either![IStr, ArrValue, ObjValue, FuncVal]) -> Result<usize> {
	use Either4::*;
	Ok(match x {
		A(x) => x.chars().count(),
		B(x) => x.len(),
		C(x) => x.len(),
		D(f) => f.params_len(),
	})
}

#[builtin]
const fn builtin_codepoint(str: char) -> Result<u32> {
	Ok(str as u32)
}

#[builtin]
fn builtin_substr(str: IStr, from: usize, len: usize) -> Result<String> {
	Ok(str.chars().skip(from as usize).take(len as usize).collect())
}

#[builtin(fields(
	settings: Rc<RefCell<Settings>>,
))]
fn builtin_ext_var(this: &builtin_ext_var, s: State, x: IStr) -> Result<Any> {
	let ctx = s.create_default_context(extvar_source(&x, ""));
	Ok(Any(this
		.settings
		.borrow()
		.ext_vars
		.get(&x)
		.cloned()
		.ok_or(UndefinedExternalVariable(x))?
		.evaluate_arg(s.clone(), ctx, true)?
		.evaluate(s)?))
}

#[builtin(fields(
	settings: Rc<RefCell<Settings>>,
))]
fn builtin_native(this: &builtin_native, name: IStr) -> Result<Any> {
	Ok(Any(this
		.settings
		.borrow()
		.ext_natives
		.get(&name)
		.cloned()
		.map_or(Val::Null, |v| {
			Val::Func(FuncVal::Builtin(v.clone()))
		})))
}

#[builtin]
fn builtin_char(n: u32) -> Result<char> {
	Ok(std::char::from_u32(n as u32).ok_or(InvalidUnicodeCodepointGot(n as u32))?)
}

#[builtin(fields(
	settings: Rc<RefCell<Settings>>,
))]
fn builtin_trace(
	this: &builtin_trace,
	s: State,
	loc: CallLocation,
	str: IStr,
	rest: Thunk<Val>,
) -> Result<Any> {
	this.settings
		.borrow()
		.trace_printer
		.print_trace(s.clone(), loc, str);
	Ok(Any(rest.evaluate(s)?))
}

#[builtin]
fn builtin_str_replace(str: String, from: IStr, to: IStr) -> Result<String> {
	Ok(str.replace(&from as &str, &to as &str))
}

#[builtin]
fn builtin_splitlimit(str: IStr, c: IStr, maxsplits: Either![usize, M1]) -> Result<VecVal> {
	use Either2::*;
	Ok(VecVal(Cc::new(match maxsplits {
		A(n) => str
			.splitn(n + 1, &c as &str)
			.map(|s| Val::Str(s.into()))
			.collect(),
		B(_) => str.split(&c as &str).map(|s| Val::Str(s.into())).collect(),
	})))
}

#[builtin]
fn builtin_ascii_upper(str: IStr) -> Result<String> {
	Ok(str.to_ascii_uppercase())
}

#[builtin]
fn builtin_ascii_lower(str: IStr) -> Result<String> {
	Ok(str.to_ascii_lowercase())
}

#[builtin]
fn builtin_find_substr(pat: IStr, str: IStr) -> Result<ArrValue> {
	if pat.is_empty() || str.is_empty() || pat.len() > str.len() {
		return Ok(ArrValue::empty());
	}

	let str = str.as_str();
	let pat = pat.as_bytes();
	let strb = str.as_bytes();

	let max_pos = str.len() - pat.len();

	let mut out: Vec<Val> = Vec::new();
	for (ch_idx, (i, _)) in str
		.char_indices()
		.take_while(|(i, _)| i <= &max_pos)
		.enumerate()
	{
		if &strb[i..i + pat.len()] == pat {
			out.push(Val::Num(ch_idx as f64))
		}
	}
	Ok(out.into())
}

#[builtin]
fn builtin_starts_with(
	s: State,
	a: Either![IStr, ArrValue],
	b: Either![IStr, ArrValue],
) -> Result<bool> {
	Ok(match (a, b) {
		(Either2::A(a), Either2::A(b)) => a.starts_with(b.as_str()),
		(Either2::B(a), Either2::B(b)) => {
			if b.len() > a.len() {
				return Ok(false);
			} else if b.len() == a.len() {
				return equals(s, &Val::Arr(a), &Val::Arr(b));
			} else {
				for (a, b) in a
					.slice(None, Some(b.len()), None)
					.iter(s.clone())
					.zip(b.iter(s.clone()))
				{
					let a = a?;
					let b = b?;
					if !equals(s.clone(), &a, &b)? {
						return Ok(false);
					}
				}
				true
			}
		}
		_ => throw_runtime!("both arguments should be of the same type"),
	})
}

#[builtin]
fn builtin_ends_with(
	s: State,
	a: Either![IStr, ArrValue],
	b: Either![IStr, ArrValue],
) -> Result<bool> {
	Ok(match (a, b) {
		(Either2::A(a), Either2::A(b)) => a.ends_with(b.as_str()),
		(Either2::B(a), Either2::B(b)) => {
			if b.len() > a.len() {
				return Ok(false);
			} else if b.len() == a.len() {
				return equals(s, &Val::Arr(a), &Val::Arr(b));
			} else {
				let a_len = a.len();
				for (a, b) in a
					.slice(Some(a_len - b.len()), None, None)
					.iter(s.clone())
					.zip(b.iter(s.clone()))
				{
					let a = a?;
					let b = b?;
					if !equals(s.clone(), &a, &b)? {
						return Ok(false);
					}
				}
				true
			}
		}
		_ => throw_runtime!("both arguments should be of the same type"),
	})
}

pub trait StateExt {
	/// This method was previously implemented in jrsonnet-evaluator itself
	fn with_stdlib(&self);
	fn add_global(&self, name: IStr, value: Thunk<Val>);
}

impl StateExt for State {
	fn with_stdlib(&self) {
		let initializer = ContextInitializer::new(self.clone());
		self.settings_mut().context_initializer = Box::new(initializer)
	}
	fn add_global(&self, name: IStr, value: Thunk<Val>) {
		// Safety:
		unsafe { self.settings().context_initializer.as_any() }
			.downcast_ref::<ContextInitializer>()
			.expect("not standard context initializer")
			.settings_mut()
			.globals
			.insert(name, value);
	}
}
