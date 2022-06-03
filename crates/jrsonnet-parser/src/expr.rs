use std::{
	fmt::{self, Debug, Display},
	ops::Deref,
	rc::Rc,
};

use gcmodule::Trace;
use jrsonnet_interner::IStr;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::source::Source;

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Trace)]
pub enum FieldName {
	/// {fixed: 2}
	Fixed(IStr),
	/// {["dyn"+"amic"]: 3}
	Dyn(LocExpr),
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Trace)]
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

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq, Trace)]
pub struct AssertStmt(pub LocExpr, pub Option<LocExpr>);

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Trace)]
pub struct FieldMember {
	pub name: FieldName,
	pub plus: bool,
	pub params: Option<ParamsDesc>,
	pub visibility: Visibility,
	pub value: LocExpr,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Trace)]
pub enum Member {
	Field(FieldMember),
	BindStmt(BindSpec),
	AssertStmt(AssertStmt),
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Trace)]
pub enum UnaryOpType {
	Plus,
	Minus,
	BitNot,
	Not,
}

impl Display for UnaryOpType {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Trace)]
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

	Eq,
	Neq,

	And,
	Or,

	// Equialent to std.objectHasEx(a, b, true)
	In,
}

impl Display for BinaryOpType {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
				Eq => "==",
				Neq => "!=",
				And => "&&",
				Or => "||",
				In => "in",
			}
		)
	}
}

/// name, default value
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Trace)]
pub struct Param(pub Destruct, pub Option<LocExpr>);

/// Defined function parameters
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Trace)]
pub struct ParamsDesc(pub Rc<Vec<Param>>);

impl Deref for ParamsDesc {
	type Target = Vec<Param>;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Trace)]
pub struct ArgsDesc {
	pub unnamed: Vec<LocExpr>,
	pub named: Vec<(IStr, LocExpr)>,
}
impl ArgsDesc {
	pub fn new(unnamed: Vec<LocExpr>, named: Vec<(IStr, LocExpr)>) -> Self {
		Self { unnamed, named }
	}
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Trace)]
pub enum DestructRest {
	/// ...rest
	Keep(IStr),
	/// ...
	Drop,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Trace)]
pub enum Destruct {
	Full(IStr),
	#[cfg(feature = "exp-destruct")]
	Skip,
	#[cfg(feature = "exp-destruct")]
	Array {
		start: Vec<Destruct>,
		rest: Option<DestructRest>,
		end: Vec<Destruct>,
	},
	#[cfg(feature = "exp-destruct")]
	Object {
		fields: Vec<(IStr, Option<Destruct>)>,
		rest: Option<DestructRest>,
	},
}
impl Destruct {
	/// Name of destructure, used for function parameter names
	pub fn name(&self) -> Option<IStr> {
		match self {
			Self::Full(name) => Some(name.clone()),
			#[cfg(feature = "exp-destruct")]
			_ => None,
		}
	}
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Trace)]
pub enum BindSpec {
	Field {
		into: Destruct,
		value: LocExpr,
	},
	Function {
		name: IStr,
		params: ParamsDesc,
		value: LocExpr,
	},
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Trace)]
pub struct IfSpecData(pub LocExpr);

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Trace)]
pub struct ForSpecData(pub IStr, pub LocExpr);

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Trace)]
pub enum CompSpec {
	IfSpec(IfSpecData),
	ForSpec(ForSpecData),
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Trace)]
pub struct ObjComp {
	pub pre_locals: Vec<BindSpec>,
	pub key: LocExpr,
	pub plus: bool,
	pub value: LocExpr,
	pub post_locals: Vec<BindSpec>,
	pub compspecs: Vec<CompSpec>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Trace)]
pub enum ObjBody {
	MemberList(Vec<Member>),
	ObjComp(ObjComp),
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Eq, Clone, Copy, Trace)]
pub enum LiteralType {
	This,
	Super,
	Dollar,
	Null,
	True,
	False,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Trace)]
pub struct SliceDesc {
	pub start: Option<LocExpr>,
	pub end: Option<LocExpr>,
	pub step: Option<LocExpr>,
}

/// Syntax base
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Trace)]
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
	Import(IStr),
	/// importStr "file.txt"
	ImportStr(IStr),
	/// importBin "file.txt"
	ImportBin(IStr),
	/// error "I'm broken"
	ErrorStmt(LocExpr),
	/// a(b, c)
	Apply(LocExpr, ArgsDesc, bool),
	/// a[b]
	Index(LocExpr, LocExpr),
	/// function(x) x
	Function(ParamsDesc, LocExpr),
	/// std.thisFile
	IntrinsicThisFile,
	/// std.id,
	IntrinsicId,
	/// std.primitiveEquals
	Intrinsic(IStr),
	/// if true == false then 1 else 2
	IfElse {
		cond: IfSpecData,
		cond_then: LocExpr,
		cond_else: Option<LocExpr>,
	},
	Slice(LocExpr, SliceDesc),
}

/// file, begin offset, end offset
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, PartialEq, Eq, Trace)]
#[skip_trace]
#[repr(C)]
pub struct ExprLocation(pub Source, pub u32, pub u32);
impl ExprLocation {
	pub fn belongs_to(&self, other: &ExprLocation) -> bool {
		other.0 == self.0 && other.1 <= self.1 && other.2 >= self.2
	}
}

#[cfg(target_pointer_width = "64")]
static_assertions::assert_eq_size!(ExprLocation, [u8; 16]);

impl Debug for ExprLocation {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{:?}:{:?}-{:?}", self.0, self.1, self.2)
	}
}

/// Holds AST expression and its location in source file
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, PartialEq, Trace)]
pub struct LocExpr(pub Rc<Expr>, pub ExprLocation);

#[cfg(target_pointer_width = "64")]
static_assertions::assert_eq_size!(LocExpr, [u8; 24]);

impl Debug for LocExpr {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if f.alternate() {
			write!(f, "{:#?}", self.0)?;
		} else {
			write!(f, "{:?}", self.0)?;
		}
		write!(f, " from {:?}", self.1)?;
		Ok(())
	}
}
