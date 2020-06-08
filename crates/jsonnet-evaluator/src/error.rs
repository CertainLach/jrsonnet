use crate::ValType;
use jsonnet_parser::LocExpr;

#[derive(Debug)]
pub enum Error {
	VariableIsNotDefined(String),
	TypeMismatch(&'static str, Vec<ValType>, ValType),
	NoSuchField(String),

	RuntimeError(String),
	StackOverflow,
	FractionalIndex,
	DivisionByZero,
}

#[derive(Clone, Debug)]
pub struct StackTraceElement(pub LocExpr, pub String);
#[derive(Debug)]
pub struct StackTrace(pub Vec<StackTraceElement>);

#[derive(Debug)]
pub struct LocError(pub Error, pub StackTrace);
pub type Result<V> = std::result::Result<V, LocError>;
