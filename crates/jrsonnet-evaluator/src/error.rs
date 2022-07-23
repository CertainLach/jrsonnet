use std::{fmt::Debug, path::PathBuf};

use jrsonnet_gcmodule::Trace;
use jrsonnet_interner::IStr;
use jrsonnet_parser::{BinaryOpType, ExprLocation, Source, UnaryOpType};
use jrsonnet_types::ValType;
use thiserror::Error;

use crate::{stdlib::format::FormatError, typed::TypeLocError};

fn format_found(list: &[IStr], what: &str) -> String {
	if list.is_empty() {
		return String::new();
	}
	let mut out = String::new();
	out.push_str("\nThere is ");
	out.push_str(what);
	if list.len() > 1 {
		out.push('s');
	}
	out.push_str(" with similar name");
	if list.len() > 1 {
		out.push('s');
	}
	out.push_str(" present: ");
	for (i, v) in list.iter().enumerate() {
		if i != 0 {
			out.push_str(", ");
		}
		out.push_str(v as &str);
	}
	out
}

const fn format_empty_str(str: &str) -> &str {
	if str.is_empty() {
		"\"\" (empty string)"
	} else {
		str
	}
}

#[derive(Error, Debug, Clone, Trace)]
pub enum Error {
	#[error("intrinsic not found: {0}")]
	IntrinsicNotFound(IStr),

	#[error("operator {0} does not operate on type {1}")]
	UnaryOperatorDoesNotOperateOnType(UnaryOpType, ValType),
	#[error("binary operation {1} {0} {2} is not implemented")]
	BinaryOperatorDoesNotOperateOnValues(BinaryOpType, ValType, ValType),

	#[error("no top level object in this context")]
	NoTopLevelObjectFound,
	#[error("self is only usable inside objects")]
	CantUseSelfOutsideOfObject,
	#[error("no super found")]
	NoSuperFound,

	#[error("for loop can only iterate over arrays")]
	InComprehensionCanOnlyIterateOverArray,

	#[error("array out of bounds: {0} is not within [0,{1})")]
	ArrayBoundsError(usize, usize),
	#[error("string out of bounds: {0} is not within [0,{1})")]
	StringBoundsError(usize, usize),

	#[error("assert failed: {}", format_empty_str(.0))]
	AssertionFailed(IStr),

	#[error("variable is not defined: {0}{}", format_found(.1, "variable"))]
	VariableIsNotDefined(IStr, Vec<IStr>),
	#[error("duplicate local var: {0}")]
	DuplicateLocalVar(IStr),

	#[error("type mismatch: expected {}, got {2} {0}", .1.iter().map(|e| format!("{}", e)).collect::<Vec<_>>().join(", "))]
	TypeMismatch(&'static str, Vec<ValType>, ValType),
	#[error("no such field: {}{}", format_empty_str(.0), format_found(.1, "field"))]
	NoSuchField(IStr, Vec<IStr>),

	#[error("only functions can be called, got {0}")]
	OnlyFunctionsCanBeCalledGot(ValType),
	#[error("parameter {0} is not defined")]
	UnknownFunctionParameter(String),
	#[error("argument {0} is already bound")]
	BindingParameterASecondTime(IStr),
	#[error("too many args, function has {0}")]
	TooManyArgsFunctionHas(usize),
	#[error("function argument is not passed: {0}")]
	FunctionParameterNotBoundInCall(IStr),

	#[error("external variable is not defined: {0}")]
	UndefinedExternalVariable(IStr),

	#[error("field name should be string, got {0}")]
	FieldMustBeStringGot(ValType),
	#[error("duplicate field name: {}", format_empty_str(.0))]
	DuplicateFieldName(IStr),

	#[error("attempted to index array with string {}", format_empty_str(.0))]
	AttemptedIndexAnArrayWithString(IStr),
	#[error("{0} index type should be {1}, got {2}")]
	ValueIndexMustBeTypeGot(ValType, ValType, ValType),
	#[error("cant index into {0}")]
	CantIndexInto(ValType),
	#[error("{0} is not indexable")]
	ValueIsNotIndexable(ValType),

	#[error("super can't be used standalone")]
	StandaloneSuper,

	#[error("can't resolve {1} from {0}")]
	ImportFileNotFound(PathBuf, String),
	#[error("resolved file not found: {0}")]
	ResolvedFileNotFound(PathBuf),
	#[error("imported file is not valid utf-8: {0:?}")]
	ImportBadFileUtf8(PathBuf),
	#[error("import io error: {0}")]
	ImportIo(String),
	#[error("tried to import {1} from {0}, but imports is not supported")]
	ImportNotSupported(PathBuf, PathBuf),
	#[error("can't import from virtual file")]
	CantImportFromVirtualFile,
	#[error(
		"syntax error: expected {}, got {:?}",
		.error.expected,
		.source_code.chars().nth(error.location.offset)
		.map_or_else(|| "EOF".into(), |c| c.to_string())
	)]
	ImportSyntaxError {
		path: Source,
		source_code: IStr,
		#[trace(skip)]
		error: Box<jrsonnet_parser::ParseError>,
	},

	#[error("runtime error: {}", format_empty_str(.0))]
	RuntimeError(IStr),
	#[error("stack overflow, try to reduce recursion, or set --max-stack to bigger value")]
	StackOverflow,
	#[error("infinite recursion detected")]
	InfiniteRecursionDetected,
	#[error("tried to index by fractional value")]
	FractionalIndex,
	#[error("attempted to divide by zero")]
	DivisionByZero,

	#[error("string manifest output is not an string")]
	StringManifestOutputIsNotAString,
	#[error("stream manifest output is not an array")]
	StreamManifestOutputIsNotAArray,
	#[error("multi manifest output is not an object")]
	MultiManifestOutputIsNotAObject,

	#[error("cant recurse stream manifest")]
	StreamManifestOutputCannotBeRecursed,
	#[error("stream manifest output cannot consist of raw strings")]
	StreamManifestCannotNestString,

	#[error("{}", format_empty_str(.0))]
	ImportCallbackError(String),
	#[error("invalid unicode codepoint: {0}")]
	InvalidUnicodeCodepointGot(u32),

	#[error("format error: {0}")]
	Format(#[from] FormatError),
	#[error("type error: {0}")]
	TypeError(TypeLocError),

	#[cfg(feature = "anyhow-error")]
	#[error(transparent)]
	Other(Rc<anyhow::Error>),
}

#[cfg(feature = "anyhow-error")]
impl From<anyhow::Error> for LocError {
	fn from(e: anyhow::Error) -> Self {
		Self::new(Error::Other(Rc::new(e)))
	}
}

impl From<Error> for LocError {
	fn from(e: Error) -> Self {
		Self::new(e)
	}
}

#[derive(Clone, Debug, Trace)]
pub struct StackTraceElement {
	pub location: Option<ExprLocation>,
	pub desc: String,
}
#[derive(Debug, Clone, Trace)]
pub struct StackTrace(pub Vec<StackTraceElement>);

#[derive(Clone, Trace)]
pub struct LocError(Box<(Error, StackTrace)>);
impl LocError {
	pub fn new(e: Error) -> Self {
		Self(Box::new((e, StackTrace(vec![]))))
	}

	pub const fn error(&self) -> &Error {
		&(self.0).0
	}
	pub fn error_mut(&mut self) -> &mut Error {
		&mut (self.0).0
	}
	pub const fn trace(&self) -> &StackTrace {
		&(self.0).1
	}
	pub fn trace_mut(&mut self) -> &mut StackTrace {
		&mut (self.0).1
	}
}
impl Debug for LocError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		writeln!(f, "{}", self.0 .0)?;
		for el in &self.0 .1 .0 {
			writeln!(f, "\t{:?}", el)?;
		}
		Ok(())
	}
}

pub type Result<V, E = LocError> = std::result::Result<V, E>;

#[macro_export]
macro_rules! throw {
	($e: expr) => {
		return Err($e.into())
	};
}

#[macro_export]
macro_rules! throw_runtime {
	($($tt:tt)*) => {
		return Err($crate::error::Error::RuntimeError(format!($($tt)*).into()).into())
	};
}
