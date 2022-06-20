use std::any::type_name;

use dprint_core::formatting::{PrintItems, PrintOptions, Signal};
use jrsonnet_rowan_parser::{
	nodes::{
		ArgsDesc, Assertion, BinaryOperator, Bind, CompSpec, Destruct, DestructArrayPart,
		DestructRest, Expr, Field, FieldName, ForSpec, IfSpec, ImportKind, LhsExpr, Literal,
		Member, Name, Number, ObjBody, ObjLocal, ParamsDesc, SliceDesc, SourceFile, String,
		UnaryOperator,
	},
	AstToken, SyntaxToken,
};

pub trait Printable {
	fn print(&self) -> PrintItems;
}

macro_rules! pi {
	(@i; $($t:tt)*) => {{
		#[allow(unused_mut)]
		let mut o = PrintItems::new();
		pi!(@s; o: $($t)*);
		o
	}};
	(@s; $o:ident: str($e:expr $(,)?) $($t:tt)*) => {{
		$o.push_str($e);
		pi!(@s; $o: $($t)*);
	}};
	(@s; $o:ident: nl $($t:tt)*) => {{
		$o.push_signal(Signal::NewLine);
		pi!(@s; $o: $($t)*);
	}};
	(@s; $o:ident: >i $($t:tt)*) => {{
		$o.push_signal(Signal::StartIndent);
		pi!(@s; $o: $($t)*);
	}};
	(@s; $o:ident: <i $($t:tt)*) => {{
		$o.push_signal(Signal::FinishIndent);
		pi!(@s; $o: $($t)*);
	}};
	(@s; $o:ident: {$expr:expr} $($t:tt)*) => {{
		$o.extend($expr.print());
		pi!(@s; $o: $($t)*);
	}};
	(@s; $o:ident: if ($e:expr)($($then:tt)*) $($t:tt)*) => {{
		if $e {
			pi!(@s; $o: $($then)*);
		}
		pi!(@s; $o: $($t)*);
	}};
	(@s; $o:ident: ifelse ($e:expr)($($then:tt)*)($($else:tt)*) $($t:tt)*) => {{
		if $e {
			pi!(@s; $o: $($then)*);
		} else {
			pi!(@s; $o: $($else)*);
		}
		pi!(@s; $o: $($t)*);
	}};
	(@s; $i:ident:) => {}
}
macro_rules! p {
	(new: $($t:tt)*) => {
		pi!(@i; $($t)*)
	};
	($o:ident: $($t:tt)*) => {
		pi!(@s; $o: $($t)*)
	};
}

impl<P> Printable for Option<P>
where
	P: Printable,
{
	fn print(&self) -> PrintItems {
		if let Some(v) = self {
			v.print()
		} else {
			p!(new: str(
				&format!(
					"/*missing {}*/",
					type_name::<P>().replace("jrsonnet_rowan_parser::generated::nodes::", "")
				),
			))
		}
	}
}

impl Printable for SyntaxToken {
	fn print(&self) -> PrintItems {
		p!(new: str(&self.to_string()))
	}
}

impl Printable for String {
	fn print(&self) -> PrintItems {
		p!(new: str(&format!("{}", self)))
	}
}
impl Printable for Number {
	fn print(&self) -> PrintItems {
		p!(new: str(&format!("{}", self)))
	}
}

impl Printable for Name {
	fn print(&self) -> PrintItems {
		p!(new: {self.ident_lit()})
	}
}

impl Printable for DestructRest {
	fn print(&self) -> PrintItems {
		let mut pi = p!(new: str("..."));
		if let Some(name) = self.into() {
			p!(pi: {name});
		}
		pi
	}
}

impl Printable for Destruct {
	fn print(&self) -> PrintItems {
		let mut pi = p!(new:);
		match self {
			Destruct::DestructFull(f) => {
				p!(pi: {f.name()})
			}
			Destruct::DestructSkip(_) => p!(pi: str("?")),
			Destruct::DestructArray(a) => {
				p!(pi: str("[") >i nl);
				for el in a.destruct_array_parts() {
					match el {
						DestructArrayPart::DestructArrayElement(e) => {
							p!(pi: {e.destruct()} str(",") nl)
						}
						DestructArrayPart::DestructRest(d) => {
							p!(pi: {d} str(",") nl)
						}
					}
				}
				p!(pi: <i str("]"));
			}
			Destruct::DestructObject(o) => {
				p!(pi: str("{") >i nl);
				for item in o.destruct_object_fields() {
					p!(pi: {item.field()});
					if let Some(des) = item.destruct() {
						p!(pi: str(": ") {des})
					}
					if let Some(def) = item.expr() {
						p!(pi: str(" = ") {def});
					}
					p!(pi: str(",") nl);
				}
				if let Some(rest) = o.destruct_rest() {
					p!(pi: {rest} nl)
				}
				p!(pi: <i str("}"));
			}
		}
		pi
	}
}

impl Printable for FieldName {
	fn print(&self) -> PrintItems {
		match self {
			FieldName::FieldNameFixed(f) => {
				if let Some(id) = f.id() {
					p!(new: {id})
				} else if let Some(str) = f.string() {
					p!(new: {str})
				} else {
					p!(new: str("/*missing FieldName*/"))
				}
			}
			FieldName::FieldNameDynamic(d) => {
				p!(new: str("[") {d.expr()} str("]"))
			}
		}
	}
}
impl Printable for Field {
	fn print(&self) -> PrintItems {
		let mut pi = p!(new:);
		match self {
			Field::FieldNormal(n) => {
				p!(pi: {n.field_name()});
				if n.plus_token().is_some() {
					p!(pi: str("+"));
				}
				p!(pi: str(": ") {n.expr()});
			}
			Field::FieldMethod(m) => {
				p!(pi: {m.field_name()} {m.params_desc()} str(": ") {m.expr()});
			}
		}
		pi
	}
}

impl Printable for ObjLocal {
	fn print(&self) -> PrintItems {
		p!(new: str("local ") {self.bind()})
	}
}

impl Printable for Assertion {
	fn print(&self) -> PrintItems {
		let mut pi = p!(new: str("assert ") {self.condition()});
		if self.colon_token().is_some() || self.message().is_some() {
			p!(pi: str(": ") {self.message()})
		}
		pi
	}
}

impl Printable for ParamsDesc {
	fn print(&self) -> PrintItems {
		let mut pi = p!(new: str("(") >i nl);
		for param in self.params() {
			p!(pi: {param.destruct()});
			if param.assign_token().is_some() || param.expr().is_some() {
				p!(pi: str(" = ") {param.expr()})
			}
			p!(pi: str(",") nl)
		}
		p!(pi: <i str(")"));
		pi
	}
}
impl Printable for ArgsDesc {
	fn print(&self) -> PrintItems {
		let mut pi = p!(new: str("(") >i nl);
		for arg in self.args() {
			if arg.name().is_some() || arg.assign_token().is_some() {
				p!(pi: {arg.name()} str(" = "));
			}
			p!(pi: {arg.expr()} str(",") nl)
		}
		p!(pi: <i str(")"));
		pi
	}
}
impl Printable for SliceDesc {
	fn print(&self) -> PrintItems {
		let mut pi = p!(new: str("["));
		if self.from().is_some() {
			p!(pi: {self.from()});
		}
		p!(pi: str(":"));
		if self.end().is_some() {
			p!(pi: {self.end().map(|e|e.expr())})
		}
		// Keep only one : in case if we don't need step
		if self.step().is_some() {
			p!(pi: str(":") {self.step().map(|e|e.expr())});
		}
		p!(pi: str("]"));
		pi
	}
}

impl Printable for ObjBody {
	fn print(&self) -> PrintItems {
		match self {
			ObjBody::ObjBodyComp(_) => todo!(),
			ObjBody::ObjBodyMemberList(l) => {
				let mut pi = p!(new:);
				for mem in l.members() {
					match mem {
						Member::MemberBindStmt(b) => {
							p!(pi: {b.obj_local()})
						}
						Member::MemberAssertStmt(ass) => {
							p!(pi: {ass.assertion()})
						}
						Member::MemberField(f) => {
							p!(pi: {f.field()})
						}
					}
					p!(pi: str(",") nl)
				}
				pi
			}
		}
	}
}
impl Printable for UnaryOperator {
	fn print(&self) -> PrintItems {
		p!(new: str(self.text()))
	}
}
impl Printable for BinaryOperator {
	fn print(&self) -> PrintItems {
		p!(new: str(self.text()))
	}
}
impl Printable for Bind {
	fn print(&self) -> PrintItems {
		match self {
			Bind::BindDestruct(d) => {
				p!(new: {d.into()} str(" = ") {d.value()})
			}
			Bind::BindFunction(f) => {
				p!(new: str("function") {f.params()} str(" = ") {f.value()})
			}
		}
	}
}
impl Printable for Literal {
	fn print(&self) -> PrintItems {
		p!(new: str(&self.syntax().to_string()))
	}
}
impl Printable for ImportKind {
	fn print(&self) -> PrintItems {
		p!(new: str(&self.syntax().to_string()))
	}
}
impl Printable for LhsExpr {
	fn print(&self) -> PrintItems {
		p!(new: {self.expr()})
	}
}
impl Printable for ForSpec {
	fn print(&self) -> PrintItems {
		p!(new: str("for ") {self.bind()} str(" in ") {self.expr()})
	}
}
impl Printable for IfSpec {
	fn print(&self) -> PrintItems {
		p!(new: str("if ") {self.expr()})
	}
}
impl Printable for CompSpec {
	fn print(&self) -> PrintItems {
		match self {
			CompSpec::ForSpec(f) => f.print(),
			CompSpec::IfSpec(i) => i.print(),
		}
	}
}
impl Printable for Expr {
	fn print(&self) -> PrintItems {
		match self {
			Expr::ExprBinary(b) => {
				p!(new: {b.lhs()} str(" ") {b.binary_operator()} str(" ") {b.rhs()})
			}
			Expr::ExprUnary(u) => p!(new: {u.unary_operator()} {u.rhs()}),
			Expr::ExprSlice(s) => {
				p!(new: {s.expr()} {s.slice_desc()})
			}
			Expr::ExprIndex(i) => {
				p!(new: {i.expr()} str(".") {i.index()})
			}
			Expr::ExprIndexExpr(i) => p!(new: {i.base()} str("[") {i.index()} str("]")),
			Expr::ExprApply(a) => {
				let mut pi = p!(new: {a.expr()} {a.args_desc()});
				if a.tailstrict_kw_token().is_some() {
					p!(pi: str(" tailstrict"));
				}
				pi
			}
			Expr::ExprObjExtend(ex) => {
				p!(new: {ex.lhs_expr()} str(" ") {ex.expr()})
			}
			Expr::ExprParened(p) => {
				p!(new: str("(") {p.expr()} str(")"))
			}
			Expr::ExprIntrinsicThisFile(_) => p!(new: str("$intrinsicThisFile")),
			Expr::ExprIntrinsicId(_) => p!(new: str("$intrinsicId")),
			Expr::ExprIntrinsic(i) => p!(new: str("$intrinsic(") {i.name()} str(")")),
			Expr::ExprString(s) => p!(new: {s.string()}),
			Expr::ExprNumber(n) => p!(new: {n.number()}),
			Expr::ExprArray(a) => {
				let mut pi = p!(new: str("[") >i nl);
				for el in a.exprs() {
					p!(pi: {el} str(",") nl);
				}
				p!(pi: <i str("]"));
				pi
			}
			Expr::ExprObject(o) => {
				p!(new: str("{") >i nl {o.obj_body()} <i str("}"))
			}
			Expr::ExprArrayComp(arr) => {
				let mut pi = p!(new: str("[") {arr.expr()});
				for spec in arr.comp_specs() {
					p!(pi: str(" ") {spec});
				}
				p!(pi: str("]"));
				pi
			}
			Expr::ExprImport(v) => {
				p!(new: {v.import_kind()} str(" ") {v.string()})
			}
			Expr::ExprVar(n) => p!(new: {n.name()}),
			Expr::ExprLocal(l) => {
				let mut pi = p!(new: str("local") >i nl);
				for bind in l.binds() {
					p!(pi: {bind} str(",") nl);
				}
				p!(pi: <i str(";") nl {l.expr()});
				pi
			}
			Expr::ExprIfThenElse(ite) => {
				let mut pi =
					p!(new: str("if ") {ite.cond()} str(" then ") {ite.then().map(|t| t.expr())});
				if ite.else_kw_token().is_some() || ite.else_().is_some() {
					p!(pi: str(" else ") {ite.else_().map(|t| t.expr())})
				}
				pi
			}
			Expr::ExprFunction(f) => p!(new: str("function") {f.params_desc()} str(" ") {f.expr()}),
			Expr::ExprAssert(a) => p!(new: {a.assertion()} str("; ") {a.expr()}),
			Expr::ExprError(e) => p!(new: str("error ") {e.expr()}),
			Expr::ExprLiteral(l) => {
				p!(new: {l.literal()})
			}
		}
	}
}

impl Printable for SourceFile {
	fn print(&self) -> PrintItems {
		assert!(self.expr().is_some());
		self.expr().print()
	}
}

fn main() {
	let (parsed, _errors) = jrsonnet_rowan_parser::parse(
		r#"


		# Edit me!
		local b = import "b.libsonnet";  # comment
		local a = import "a.libsonnet";

			 local f(x,y)=x+y;

		local {a: [b, ..., c], d, ...e} = null;

		local ass = assert false : false; false;

		local fn = function(a, b, c = 3) 4;

		local comp = [a for b in c if d == e];
		local ocomp = {[k]: 1 for k in v};

		local ? = skip;

		local intr = $intrinsic(test);
		local intrId = $intrinsicId;
		local intrThisFile = $intrinsicThisFile;

		local ie = a[expr];

		local unary = !a;

		local Template = {z: "foo"};

		{
						local

					h = 3,
					assert self.a == 1

					: "error",
		"f": ((((((3)))))) ,
		"g g":
		f(4,2),
		arr: [[
		  1, 2,
		  ],
		  3,
		  {
			  b: {
				  c: {
					  k: [16]
				  }
			  }
		  }
		  ],
		  m: a[1::],
		  m: b[::],
		  k: if a         == b    then


		  2

		  else Template {}
		} + Template


"#,
	);

	// dbg!(errors);
	dbg!(&parsed);

	let o = dprint_core::formatting::format(
		|| parsed.print(),
		PrintOptions {
			indent_width: 2,
			max_width: 100,
			use_tabs: false,
			new_line_text: "\n",
		},
	);
	println!("{}", o);
}
