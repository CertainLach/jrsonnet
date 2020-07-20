use crate::{Val, ValType};
use jrsonnet_parser::{BinaryOpType, ExprLocation, UnaryOpType};
use std::{path::PathBuf, rc::Rc};

#[derive(Debug, Clone)]
pub enum Error {
	IntristicNotFound(Rc<str>, Rc<str>),
	IntristicArgumentReorderingIsNotSupportedYet,

	UnaryOperatorDoesNotOperateOnType(UnaryOpType, ValType),
	BinaryOperatorDoesNotOperateOnValues(BinaryOpType, ValType, ValType),

	NoTopLevelObjectFound,
	CantUseSelfOutsideOfObject,
	CantUseSuperOutsideOfObject,

	InComprehensionCanOnlyIterateOverArray,

	ArrayBoundsError(usize, usize),

	AssertionFailed(Val),

	VariableIsNotDefined(String),
	TypeMismatch(&'static str, Vec<ValType>, ValType),
	NoSuchField(Rc<str>),

	UnknownVariable(Rc<str>),

	OnlyFunctionsCanBeCalledGot(ValType),
	UnknownFunctionParameter(String),
	BindingParameterASecondTime(Rc<str>),
	TooManyArgsFunctionHas(usize),
	FunctionParameterNotBoundInCall(Rc<str>),

	UndefinedExternalVariable(Rc<str>),

	FieldMustBeStringGot(ValType),

	AttemptedIndexAnArrayWithString(Rc<str>),
	ValueIndexMustBeTypeGot(ValType, ValType, ValType),
	CantIndexInto(ValType),

	StandaloneSuper,

	ImportFileNotFound(PathBuf, PathBuf),
	ResolvedFileNotFound(PathBuf),
	ImportBadFileUtf8(PathBuf),
	ImportNotSupported(PathBuf, PathBuf),
	ImportSyntaxError {
		path: Rc<PathBuf>,
		source_code: Rc<str>,
		error: jrsonnet_parser::ParseError,
	},

	RuntimeError(Rc<str>),
	StackOverflow,
	FractionalIndex,
	DivisionByZero,

	StringManifestOutputIsNotAString,

	ImportCallbackError(String),
	InvalidUnicodeCodepointGot(u32),
}

#[derive(Clone, Debug)]
pub struct StackTraceElement {
	pub location: ExprLocation,
	pub desc: String,
}
#[derive(Debug, Clone)]
pub struct StackTrace(pub Vec<StackTraceElement>);

#[derive(Debug, Clone)]
pub struct LocError(pub Error, pub StackTrace);
pub type Result<V> = std::result::Result<V, LocError>;
