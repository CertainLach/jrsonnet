use std::fmt;
use std::ops::Deref;
use std::rc::Rc;

use jrsonnet_gcmodule::Acyclic;
use jrsonnet_interner::IStr;

#[derive(Clone, Acyclic, Debug, PartialEq, Eq)]
pub struct ParamName(pub Option<IStr>);
impl ParamName {
	pub const ANONYMOUS: Self = Self(None);
	pub fn new(name: IStr) -> Self {
		Self(Some(name))
	}
	pub fn as_str(&self) -> Option<&str> {
		self.0.as_deref()
	}
	pub fn is_anonymous(&self) -> bool {
		self.0.is_none()
	}
}
impl PartialEq<IStr> for ParamName {
	fn eq(&self, other: &IStr) -> bool {
		self.0
			.as_ref()
			.map_or(false, |s| s.as_bytes() == other.as_bytes())
	}
}

impl fmt::Display for ParamName {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match &self.0 {
			Some(v) => write!(f, "{v}"),
			None => write!(f, "<unnamed>"),
		}
	}
}

#[derive(Clone, Copy, Debug, Acyclic, PartialEq, Eq)]
pub enum ParamDefault {
	None,
	Exists,
	Literal(&'static str),
}
impl ParamDefault {
	pub const fn exists(is_exists: bool) -> Self {
		if is_exists {
			Self::Exists
		} else {
			Self::None
		}
	}
}
impl fmt::Display for ParamDefault {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			ParamDefault::None => Ok(()),
			ParamDefault::Exists => write!(f, " = <default>"),
			ParamDefault::Literal(lit) => write!(f, " = {lit}"),
		}
	}
}

#[derive(Clone, Acyclic, Debug, PartialEq, Eq)]
pub struct ParamParse {
	name: ParamName,
	default: ParamDefault,
}
impl ParamParse {
	pub fn new(name: ParamName, default: ParamDefault) -> Self {
		Self { name, default }
	}
	/// Parameter name for named call parsing
	pub fn name(&self) -> &ParamName {
		&self.name
	}
	pub fn default(&self) -> ParamDefault {
		self.default
	}
	pub fn has_default(&self) -> bool {
		!matches!(self.default, ParamDefault::None)
	}
}
impl fmt::Display for ParamParse {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}{}", self.name, self.default)
	}
}

#[derive(Debug, Clone, Acyclic, PartialEq, Eq)]
pub struct FunctionSignature(Rc<[ParamParse]>);
impl Deref for FunctionSignature {
	type Target = [ParamParse];

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

thread_local! {
	static EMPTY_SIGNATURE: FunctionSignature = FunctionSignature::new([].into());
}

impl FunctionSignature {
	pub fn new(v: Rc<[ParamParse]>) -> Self {
		Self(v)
	}
	pub fn empty() -> Self {
		EMPTY_SIGNATURE.with(|p| p.clone())
	}
}
impl fmt::Display for FunctionSignature {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if self.0.is_empty() {
			return write!(f, "(/*no arguments*/)");
		}
		write!(f, "(")?;
		for (i, par) in self.0.iter().enumerate() {
			if i != 0 {
				write!(f, ", ")?;
			}
			write!(f, "{par}")?;
		}
		write!(f, ")")
	}
}
