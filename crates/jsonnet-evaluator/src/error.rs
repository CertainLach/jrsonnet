use crate::ValType;
use jsonnet_parser::ExprLocation;
use std::{path::PathBuf, rc::Rc};

#[derive(Debug, Clone)]
pub enum Error {
	VariableIsNotDefined(String),
	TypeMismatch(&'static str, Vec<ValType>, ValType),
	NoSuchField(String),

	UnknownVariable(String),

	UnknownFunctionParameter(String),
	BindingParameterASecondTime(String),
	TooManyArgsFunctionHas(usize),
	FunctionParameterNotBoundInCall(String),

	UndefinedExternalVariable(String),

	FieldMustBeStringGot(ValType),

	AttemptedIndexAnArrayWithString(String),
	ValueIndexMustBeTypeGot(ValType, ValType, ValType),
	CantIndexInto(ValType),

	StandaloneSuper,

	ImportFileNotFound(PathBuf, PathBuf),
	ResolvedFileNotFound(PathBuf),
	ImportBadFileUtf8(PathBuf),
	ImportNotSupported(PathBuf, PathBuf),
	ImportSyntaxError(jsonnet_parser::ParseError),

	RuntimeError(String),
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
