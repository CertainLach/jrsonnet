use std::{
	cell::RefCell,
	fmt::{self, Debug, Display},
	ops::{Deref, RangeInclusive},
	rc::Rc,
	vec::Vec,
};

use jrsonnet_gcmodule::{Trace, Tracer};
use jrsonnet_interner::IStr;

use crate::source::Source;

// Holds u128 because otherwise it fails unsafe precondition due to ZST.
thread_local! {
	static EMPTY_RC_VEC: Rc<Vec<u128>> = {
		let v = Rc::new(Vec::new());
		let v_ = v.clone();
		std::mem::forget(v_);
		// Leaks 8 bytes for ref counter
		v
	};
}
// FIXME: T should not be ZST
pub(crate) fn rc_vec<T>(v: Vec<T>) -> Rc<Vec<T>> {
	if v.is_empty() {
		// Safety:
		// The last alive item will be a thread_local value here, thus preventing destructor
		// of Vec<T> from being run. EMPTY_RC_VEC is always empty (with no save way to alter that,
		// because Rc::as_mut wants mutable reference to Rc), so no actual data is being transmutted.
		unsafe {
			std::mem::transmute::<Rc<Vec<u128>>, Rc<Vec<T>>>(EMPTY_RC_VEC.with(|v| v.clone()))
		}
	} else {
		Rc::new(v)
	}
}

#[derive(Debug, PartialEq, Trace)]
pub enum FieldName {
	/// {fixed: 2}
	Fixed(IStr),
	/// {["dyn"+"amic"]: 3}
	Dyn(SpannedExpr),
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

#[derive(Debug, PartialEq, Trace)]
pub struct AssertStmt(pub SpannedExpr, pub Option<Expr>);

#[derive(Debug, PartialEq, Trace)]
pub struct FieldMember {
	pub name: FieldName,
	pub plus: bool,
	pub params: Option<ParamsDesc>,
	pub visibility: Visibility,
	pub value: Rc<SpannedExpr>,
}

#[derive(Debug, PartialEq, Trace)]
pub(crate) enum Member {
	Field(FieldMember),
	BindStmt(BindSpec),
	AssertStmt(AssertStmt),
}

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
#[derive(Debug, PartialEq, Trace)]
pub struct Param(pub Destruct, pub Option<SpannedExpr>);

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
	pub unnamed: Rc<Vec<Spanned<Expr>>>,
	pub named: Rc<Vec<(IStr, Spanned<Expr>)>>,
}
impl ArgsDesc {
	pub fn new(unnamed: Vec<Spanned<Expr>>, named: Vec<(IStr, Spanned<Expr>)>) -> Self {
		Self {
			unnamed: rc_vec(unnamed),
			named: rc_vec(named),
		}
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
		fields: Vec<(IStr, Option<Destruct>, Option<LocExpr>)>,
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

#[derive(Debug, PartialEq, Trace)]
pub struct BindSpec {
	pub into: Destruct,
	pub value: SpannedExpr,
}
impl BindSpec {
	pub fn capacity_hint(&self) -> usize {
		self.into.capacity_hint()
	}
}

#[derive(Debug, PartialEq, Trace)]
pub struct IfSpecData(pub SpannedExpr);

#[derive(Debug, PartialEq, Trace)]
pub struct ForSpecData(pub Destruct, pub SpannedExpr);

#[derive(Debug, PartialEq, Trace)]
pub enum CompSpec {
	IfSpec(IfSpecData),
	ForSpec(ForSpecData),
}

pub(crate) struct ArrUnknown {
	pub elements: Vec<Expr>,
	pub compspec: Option<Vec<CompSpec>>,
}
impl ArrUnknown {
	pub(crate) fn new(elements: Vec<Expr>, compspec: Option<Vec<CompSpec>>) -> Self {
		Self { elements, compspec }
	}
	pub(crate) fn classify(self) -> Result<Expr, &'static str> {
		if let Some(compspec) = self.compspec {
			if self.elements.len() > 1 {
				return Err("<array comprehensions can define only one element per definition>");
			}
			let Some(field) = self.elements.into_iter().next() else {
				return Err("<array comprehensions should define one element>");
			};
			Ok(Expr::ArrComp(Rc::new(field), compspec))
		} else {
			Ok(Expr::Arr(rc_vec(self.elements)))
		}
	}
}

pub(crate) struct ObjUnknown {
	pub fields: Vec<FieldMember>,
	pub locals: Rc<Vec<BindSpec>>,
	pub asserts: Rc<Vec<AssertStmt>>,
	pub compspecs: Option<Vec<CompSpec>>,
}
impl ObjUnknown {
	pub(crate) fn new(members: Vec<Member>, compspecs: Option<Vec<CompSpec>>) -> Self {
		let mut fields = Vec::new();
		let mut binds = Vec::new();
		let mut asserts = Vec::new();
		for member in members {
			match member {
				Member::Field(field) => fields.push(field),
				Member::BindStmt(bind) => binds.push(bind),
				Member::AssertStmt(assert) => asserts.push(assert),
			}
		}
		Self {
			fields,
			locals: rc_vec(binds),
			asserts: rc_vec(asserts),
			compspecs,
		}
	}
	pub(crate) fn classify(self) -> Result<ObjInner, &'static str> {
		if let Some(compspecs) = self.compspecs {
			if !self.asserts.is_empty() {
				return Err("<object comprehensions can't have assertions>");
			}
			if self.fields.len() > 1 {
				return Err("<object comprehensions can define only one field per definition>");
			}
			let Some(field) = self.fields.into_iter().next() else {
				return Err("<object comprehensions should define one field>");
			};
			if !matches!(field.name, FieldName::Dyn(_)) {
				return Err("<object comprehension field name should be computed>");
			}
			Ok(ObjInner::Comp(ObjComp {
				locals: self.locals,
				field,
				compspecs,
			}))
		} else {
			Ok(ObjInner::Members(ObjMembers {
				fields: self.fields,
				locals: self.locals,
				asserts: self.asserts,
			}))
		}
	}
}

pub(crate) enum ObjInner {
	Members(ObjMembers),
	Comp(ObjComp),
}

#[derive(Debug, PartialEq, Trace)]
pub struct ObjComp {
	pub locals: Rc<Vec<BindSpec>>,
	pub field: FieldMember,
	pub compspecs: Vec<CompSpec>,
}

#[derive(Debug, PartialEq, Trace)]
pub struct ObjMembers {
	pub fields: Vec<FieldMember>,
	pub locals: Rc<Vec<BindSpec>>,
	pub asserts: Rc<Vec<AssertStmt>>,
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
	pub start: Option<SpannedExpr>,
	pub end: Option<SpannedExpr>,
	pub step: Option<SpannedExpr>,
}

#[derive(Debug, PartialEq, Trace)]
pub enum ImportKind {
	Normal,
	Binary,
	String,
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
	Var(Spanned<IStr>),

	/// Array of expressions: [1, 2, "Hello"]
	Arr(Rc<Vec<Expr>>),
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
	ArrComp(Rc<Expr>, Vec<CompSpec>),

	/// Object: {a: 2}
	ObjMembers(Box<ObjMembers>),
	ObjComp(Box<ObjComp>),
	/// Object extension: var1 {b: 2}
	ObjExtendMembers(Box<(Expr, ObjMembers)>),
	ObjExtendComp(Box<(Expr, ObjComp)>),

	/// -2
	UnaryOp(UnaryOpType, Box<Expr>),
	/// 2 - 2
	BinaryOp {
		op: BinaryOpType,
		ab: Box<(Expr, Expr)>,
	},
	/// assert 2 == 2 : "Math is broken"
	AssertExpr(Box<(AssertStmt, Expr)>),
	/// local a = 2; { b: a }
	LocalExpr(Rc<Vec<BindSpec>>, Box<Expr>),

	/// import{,str,bin} "hello"
	Import(Box<Spanned<(ImportKind, Expr)>>),
	/// error "I'm broken"
	ErrorStmt(Box<Spanned<Expr>>),
	/// a(b, c)
	Apply(Box<ApplyBody>),
	/// a[b], a.b, a?.b
	Index {
		indexable: Box<Expr>,
		parts: Vec<IndexPart>,
	},
	/// function(x) x
	Function(ParamsDesc, LocExpr),
	/// if true == false then 1 else 2
	IfElse(Box<IfElseBody>),
	Slice(Box<(Expr, SliceDesc)>),
}

#[derive(Debug, PartialEq, Trace)]
pub struct IfElseBody {
	pub condition: IfSpecData,
	pub then: SpannedExpr,
	pub else_: Option<SpannedExpr>,
}
#[derive(Debug, PartialEq, Trace)]
pub struct ApplyBody {
	pub lhs: Expr,
	pub args: Spanned<ArgsDesc>,
	pub tailstrict: bool,
}

#[derive(Debug, PartialEq, Trace)]
pub struct IndexPart {
	pub value: SpannedExpr,
	#[cfg(feature = "exp-null-coaelse")]
	pub null_coaelse: bool,
}

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

thread_local! {
	static DUMMY_SOURCE: Source = Source::new_virtual("<dummy (should not be used)>".into(), "".into())
}
/// file, begin offset, end offset
#[derive(Clone, PartialEq, Eq, Trace)]
#[trace(skip)]
#[repr(C)]
pub struct Span(pub Source, pub u32, pub u32);
impl Span {
	pub fn belongs_to(&self, other: &Span) -> bool {
		other.0 == self.0 && other.1 <= self.1 && other.2 >= self.2
	}
	pub fn encompassing(a: Self, b: Self) -> Self {
		assert_eq!(a.0, b.0);
		let start = a.1.min(b.1);
		let end = a.2.max(b.2);
		Span(a.0, start, end)
	}
	pub fn range(&self) -> RangeInclusive<usize> {
		self.1 as usize..=self.2.saturating_sub(1).max(self.1) as usize
	}
	pub(crate) fn dummy() -> Self {
		Self(DUMMY_SOURCE.with(|v| v.clone()), 0, 0)
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
	pub fn map<U: Trace>(self, map: impl FnOnce(T) -> U) -> Spanned<U> {
		Spanned(map(self.0), self.1)
	}
	pub(crate) fn dummy(t: T) -> Self {
		Self(t, Span::dummy())
	}
	pub fn span(&self) -> Span {
		self.1.clone()
	}
}
impl<T: Trace> Deref for Spanned<T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		&self.0
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

pub type SpannedExpr = Spanned<Expr>;

pub type LocExpr = Rc<SpannedExpr>;

static_assertions::assert_eq_size!(LocExpr, usize);

pub trait RcVecExt<T> {
	fn rc_idx(&self, idx: usize) -> RcElem<T>;
	fn rc_iter(&self) -> impl Iterator<Item = RcElem<T>>;
}
impl<T> RcVecExt<T> for Rc<Vec<T>> {
	fn rc_idx(&self, idx: usize) -> RcElem<T> {
		RcElem {
			item: &self[idx],
			vec: self.clone(),
		}
	}

	fn rc_iter(&self) -> impl Iterator<Item = RcElem<T>> {
		self.iter().map(|i| RcElem {
			item: i,
			vec: self.clone(),
		})
	}
}

pub struct RcElem<T> {
	item: *const T,
	vec: Rc<Vec<T>>,
}
impl<T> Clone for RcElem<T> {
	fn clone(&self) -> Self {
		Self {
			item: self.item,
			vec: self.vec.clone(),
		}
	}
}
impl<T> Deref for RcElem<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		// Safety: Item ptr is alive as long as vec is alive, and you can't destroy vec while there
		// are item references alive.
		unsafe { &*self.item }
	}
}
impl<T: Trace> Trace for RcElem<T> {
	fn trace(&self, tracer: &mut Tracer) {
		if T::is_type_tracked() {
			self.vec.trace(tracer)
		}
	}

	fn is_type_tracked() -> bool
	where
		Self: Sized,
	{
		T::is_type_tracked()
	}
}
