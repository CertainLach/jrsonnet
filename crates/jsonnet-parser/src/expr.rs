#[derive(Debug, Clone, PartialEq)]
pub enum FieldName {
	/// {fixed: 2}
	Fixed(String),
	/// {["dyn"+"amic"]: 3}
	Dyn(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Visibility {
	/// :
	Normal,
	/// ::
	Hidden,
	/// :::
	Unhide,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssertStmt(pub Box<Expr>, pub Option<Box<Expr>>);

#[derive(Debug, Clone, PartialEq)]
pub enum FieldMember {
	Value {
		name: FieldName,
		plus: bool,
		visibility: Visibility,
		value: Expr,
	},
	Function {
		name: FieldName,
		params: Params,
		visibility: Visibility,
		value: Expr,
	},
}

#[derive(Debug, Clone, PartialEq)]
pub enum Member {
	Field(FieldMember),
	BindStmt(Bind),
	AssertStmt(AssertStmt),
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOpType {
	Plus,
	Minus,
	BitNot,
	Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOpType {
	Mul,
	Div,
	Mod,

	Add,
	Sub,

	Lhs,
	Rhs,

	Lt,
	Gt,
	Lte,
	Gte,

	In,

	Eq,
	Ne,

	BitAnd,
	BitOr,
	And,
	Or,

	BitXor,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Param {
	Positional(String),
	Named(String, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Params(pub Vec<Param>);

#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
	Positional(Box<Expr>),
	Named(String, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Args(pub Vec<Arg>);

#[derive(Debug, Clone, PartialEq)]
pub enum Bind {
	Value(String, Box<Expr>),
	Function(String, Params, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfSpec(pub Box<Expr>);
#[derive(Debug, Clone, PartialEq)]
pub struct ForSpec(pub String, pub Vec<IfSpec>);

#[derive(Debug, Clone, PartialEq)]
pub enum CompSpec {
	IfSpec(IfSpec),
	ForSpec(ForSpec),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjBody {
	MemberList(Vec<Member>),
	ObjComp {
		pre_locals: Vec<Bind>,
		key: Box<Expr>,
		value: Box<Expr>,
		post_locals: Vec<Bind>,
		first: ForSpec,
		rest: Vec<CompSpec>,
	},
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValueType {
	Null,
	True,
	False,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LiteralType {
	This,
	Super,
	Dollar,
}

/// Syntax base
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
	Value(ValueType),
	/// Plain value: null/true/false
	Literal(LiteralType),

	/// String value: "hello"
	Str(String),
	/// Number: 1, 2.0, 2e+20
	Num(f64),
	/// Variable name: test
	Var(String),

	/// Array of expressions: [1, 2, "Hello"]
	Arr(Vec<Expr>),
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
	ArrComp(Box<Expr>, Vec<ForSpec>),

	/// Object: {a: 2}
	Obj(ObjBody),
	/// Object extension: var1 {b: 2}
	ObjExtend(Box<Expr>, ObjBody),

	/// (obj)
	Parened(Box<Expr>),

	Params(Params),
	Args(Args),

	UnaryOp(UnaryOpType, Box<Expr>),
	BinaryOp(Box<Expr>, BinaryOpType, Box<Expr>),
	AssertExpr(AssertStmt, Box<Expr>),
	LocalExpr(Vec<Bind>, Box<Expr>),

	Bind(Bind),
	Import(String),
	ImportStr(String),
	Error(Box<Expr>),
	Apply(Box<Expr>, Args),
	Select(Box<Expr>, String),
	Index(Box<Expr>, Box<Expr>),
	Slice {
		value: Box<Expr>,
		start: Option<Box<Expr>>,
		end: Option<Box<Expr>>,
		step: Option<Box<Expr>>,
	},
	Function(Params, Box<Expr>),
	IfElse {
		cond: IfSpec,
		cond_then: Box<Expr>,
		cond_else: Option<Box<Expr>>,
	},
	IfSpec(IfSpec),
	ForSpec(ForSpec),
}
