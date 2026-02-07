use std::{cell::RefCell, num::NonZeroUsize, rc::Rc};

use ::regex::Regex;
use jrsonnet_evaluator::{
	error::{ErrorKind::*, Result},
	rustc_hash::FxBuildHasher,
	val::StrValue,
	IStr, ObjValueBuilder, Val,
};
use jrsonnet_gcmodule::Acyclic;
use jrsonnet_macros::builtin;
use lru::LruCache;

#[derive(Acyclic)]
pub struct RegexCacheInner {
	cache: RefCell<LruCache<IStr, Rc<Regex>, FxBuildHasher>>,
}
impl Default for RegexCacheInner {
	fn default() -> Self {
		Self {
			cache: RefCell::new(LruCache::with_hasher(
				NonZeroUsize::new(20).unwrap(),
				FxBuildHasher::default(),
			)),
		}
	}
}
pub type RegexCache = Rc<RegexCacheInner>;
impl RegexCacheInner {
	fn parse(&self, pattern: IStr) -> Result<Rc<Regex>> {
		let mut cache = self.cache.borrow_mut();
		if let Some(found) = cache.get(&pattern) {
			return Ok(found.clone());
		}
		let regex = Regex::new(&pattern)
			.map_err(|e| RuntimeError(format!("regex parse failed: {e}").into()))?;
		let regex = Rc::new(regex);
		cache.push(pattern, regex.clone());
		Ok(regex)
	}
}

pub fn regex_match_inner(regex: &Regex, str: String) -> Result<Val> {
	let mut out = ObjValueBuilder::with_capacity(3);

	let mut captures = Vec::with_capacity(regex.captures_len());
	let mut named_captures = ObjValueBuilder::with_capacity(regex.capture_names().len());

	let Some(captured) = regex.captures(&str) else {
		return Ok(Val::Null);
	};

	for ele in captured.iter().skip(1) {
		if let Some(ele) = ele {
			captures.push(Val::Str(StrValue::Flat(ele.as_str().into())));
		} else {
			captures.push(Val::Str(StrValue::Flat(IStr::empty())));
		}
	}
	for (i, name) in regex
		.capture_names()
		.skip(1)
		.enumerate()
		.filter_map(|(i, v)| Some((i, v?)))
	{
		let capture = captures[i].clone();
		named_captures.field(name).try_value(capture)?;
	}

	out.field("string")
		.value(Val::Str(captured.get(0).unwrap().as_str().into()));
	out.field("captures").value(Val::Arr(captures.into()));
	out.field("namedCaptures")
		.value(Val::Obj(named_captures.build()));

	Ok(Val::Obj(out.build()))
}

#[builtin(fields(
    cache: RegexCache,
))]
pub fn builtin_regex_partial_match(
	this: &builtin_regex_partial_match,
	pattern: IStr,
	str: String,
) -> Result<Val> {
	let regex = this.cache.parse(pattern)?;
	regex_match_inner(&regex, str)
}

#[builtin(fields(
    cache: RegexCache,
))]
pub fn builtin_regex_full_match(
	this: &builtin_regex_full_match,
	pattern: StrValue,
	str: String,
) -> Result<Val> {
	let pattern = format!("^{pattern}$").into();
	let regex = this.cache.parse(pattern)?;
	regex_match_inner(&regex, str)
}

#[builtin]
pub fn builtin_regex_quote_meta(pattern: String) -> String {
	regex::escape(&pattern)
}

#[builtin(fields(
    cache: RegexCache,
))]
pub fn builtin_regex_replace(
	this: &builtin_regex_replace,
	str: String,
	pattern: IStr,
	to: String,
) -> Result<String> {
	let regex = this.cache.parse(pattern)?;
	let replaced = regex.replace(&str, to);
	Ok(replaced.to_string())
}

#[builtin(fields(
    cache: RegexCache,
))]
pub fn builtin_regex_global_replace(
	this: &builtin_regex_global_replace,
	str: String,
	pattern: IStr,
	to: String,
) -> Result<String> {
	let regex = this.cache.parse(pattern)?;
	let replaced = regex.replace_all(&str, to);
	Ok(replaced.to_string())
}
