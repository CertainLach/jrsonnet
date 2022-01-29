use crate::{
	builtin::{format::FormatError, sort::SortError},
	typed::TypeLocError,
};
use gcmodule::Trace;
use jrsonnet_interner::IStr;
use jrsonnet_parser::{BinaryOpType, ExprLocation, UnaryOpType};
use jrsonnet_types::ValType;
use std::{
	path::{Path, PathBuf},
	rc::Rc,
};
use thiserror::Error;

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

	#[error("assert failed: {0}")]
	AssertionFailed(IStr),

	#[error("variable is not defined: {0}")]
	VariableIsNotDefined(IStr),
	#[error("type mismatch: expected {}, got {2} {0}", .1.iter().map(|e| format!("{}", e)).collect::<Vec<_>>().join(", "))]
	TypeMismatch(&'static str, Vec<ValType>, ValType),
	#[error("no such field: {0}")]
	NoSuchField(IStr),

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
	#[error("native is not defined: {0}")]
	UndefinedExternalFunction(IStr),

	#[error("field name should be string, got {0}")]
	FieldMustBeStringGot(ValType),

	#[error("attempted to index array with string {0}")]
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
	ImportFileNotFound(PathBuf, PathBuf),
	#[error("resolved file not found: {0}")]
	ResolvedFileNotFound(PathBuf),
	#[error("imported file is not valid utf-8: {0:?}")]
	ImportBadFileUtf8(PathBuf),
	#[error("import io error: {0}")]
	ImportIo(String),
	#[error("tried to import {1} from {0}, but imports is not supported")]
	ImportNotSupported(PathBuf, PathBuf),
	#[error(
		"syntax error: expected {}, got {:?}",
		.error.expected,
		.source_code.chars().nth(error.location.offset).map(|c| c.to_string()).unwrap_or_else(|| "EOF".into())
	)]
	ImportSyntaxError {
		#[skip_trace]
		path: Rc<Path>,
		source_code: IStr,
		#[skip_trace]
		error: Box<jrsonnet_parser::ParseError>,
	},

	#[error("runtime error: {0}")]
	RuntimeError(IStr),
	#[error("stack overflow, try to reduce recursion, or set --max-stack to bigger value")]
	StackOverflow,
	#[error("infinite recursion detected")]
	RecursiveLazyValueEvaluation,
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

	#[error("{0}")]
	ImportCallbackError(String),
	#[error("invalid unicode codepoint: {0}")]
	InvalidUnicodeCodepointGot(u32),

	#[error("format error: {0}")]
	Format(#[from] FormatError),
	#[error("type error: {0}")]
	TypeError(TypeLocError),
	#[error("sort error: {0}")]
	Sort(#[from] SortError),

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

#[derive(Debug, Clone, Trace)]
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

pub type Result<V> = std::result::Result<V, LocError>;

#[macro_export]
macro_rules! throw {
	($e: expr) => {
		return Err($e.into())
	};
}
