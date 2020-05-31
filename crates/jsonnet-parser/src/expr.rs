use std::fmt::Display;

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
pub struct FieldMember {
	pub name: FieldName,
	pub plus: bool,
	pub params: Option<ParamsDesc>,
	pub visibility: Visibility,
	pub value: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Member {
	Field(FieldMember),
	BindStmt(BindSpec),
	AssertStmt(AssertStmt),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOpType {
	Plus,
	Minus,
	BitNot,
	Not,
}

#[derive(Debug, Clone, Copy, PartialEq)]
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
	BitXor,

	And,
	Or,
}

/// name, default value
#[derive(Debug, Clone, PartialEq)]
pub struct Param(pub String, pub Option<Box<Expr>>);
/// Defined function parameters
#[derive(Debug, Clone, PartialEq)]
pub struct ParamsDesc(pub Vec<Param>);
impl ParamsDesc {
	pub fn with_defaults(&self) -> Vec<Param> {
		self.0.iter().filter(|e| e.1.is_some()).cloned().collect()
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct Arg(pub Option<String>, pub Box<Expr>);
#[derive(Debug, Clone, PartialEq)]
pub struct ArgsDesc(pub Vec<Arg>);

#[derive(Debug, Clone, PartialEq)]
pub struct BindSpec {
	pub name: String,
	pub params: Option<ParamsDesc>,
	pub value: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfSpecData(pub Box<Expr>);
#[derive(Debug, Clone, PartialEq)]
pub struct ForSpecData(pub String, pub Box<Expr>);

#[derive(Debug, Clone, PartialEq)]
pub enum CompSpec {
	IfSpec(IfSpecData),
	ForSpec(ForSpecData),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjBody {
	MemberList(Vec<Member>),
	ObjComp {
		pre_locals: Vec<BindSpec>,
		key: Box<Expr>,
		value: Box<Expr>,
		post_locals: Vec<BindSpec>,
		first: ForSpecData,
		rest: Vec<CompSpec>,
	},
}

#[derive(Debug, Clone, PartialEq)]
pub enum LiteralType {
	This,
	Super,
	Dollar,
	Null,
	True,
	False,
}
impl Display for LiteralType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		use LiteralType::*;
		match self {
			This => write!(f, "this"),
			Null => write!(f, "null"),
			True => write!(f, "true"),
			False => write!(f, "false"),
			_ => panic!("non printable item"),
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct SliceDesc {
	pub start: Option<Box<Expr>>,
	pub end: Option<Box<Expr>>,
	pub step: Option<Box<Expr>>,
}

/// Syntax base
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
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
	ArrComp(Box<Expr>, ForSpecData, Vec<CompSpec>),

	/// Object: {a: 2}
	Obj(ObjBody),
	/// Object extension: var1 {b: 2}
	ObjExtend(Box<Expr>, ObjBody),

	/// (obj)
	Parened(Box<Expr>),

	/// Params in function definition
	/// hello, world, test = 2
	Params(ParamsDesc),
	/// Args in function call
	/// 2 + 2, 3, named = 6
	Args(ArgsDesc),

	/// -2
	UnaryOp(UnaryOpType, Box<Expr>),
	/// 2 - 2
	BinaryOp(Box<Expr>, BinaryOpType, Box<Expr>),
	/// assert 2 == 2 : "Math is broken"
	AssertExpr(AssertStmt, Box<Expr>),
	/// local a = 2; { b: a }
	LocalExpr(Vec<BindSpec>, Box<Expr>),

	/// a = 3
	Bind(BindSpec),
	/// import "hello"
	Import(String),
	/// importStr "file.txt"
	ImportStr(String),
	/// error "I'm broken"
	Error(Box<Expr>),
	/// a(b, c)
	Apply(Box<Expr>, ArgsDesc),
	///
	Select(Box<Expr>, String),
	/// a[b]
	Index(Box<Expr>, Box<Expr>),
	/// a[1::2]
	Slice(Box<Expr>, SliceDesc),
	/// function(x) x
	Function(ParamsDesc, Box<Expr>),
	/// if true == false then 1 else 2
	IfElse {
		cond: IfSpecData,
		cond_then: Box<Expr>,
		cond_else: Option<Box<Expr>>,
	},
	/// if 2 = 3
	IfSpec(IfSpecData),
	/// for elem in array
	ForSpec(ForSpecData),
}
