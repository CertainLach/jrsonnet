use jrsonnet_interner::IStr;
#[cfg(feature = "deserialize")]
use serde::Deserialize;
#[cfg(feature = "serialize")]
use serde::Serialize;
use std::{
	fmt::{Debug, Display},
	ops::Deref,
	path::PathBuf,
	rc::Rc,
};

#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[derive(Debug, PartialEq)]
pub enum FieldName {
	/// {fixed: 2}
	Fixed(IStr),
	/// {["dyn"+"amic"]: 3}
	Dyn(LocExpr),
}

#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Visibility {
	/// :
	Normal,
	/// ::
	Hidden,
	/// :::
	Unhide,
}

impl Visibility {
	pub fn is_visible(&self) -> bool {
		matches!(self, Self::Normal | Self::Unhide)
	}
}

#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[derive(Debug, PartialEq)]
pub struct AssertStmt(pub LocExpr, pub Option<LocExpr>);

#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[derive(Debug, PartialEq)]
pub struct FieldMember {
	pub name: FieldName,
	pub plus: bool,
	pub params: Option<ParamsDesc>,
	pub visibility: Visibility,
	pub value: LocExpr,
}

#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[derive(Debug, PartialEq)]
pub enum Member {
	Field(FieldMember),
	BindStmt(BindSpec),
	AssertStmt(AssertStmt),
}

#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOpType {
	Plus,
	Minus,
	BitNot,
	Not,
}
impl Display for UnaryOpType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		use UnaryOpType::*;
		write!(
			f,
			"{}",
			match self {
				Plus => "+",
				Minus => "-",
				BitNot => "~",
				Not => "!",
			}
		)
	}
}

#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinaryOpType {
	Mul,
	Div,

	/// Implemented as intrinsic, put here for completeness
	Mod,

	Add,
	Sub,

	Lhs,
	Rhs,

	Lt,
	Gt,
	Lte,
	Gte,

	BitAnd,
	BitOr,
	BitXor,

	And,
	Or,
}
impl Display for BinaryOpType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		use BinaryOpType::*;
		write!(
			f,
			"{}",
			match self {
				Mul => "*",
				Div => "/",
				Mod => "%",
				Add => "+",
				Sub => "-",
				Lhs => "<<",
				Rhs => ">>",
				Lt => "<",
				Gt => ">",
				Lte => "<=",
				Gte => ">=",
				BitAnd => "&",
				BitOr => "|",
				BitXor => "^",
				And => "&&",
				Or => "||",
			}
		)
	}
}

/// name, default value
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[derive(Debug, PartialEq)]
pub struct Param(pub IStr, pub Option<LocExpr>);

/// Defined function parameters
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct ParamsDesc(pub Rc<Vec<Param>>);
impl Deref for ParamsDesc {
	type Target = Vec<Param>;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[derive(Debug, PartialEq)]
pub struct Arg(pub Option<String>, pub LocExpr);

#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[derive(Debug, PartialEq)]
pub struct ArgsDesc(pub Vec<Arg>);
impl Deref for ArgsDesc {
	type Target = Vec<Arg>;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct BindSpec {
	pub name: IStr,
	pub params: Option<ParamsDesc>,
	pub value: LocExpr,
}

#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[derive(Debug, PartialEq)]
pub struct IfSpecData(pub LocExpr);

#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[derive(Debug, PartialEq)]
pub struct ForSpecData(pub IStr, pub LocExpr);

#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[derive(Debug, PartialEq)]
pub enum CompSpec {
	IfSpec(IfSpecData),
	ForSpec(ForSpecData),
}

#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[derive(Debug, PartialEq)]
pub struct ObjComp {
	pub pre_locals: Vec<BindSpec>,
	pub key: LocExpr,
	pub value: LocExpr,
	pub post_locals: Vec<BindSpec>,
	pub compspecs: Vec<CompSpec>,
}

#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[derive(Debug, PartialEq)]
pub enum ObjBody {
	MemberList(Vec<Member>),
	ObjComp(ObjComp),
}

#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum LiteralType {
	This,
	Super,
	Dollar,
	Null,
	True,
	False,
}

#[derive(Debug, PartialEq)]
pub struct SliceDesc {
	pub start: Option<LocExpr>,
	pub end: Option<LocExpr>,
	pub step: Option<LocExpr>,
}

/// Syntax base
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[derive(Debug, PartialEq)]
pub enum Expr {
	Literal(LiteralType),

	/// String value: "hello"
	Str(IStr),
	/// Number: 1, 2.0, 2e+20
	Num(f64),
	/// Variable name: test
	Var(IStr),

	/// Array of expressions: [1, 2, "Hello"]
	Arr(Vec<LocExpr>),
	/// Array comprehension:
	/// ```jsonnet
	///  ingredients: [
	///    { kind: kind, qty: 4 / 3 }
	///    for kind in [
	///      'Honey Syrup',
	///      'Lemon Juice',
	///      'Farmers Gin',
	///    ]
	///  ],
	/// ```
	ArrComp(LocExpr, Vec<CompSpec>),

	/// Object: {a: 2}
	Obj(ObjBody),
	/// Object extension: var1 {b: 2}
	ObjExtend(LocExpr, ObjBody),

	/// (obj)
	Parened(LocExpr),

	/// -2
	UnaryOp(UnaryOpType, LocExpr),
	/// 2 - 2
	BinaryOp(LocExpr, BinaryOpType, LocExpr),
	/// assert 2 == 2 : "Math is broken"
	AssertExpr(AssertStmt, LocExpr),
	/// local a = 2; { b: a }
	LocalExpr(Vec<BindSpec>, LocExpr),

	/// import "hello"
	Import(PathBuf),
	/// importStr "file.txt"
	ImportStr(PathBuf),
	/// error "I'm broken"
	ErrorStmt(LocExpr),
	/// a(b, c)
	Apply(LocExpr, ArgsDesc, bool),
	/// a[b]
	Index(LocExpr, LocExpr),
	/// function(x) x
	Function(ParamsDesc, LocExpr),
	/// std.primitiveEquals
	Intrinsic(IStr),
	/// if true == false then 1 else 2
	IfElse {
		cond: IfSpecData,
		cond_then: LocExpr,
		cond_else: Option<LocExpr>,
	},
}

/// file, begin offset, end offset
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[derive(Clone, PartialEq)]
pub struct ExprLocation(pub Rc<PathBuf>, pub usize, pub usize);
impl Debug for ExprLocation {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{:?}:{:?}-{:?}", self.0, self.1, self.2)
	}
}

/// Holds AST expression and its location in source file
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[derive(Clone, PartialEq)]
pub struct LocExpr(pub Rc<Expr>, pub Option<ExprLocation>);
impl Debug for LocExpr {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{:?} from {:?}", self.0, self.1)
	}
}

/// Creates LocExpr from Expr and ExprLocation components
#[macro_export]
macro_rules! loc_expr {
	($expr:expr, $need_loc:expr,($name:expr, $start:expr, $end:expr)) => {
		LocExpr(
			std::rc::Rc::new($expr),
			if $need_loc {
				Some(ExprLocation($name, $start, $end))
			} else {
				None
			},
		)
	};
}

/// Creates LocExpr without location info
#[macro_export]
macro_rules! loc_expr_todo {
	($expr:expr) => {
		LocExpr(Rc::new($expr), None)
	};
}
