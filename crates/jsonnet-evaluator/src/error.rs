use crate::ValType;
use jsonnet_parser::{BinaryOpType, ExprLocation, UnaryOpType};
use std::{path::PathBuf, rc::Rc};

#[derive(Debug, Clone)]
pub enum Error {
	IntristicNotFound(Rc<str>, Rc<str>),
	IntristicArgumentReorderingIsNotSupportedYet,

	UnaryOperatorDoesNotOperateOnType(UnaryOpType, ValType),
	BinaryOperatorDoesNotOperateOnValues(BinaryOpType, ValType, ValType),

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
