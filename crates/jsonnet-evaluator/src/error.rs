use crate::ValType;
use jsonnet_parser::ExprLocation;
use std::{path::PathBuf, rc::Rc};

#[derive(Debug, Clone)]
pub enum Error {
	VariableIsNotDefined(String),
	TypeMismatch(&'static str, Vec<ValType>, ValType),
	IntristicArgumentReorderingIsNotSupportedYet,
	NoSuchField(Rc<str>),

	UnknownVariable(Rc<str>),

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
	ImportSyntaxError(jsonnet_parser::ParseError),

	RuntimeError(Rc<str>),
	StackOverflow,
	FractionalIndex,
	DivisionByZero,
}

#[derive(Clone, Debug)]
pub struct StackTraceElement(pub ExprLocation, pub String);
#[derive(Debug, Clone)]
pub struct StackTrace(pub Vec<StackTraceElement>);

#[derive(Debug, Clone)]
pub struct LocError(pub Error, pub StackTrace);
pub type Result<V> = std::result::Result<V, LocError>;
