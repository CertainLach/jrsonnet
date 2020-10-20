use crate::{
	builtin::{format::FormatError, sort::SortError},
	ValType,
};
use jrsonnet_parser::{BinaryOpType, ExprLocation, GcStr, UnaryOpType};
use std::{path::PathBuf, rc::Rc};
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum Error {
	#[error("intrinsic not found: {0}.{1}")]
	IntrinsicNotFound(GcStr, GcStr),
	#[error("argument reordering in intrisics not supported yet")]
	IntrinsicArgumentReorderingIsNotSupportedYet,

	#[error("operator {0} does not operate on type {1}")]
	UnaryOperatorDoesNotOperateOnType(UnaryOpType, ValType),
	#[error("binary operation {1} {0} {2} is not implemented")]
	BinaryOperatorDoesNotOperateOnValues(BinaryOpType, ValType, ValType),

	#[error("no top level object in this context")]
	NoTopLevelObjectFound,
	#[error("self is only usable inside objects")]
	CantUseSelfOutsideOfObject,
	#[error("super is only usable inside objects")]
	CantUseSuperOutsideOfObject,

	#[error("for loop can only iterate over arrays")]
	InComprehensionCanOnlyIterateOverArray,

	#[error("array out of bounds: {0} is not within [0,{1})")]
	ArrayBoundsError(usize, usize),

	#[error("assert failed: {0}")]
	AssertionFailed(GcStr),

	#[error("variable is not defined: {0}")]
	VariableIsNotDefined(GcStr),
	#[error("type mismatch: expected {}, got {2} {0}", .1.iter().map(|e| format!("{}", e)).collect::<Vec<_>>().join(", "))]
	TypeMismatch(&'static str, Vec<ValType>, ValType),
	#[error("no such field: {0}")]
	NoSuchField(GcStr),

	#[error("only functions can be called, got {0}")]
	OnlyFunctionsCanBeCalledGot(ValType),
	#[error("parameter {0} is not defined")]
	UnknownFunctionParameter(String),
	#[error("argument {0} is already bound")]
	BindingParameterASecondTime(GcStr),
	#[error("too many args, function has {0}")]
	TooManyArgsFunctionHas(usize),
	#[error("founction argument is not passed: {0}")]
	FunctionParameterNotBoundInCall(GcStr),

	#[error("external variable is not defined: {0}")]
	UndefinedExternalVariable(GcStr),
	#[error("native is not defined: {0}")]
	UndefinedExternalFunction(GcStr),

	#[error("field name should be string, got {0}")]
	FieldMustBeStringGot(ValType),

	#[error("attempted to index array with string {0}")]
	AttemptedIndexAnArrayWithString(GcStr),
	#[error("{0} index type should be {1}, got {2}")]
	ValueIndexMustBeTypeGot(ValType, ValType, ValType),
	#[error("cant index into {0}")]
	CantIndexInto(ValType),

	#[error("super can't be used standalone")]
	StandaloneSuper,

	#[error("can't resolve {1} from {0}")]
	ImportFileNotFound(PathBuf, PathBuf),
	#[error("resolved file not found: {0}")]
	ResolvedFileNotFound(PathBuf),
	#[error("imported file is not valid utf-8: {0:?}")]
	ImportBadFileUtf8(PathBuf),
	#[error("tried to import {1} from {0}, but imports is not supported")]
	ImportNotSupported(PathBuf, PathBuf),
	#[error("syntax error")]
	ImportSyntaxError {
		path: Rc<PathBuf>,
		source_code: GcStr,
		error: Box<jrsonnet_parser::ParseError>,
	},

	#[error("runtime error: {0}")]
	RuntimeError(GcStr),
	#[error("stack overflow, try to reduce recursion, or set --max-stack to bigger value")]
	StackOverflow,
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
	#[error("sort error: {0}")]
	Sort(#[from] SortError),
}
impl From<Error> for LocError {
	fn from(e: Error) -> Self {
		Self::new(e)
	}
}

#[derive(Clone, Debug)]
pub struct StackTraceElement {
	pub location: ExprLocation,
	pub desc: String,
}
#[derive(Debug, Clone)]
pub struct StackTrace(pub Vec<StackTraceElement>);

#[derive(Debug, Clone)]
pub struct LocError(Box<(Error, StackTrace)>);
impl LocError {
	pub fn new(e: Error) -> Self {
		Self(Box::new((e, StackTrace(vec![]))))
	}

	pub const fn error(&self) -> &Error {
		&(self.0).0
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
		return Err($e.into());
	};
}
