use std::{
	fmt::{self, Debug, Display},
	ops::Deref,
	rc::Rc,
};

use jrsonnet_gcmodule::Acyclic;
use jrsonnet_interner::IStr;

use crate::source::Source;

#[derive(Debug, PartialEq, Acyclic)]
pub enum FieldName {
	/// {fixed: 2}
	Fixed(IStr),
	/// {["dyn"+"amic"]: 3}
	Dyn(Spanned<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Acyclic)]
#[repr(u8)]
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

#[derive(Debug, PartialEq, Acyclic)]
pub struct AssertStmt(pub Spanned<Expr>, pub Option<Spanned<Expr>>);

#[derive(Debug, PartialEq, Acyclic)]
pub struct FieldMember {
	pub name: FieldName,
	pub plus: bool,
	pub params: Option<ParamsDesc>,
	pub visibility: Visibility,
	pub value: Rc<Spanned<Expr>>,
}

#[derive(Debug, PartialEq, Acyclic)]
pub(crate) enum Member {
	Field(FieldMember),
	BindStmt(BindSpec),
	AssertStmt(AssertStmt),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Acyclic)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Acyclic)]
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
	#[cfg(feature = "exp-null-coaelse")]
	NullCoaelse,

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
				#[cfg(feature = "exp-null-coaelse")]
				NullCoaelse => "??",
			}
		)
	}
}

/// name, default value
#[derive(Debug, PartialEq, Acyclic)]
pub struct Param(pub Destruct, pub Option<Rc<Spanned<Expr>>>);

/// Defined function parameters
#[derive(Debug, Clone, PartialEq, Acyclic)]
pub struct ParamsDesc(pub Rc<Vec<Param>>);

impl Deref for ParamsDesc {
	type Target = Vec<Param>;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

#[derive(Debug, PartialEq, Acyclic)]
pub struct ArgsDesc {
	pub unnamed: Vec<Rc<Spanned<Expr>>>,
	pub named: Vec<(IStr, Rc<Spanned<Expr>>)>,
}
impl ArgsDesc {
	pub fn new(unnamed: Vec<Rc<Spanned<Expr>>>, named: Vec<(IStr, Rc<Spanned<Expr>>)>) -> Self {
		Self { unnamed, named }
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Acyclic)]
pub enum DestructRest {
	/// ...rest
	Keep(IStr),
	/// ...
	Drop,
}

#[derive(Debug, Clone, PartialEq, Acyclic)]
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
		fields: Vec<(IStr, Option<Destruct>, Option<Spanned<Expr>>)>,
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
	pub fn capacity_hint(&self) -> usize {
		#[cfg(feature = "exp-destruct")]
		fn cap_rest(rest: &Option<DestructRest>) -> usize {
			match rest {
				Some(DestructRest::Keep(_)) => 1,
				Some(DestructRest::Drop) => 0,
				None => 0,
			}
		}
		match self {
			Self::Full(_) => 1,
			#[cfg(feature = "exp-destruct")]
			Self::Skip => 0,
			#[cfg(feature = "exp-destruct")]
			Self::Array { start, rest, end } => {
				start.iter().map(Destruct::capacity_hint).sum::<usize>()
					+ end.iter().map(Destruct::capacity_hint).sum::<usize>()
					+ cap_rest(rest)
			}
			#[cfg(feature = "exp-destruct")]
			Self::Object { fields, rest } => {
				let mut out = 0;
				for (_, into, _) in fields {
					match into {
						Some(v) => out += v.capacity_hint(),
						// Field is destructured to default name
						None => out += 1,
					}
				}
				out + cap_rest(rest)
			}
		}
	}
}

#[derive(Debug, PartialEq, Acyclic)]
pub enum BindSpec {
	Field {
		into: Destruct,
		value: Rc<Spanned<Expr>>,
	},
	Function {
		name: IStr,
		params: ParamsDesc,
		value: Rc<Spanned<Expr>>,
	},
}
impl BindSpec {
	pub fn capacity_hint(&self) -> usize {
		match self {
			BindSpec::Field { into, .. } => into.capacity_hint(),
			BindSpec::Function { .. } => 1,
		}
	}
}

#[derive(Debug, PartialEq, Acyclic)]
pub struct IfSpecData(pub Spanned<Expr>);

#[derive(Debug, PartialEq, Acyclic)]
pub struct ForSpecData(pub Destruct, pub Spanned<Expr>);

#[derive(Debug, PartialEq, Acyclic)]
pub enum CompSpec {
	IfSpec(IfSpecData),
	ForSpec(ForSpecData),
}

#[derive(Debug, PartialEq, Acyclic)]
pub struct ObjComp {
	pub locals: Rc<Vec<BindSpec>>,
	pub field: Rc<FieldMember>,
	pub compspecs: Vec<CompSpec>,
}

#[derive(Debug, PartialEq, Acyclic)]
pub struct ObjMembers {
	pub locals: Rc<Vec<BindSpec>>,
	pub asserts: Rc<Vec<AssertStmt>>,
	pub fields: Vec<FieldMember>,
}

#[derive(Debug, PartialEq, Acyclic)]
pub enum ObjBody {
	MemberList(ObjMembers),
	ObjComp(ObjComp),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Acyclic)]
pub enum LiteralType {
	This,
	Super,
	Dollar,
	Null,
	True,
	False,
}

#[derive(Debug, PartialEq, Acyclic)]
pub struct SliceDesc {
	pub start: Option<Spanned<Expr>>,
	pub end: Option<Spanned<Expr>>,
	pub step: Option<Spanned<Expr>>,
}

#[derive(Debug, PartialEq, Acyclic)]
pub struct AssertExpr {
	pub assert: AssertStmt,
	pub rest: Spanned<Expr>,
}

#[derive(Debug, PartialEq, Acyclic)]
pub struct BinaryOp {
	pub lhs: Spanned<Expr>,
	pub op: BinaryOpType,
	pub rhs: Spanned<Expr>,
}

#[derive(Debug, PartialEq, Acyclic)]
pub enum ImportKind {
	Normal,
	Str,
	Bin,
}

#[derive(Debug, PartialEq, Acyclic)]
pub struct IfElse {
	pub cond: IfSpecData,
	pub cond_then: Spanned<Expr>,
	pub cond_else: Option<Spanned<Expr>>,
}

#[derive(Debug, PartialEq, Acyclic)]
pub struct Slice {
	pub value: Spanned<Expr>,
	pub slice: SliceDesc,
}

/// Syntax base
#[derive(Debug, PartialEq, Acyclic)]
pub enum Expr {
	Literal(LiteralType),

	/// String value: "hello"
	Str(IStr),
	/// Number: 1, 2.0, 2e+20
	Num(f64),
	/// Variable name: test
	Var(IStr),

	/// Array of expressions: [1, 2, "Hello"]
	Arr(Rc<Vec<Spanned<Expr>>>),
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
	ArrComp(Rc<Spanned<Expr>>, Vec<CompSpec>),

	/// Object: {a: 2}
	Obj(ObjBody),
	/// Object extension: var1 {b: 2}
	ObjExtend(Rc<Spanned<Expr>>, ObjBody),

	/// -2
	UnaryOp(UnaryOpType, Box<Spanned<Expr>>),
	/// 2 - 2
	BinaryOp(Box<BinaryOp>),
	/// assert 2 == 2 : "Math is broken"
	AssertExpr(Rc<AssertExpr>),
	/// local a = 2; { b: a }
	LocalExpr(Vec<BindSpec>, Box<Spanned<Expr>>),

	/// import* "hello"
	Import(ImportKind, Box<Spanned<Expr>>),
	/// error "I'm broken"
	ErrorStmt(Box<Spanned<Expr>>),
	/// a(b, c)
	Apply(Box<Spanned<Expr>>, ArgsDesc, bool),
	/// a[b], a.b, a?.b
	Index {
		indexable: Box<Spanned<Expr>>,
		parts: Vec<IndexPart>,
	},
	/// function(x) x
	Function(ParamsDesc, Rc<Spanned<Expr>>),
	/// if true == false then 1 else 2
	IfElse(Box<IfElse>),
	Slice(Box<Slice>),
}

#[derive(Debug, PartialEq, Acyclic)]
pub struct IndexPart {
	pub value: Spanned<Expr>,
	#[cfg(feature = "exp-null-coaelse")]
	pub null_coaelse: bool,
}

/// file, begin offset, end offset
#[derive(Clone, PartialEq, Eq, Acyclic)]
#[repr(C)]
pub struct Span(pub Source, pub u32, pub u32);
impl Span {
	pub fn belongs_to(&self, other: &Span) -> bool {
		other.0 == self.0 && other.1 <= self.1 && other.2 >= self.2
	}
}

static_assertions::assert_eq_size!(Span, (usize, usize));

impl Debug for Span {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{:?}:{:?}-{:?}", self.0, self.1, self.2)
	}
}

#[derive(Clone, PartialEq, Acyclic)]
pub struct Spanned<T: Acyclic>(T, Span);
impl<T: Acyclic> Deref for Spanned<T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
impl<T: Acyclic> Spanned<T> {
	#[inline]
	pub fn new(v: T, s: Span) -> Self {
		Self(v, s)
	}
	#[inline]
	pub fn span(&self) -> Span {
		self.1.clone()
	}
}

impl<T: Debug + Acyclic> Debug for Spanned<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let expr = &**self;
		if f.alternate() {
			write!(f, "{:#?}", expr)?;
		} else {
			write!(f, "{:?}", expr)?;
		}
		write!(f, " from {:?}", self.span())?;
		Ok(())
	}
}
