use std::{
	cell::RefCell,
	fmt::{self, Debug, Display},
	ops::{Deref, RangeInclusive},
	rc::Rc,
};

use jrsonnet_gcmodule::Trace;
use jrsonnet_interner::IStr;
use jrsonnet_macros::AssociatedData;

use crate::source::Source;

#[derive(Debug, PartialEq, Trace)]
pub enum FieldName {
	/// {fixed: 2}
	Fixed(IStr),
	/// {["dyn"+"amic"]: 3}
	Dyn(LocExpr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Trace)]
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

#[derive(Clone, Debug, PartialEq, Trace)]
pub struct AssertStmt(pub LocExpr, pub Option<LocExpr>);

#[derive(Debug, PartialEq, Trace)]
pub struct FieldMember {
	pub name: FieldName,
	pub plus: bool,
	pub params: Option<ParamsDesc>,
	pub visibility: Visibility,
	pub value: LocExpr,
}

#[derive(Debug, PartialEq, Trace)]
pub enum Member {
	Field(FieldMember),
	BindStmt(BindSpec),
	AssertStmt(AssertStmt),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Trace, AssociatedData)]
#[associated(fields(pub name: &'static str, pub binding_power: ((), u8)))]
pub enum UnaryOpType {
	#[associated("+", ((), 20))]
	Plus,
	#[associated("-", ((), 20))]
	Minus,
	#[associated("~", ((), 20))]
	BitNot,
	#[associated("!", ((), 20))]
	Not,
}

impl Display for UnaryOpType {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.name(),)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Trace, AssociatedData)]
#[associated(fields(pub name: &'static str, pub binding_power: (u8, u8)))]
pub enum BinaryOpType {
	// Fake: inserted when `ident {objinside}` is detected, does not have an actual operator,
	// but works as +
	#[associated("<apply>", (20, 21))]
	ObjectApply,

	#[associated("*", (18, 19))]
	Mul,
	#[associated("/", (18, 19))]
	Div,
	#[associated("%", (18, 19))]
	Mod,

	#[associated("+", (16, 17))]
	Add,
	#[associated("-", (16, 17))]
	Sub,

	#[associated("<<", (14, 15))]
	Lhs,
	#[associated(">>", (14, 15))]
	Rhs,

	#[associated("<", (12, 13))]
	Lt,
	#[associated(">", (12, 13))]
	Gt,
	#[associated("<=", (12, 13))]
	Lte,
	#[associated(">=", (12, 13))]
	Gte,
	#[associated("in", (12, 13))]
	In,

	#[associated("==", (10, 11))]
	Eq,
	#[associated("!=", (10, 11))]
	Neq,

	#[associated("&", (8, 9))]
	BitAnd,

	#[associated("^", (6, 7))]
	BitXor,

	#[associated("|", (4, 5))]
	BitOr,

	#[associated("&&", (2, 3))]
	And,

	#[associated("||", (0, 1))]
	Or,
	#[cfg(feature = "exp-null-coaelse")]
	#[associated("??", (0, 1))]
	NullCoaelse,
}

impl Display for BinaryOpType {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.name())
	}
}

/// name, default value
#[derive(Debug, PartialEq, Trace)]
pub struct Param(pub Destruct, pub Option<LocExpr>);

/// Defined function parameters
#[derive(Debug, Clone, PartialEq, Trace)]
pub struct ParamsDesc(pub Rc<Vec<Param>>);

impl Deref for ParamsDesc {
	type Target = Vec<Param>;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

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

#[derive(Debug, Clone, PartialEq, Eq, Trace)]
pub enum DestructRest {
	/// ...rest
	Keep(IStr),
	/// ...
	Drop,
}

#[derive(Debug, Clone, PartialEq, Trace)]
pub enum Destruct {
	Full(Spanned<IStr>),
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
		fields: Vec<(IStr, Option<Destruct>, Option<LocExpr>)>,
		rest: Option<DestructRest>,
	},
}
impl Destruct {
	/// Name of destructure, used for function parameter names
	pub fn name(&self) -> Option<IStr> {
		match self {
			Self::Full(name) => Some(name.0.clone()),
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

#[derive(Debug, Clone, PartialEq, Trace)]
pub enum BindSpec {
	Field {
		into: Destruct,
		value: LocExpr,
	},
	Function {
		// Always Destruct::Full
		name: Destruct,
		params: ParamsDesc,
		value: LocExpr,
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

#[derive(Debug, PartialEq, Trace)]
pub struct IfSpecData(pub LocExpr);

#[derive(Debug, PartialEq, Trace)]
pub struct ForSpecData(pub Destruct, pub LocExpr);

#[derive(Debug, PartialEq, Trace)]
pub enum CompSpec {
	IfSpec(IfSpecData),
	ForSpec(ForSpecData),
}

#[derive(Debug, PartialEq, Trace)]
pub struct ObjComp {
	pub pre_locals: Vec<BindSpec>,
	pub field: FieldMember,
	pub post_locals: Vec<BindSpec>,
	pub compspecs: Vec<CompSpec>,
}

#[derive(Debug, PartialEq, Trace)]
pub enum ObjBody {
	MemberList(Vec<Member>),
	ObjComp(ObjComp),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Trace)]
pub enum LiteralType {
	This,
	Super,
	Dollar,
	Null,
	True,
	False,
}

#[derive(Debug, PartialEq, Trace)]
pub struct SliceDesc {
	pub start: Option<LocExpr>,
	pub end: Option<LocExpr>,
	pub step: Option<LocExpr>,
}

/// Syntax base
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

	/// -2
	UnaryOp(UnaryOpType, LocExpr),
	/// 2 - 2
	BinaryOp(LocExpr, BinaryOpType, LocExpr),
	/// assert 2 == 2 : "Math is broken"
	AssertExpr(AssertStmt, LocExpr),
	/// local a = 2; { b: a }
	LocalExpr(Vec<BindSpec>, LocExpr),

	/// import "hello"
	Import(LocExpr),
	/// importStr "file.txt"
	ImportStr(LocExpr),
	/// importBin "file.txt"
	ImportBin(LocExpr),
	/// error "I'm broken"
	ErrorStmt(LocExpr),
	/// a(b, c)
	Apply(LocExpr, ArgsDesc, bool),
	/// a[b], a.b, a?.b
	Index {
		indexable: LocExpr,
		parts: Vec<IndexPart>,
	},
	/// function(x) x
	Function(ParamsDesc, LocExpr),
	/// if true == false then 1 else 2
	IfElse {
		cond: IfSpecData,
		cond_then: LocExpr,
		cond_else: Option<LocExpr>,
	},
	Slice(LocExpr, SliceDesc),
}

#[derive(Debug, PartialEq, Trace)]
pub struct IndexPart {
	pub value: LocExpr,
	#[cfg(feature = "exp-null-coaelse")]
	pub null_coaelse: bool,
}

/// file, begin offset, end offset
#[derive(Clone, PartialEq, Eq, Trace)]
#[trace(skip)]
#[repr(C)]
pub struct Span(pub Source, pub u32, pub u32);

thread_local! {
	static CURRENT_SOURCE: RefCell<Option<Source>> = const { RefCell::new(None) };
}
// Only available during parsing
pub(crate) fn current_source() -> Source {
	CURRENT_SOURCE
		.with_borrow(|v| v.clone())
		.expect("no parsing happening right now!")
}
pub(crate) fn with_current_source<T>(current: Source, v: impl FnOnce() -> T) -> T {
	CURRENT_SOURCE.set(Some(current));
	let result = v();
	// TODO: Handle panics?
	CURRENT_SOURCE.set(None);
	result
}
impl Span {
	pub fn belongs_to(&self, other: &Span) -> bool {
		other.0 == self.0 && other.1 <= self.1 && other.2 >= self.2
	}
	pub fn range(&self) -> RangeInclusive<usize> {
		self.1 as usize..=self.2.saturating_sub(1).max(self.1) as usize
	}
	pub(crate) fn dummy() -> Self {
		Self(current_source(), 0, 0)
	}
}

static_assertions::assert_eq_size!(Span, (usize, usize));

impl Debug for Span {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{:?}:{:?}-{:?}", self.0, self.1, self.2)
	}
}

#[derive(Clone, PartialEq, Trace)]
pub struct Spanned<T: Trace>(pub T, pub Span);
impl<T: Trace> Spanned<T> {
	pub(crate) fn dummy(t: T) -> Self {
		Self(t, Span::dummy())
	}
}
impl<T: Debug + Trace> Debug for Spanned<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let expr = &self.0;
		if f.alternate() {
			write!(f, "{:#?}", expr)?;
		} else {
			write!(f, "{:?}", expr)?;
		}
		write!(f, " from {:?}", self.1)?;
		Ok(())
	}
}

/// Holds AST expression and its location in source file
#[derive(Clone, PartialEq, Trace)]
pub struct LocExpr(Rc<(Expr, Span)>);
impl LocExpr {
	pub fn new(expr: Expr, span: Span) -> Self {
		Self(Rc::new((expr, span)))
	}
	#[inline]
	pub fn span(&self) -> Span {
		self.0 .1.clone()
	}
	#[inline]
	pub fn expr(&self) -> &Expr {
		&self.0 .0
	}
}

static_assertions::assert_eq_size!(LocExpr, usize);

impl Debug for LocExpr {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let expr = self.expr();
		if f.alternate() {
			write!(f, "{:#?}", expr)?;
		} else {
			write!(f, "{:?}", expr)?;
		}
		write!(f, " from {:?}", self.span())?;
		Ok(())
	}
}
