use std::any::type_name;

use children::{children_between, trivia_before};
use dprint_core::formatting::{PrintItems, PrintOptions};
use jrsonnet_rowan_parser::{
	nodes::{
		ArgsDesc, Assertion, BinaryOperator, Bind, CompSpec, Destruct, DestructArrayPart,
		DestructRest, Expr, FieldName, ForSpec, IfSpec, ImportKind, LhsExpr, Literal, Member, Name,
		Number, ObjBody, ObjLocal, ParamsDesc, SliceDesc, SourceFile, Text, UnaryOperator,
		Visibility, VisibilityKind,
	},
	rowan::NodeOrToken,
	AstNode, AstToken, SyntaxToken,
};

use crate::{
	children::{trivia_after, trivia_between},
	comments::{format_comments, CommentLocation},
};

mod children;
mod comments;
#[cfg(test)]
mod tests;

pub trait Printable {
	fn print(&self) -> PrintItems;
}

macro_rules! pi {
	(@i; $($t:tt)*) => {{
		#[allow(unused_mut)]
		let mut o = dprint_core::formatting::PrintItems::new();
		pi!(@s; o: $($t)*);
		o
	}};
	(@s; $o:ident: str($e:expr $(,)?) $($t:tt)*) => {{
		$o.push_str($e);
		pi!(@s; $o: $($t)*);
	}};
	(@s; $o:ident: string($e:expr $(,)?) $($t:tt)*) => {{
		$o.push_string($e);
		pi!(@s; $o: $($t)*);
	}};
	(@s; $o:ident: nl $($t:tt)*) => {{
		$o.push_signal(dprint_core::formatting::Signal::NewLine);
		pi!(@s; $o: $($t)*);
	}};
	(@s; $o:ident: tab $($t:tt)*) => {{
		$o.push_signal(dprint_core::formatting::Signal::Tab);
		pi!(@s; $o: $($t)*);
	}};
	(@s; $o:ident: >i $($t:tt)*) => {{
		$o.push_signal(dprint_core::formatting::Signal::StartIndent);
		pi!(@s; $o: $($t)*);
	}};
	(@s; $o:ident: <i $($t:tt)*) => {{
		$o.push_signal(dprint_core::formatting::Signal::FinishIndent);
		pi!(@s; $o: $($t)*);
	}};
	(@s; $o:ident: {$expr:expr} $($t:tt)*) => {{
		$o.extend($expr.print());
		pi!(@s; $o: $($t)*);
	}};
	(@s; $o:ident: items($expr:expr) $($t:tt)*) => {{
		$o.extend($expr);
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
pub(crate) use p;
pub(crate) use pi;

impl<P> Printable for Option<P>
where
	P: Printable,
{
	fn print(&self) -> PrintItems {
		if let Some(v) = self {
			v.print()
		} else {
			p!(new: string(
				format!(
					"/*missing {}*/",
					type_name::<P>().replace("jrsonnet_rowan_parser::generated::nodes::", "")
				),
			))
		}
	}
}

impl Printable for SyntaxToken {
	fn print(&self) -> PrintItems {
		p!(new: string(self.to_string()))
	}
}

impl Printable for Text {
	fn print(&self) -> PrintItems {
		p!(new: string(format!("{}", self)))
	}
}
impl Printable for Number {
	fn print(&self) -> PrintItems {
		p!(new: string(format!("{}", self)))
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
				} else if let Some(str) = f.text() {
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

impl Printable for Visibility {
	fn print(&self) -> PrintItems {
		p!(new: string(self.to_string()))
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

impl Printable for Member {
	fn print(&self) -> PrintItems {
		match self {
			Member::MemberBindStmt(b) => {
				p!(new: {b.obj_local()})
			}
			Member::MemberAssertStmt(ass) => {
				p!(new: {ass.assertion()})
			}
			Member::MemberFieldNormal(n) => {
				p!(new: {n.field_name()} if(n.plus_token().is_some())({n.plus_token()}) {n.visibility()} str(" ") {n.expr()})
			}
			Member::MemberFieldMethod(_) => todo!(),
		}
	}
}

impl Printable for ObjBody {
	fn print(&self) -> PrintItems {
		match self {
			ObjBody::ObjBodyComp(l) => {
				let mut pi = p!(new: str("{") >i nl);
				let (children, end_comments) = children_between::<Member>(
					l.syntax().clone(),
					l.l_brace_token().map(Into::into).as_ref(),
					Some(
						&(l.comp_specs()
							.next()
							.expect("at least one spec is defined")
							.syntax()
							.clone())
						.into(),
					),
				);
				for mem in children.into_iter() {
					if mem.should_start_with_newline {
						p!(pi: nl);
					}
					p!(pi: items(format_comments(&mem.before_trivia, CommentLocation::AboveItem)));
					p!(pi: {mem.value} str(","));
					p!(pi: items(format_comments(&mem.inline_trivia, CommentLocation::ItemInline)));
					p!(pi: nl)
				}

				if end_comments.should_start_with_newline {
					p!(pi: nl);
				}
				p!(pi: items(format_comments(&end_comments.trivia, CommentLocation::EndOfItems)));

				let (compspecs, end_comments) = children_between::<CompSpec>(
					l.syntax().clone(),
					l.member_comps()
						.last()
						.map(|m| m.syntax().clone())
						.map(Into::into)
						.or_else(|| l.l_brace_token().map(Into::into))
						.as_ref(),
					l.r_brace_token().map(Into::into).as_ref(),
				);
				for mem in compspecs.into_iter() {
					if mem.should_start_with_newline {
						p!(pi: nl);
					}
					p!(pi: items(format_comments(&mem.before_trivia, CommentLocation::AboveItem)));
					p!(pi: {mem.value});
					p!(pi: items(format_comments(&mem.inline_trivia, CommentLocation::ItemInline)));
					p!(pi: nl)
				}
				if end_comments.should_start_with_newline {
					p!(pi: nl);
				}
				p!(pi: items(format_comments(&end_comments.trivia, CommentLocation::EndOfItems)));

				p!(pi: <i str("}"));
				pi
			}
			ObjBody::ObjBodyMemberList(l) => {
				let mut pi = p!(new: str("{") >i nl);
				let (children, end_comments) = children_between::<Member>(
					l.syntax().clone(),
					l.l_brace_token().map(Into::into).as_ref(),
					l.r_brace_token().map(Into::into).as_ref(),
				);
				for mem in children.into_iter() {
					if mem.should_start_with_newline {
						p!(pi: nl);
					}
					p!(pi: items(format_comments(&mem.before_trivia, CommentLocation::AboveItem)));
					p!(pi: {mem.value} str(","));
					p!(pi: items(format_comments(&mem.inline_trivia, CommentLocation::ItemInline)));
					p!(pi: nl)
				}

				if end_comments.should_start_with_newline {
					p!(pi: nl);
				}
				p!(pi: items(format_comments(&end_comments.trivia, CommentLocation::EndOfItems)));
				p!(pi: <i str("}"));
				pi
			}
		}
	}
}
impl Printable for UnaryOperator {
	fn print(&self) -> PrintItems {
		p!(new: string(self.text().to_string()))
	}
}
impl Printable for BinaryOperator {
	fn print(&self) -> PrintItems {
		p!(new: string(self.text().to_string()))
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
		p!(new: string(self.syntax().to_string()))
	}
}
impl Printable for ImportKind {
	fn print(&self) -> PrintItems {
		p!(new: string(self.syntax().to_string()))
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
			Expr::ExprString(s) => p!(new: {s.text()}),
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
				p!(new: {o.obj_body()})
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
				p!(new: {v.import_kind()} str(" ") {v.text()})
			}
			Expr::ExprVar(n) => p!(new: {n.name()}),
			Expr::ExprLocal(l) => {
				let mut pi = p!(new:);
				let (binds, end_comments) = children_between::<Bind>(
					l.syntax().clone(),
					l.local_kw_token().map(Into::into).as_ref(),
					l.semi_token().map(Into::into).as_ref(),
				);
				if binds.len() == 1 {
					let bind = &binds[0];
					p!(pi: items(format_comments(&bind.before_trivia, CommentLocation::AboveItem)));
					p!(pi: str("local ") {bind.value});
				// TODO: keep end_comments, child.inline_trivia somehow, force multiple locals formatting in case of presence?
				} else {
					p!(pi: str("local") >i nl);
					for bind in binds {
						if bind.should_start_with_newline {
							p!(pi: nl);
						}
						p!(pi: items(format_comments(&bind.before_trivia, CommentLocation::AboveItem)));
						p!(pi: {bind.value} str(";"));
						p!(pi: items(format_comments(&bind.inline_trivia, CommentLocation::ItemInline)) nl);
					}
					if end_comments.should_start_with_newline {
						p!(pi: nl)
					}
					p!(pi: items(format_comments(&end_comments.trivia, CommentLocation::EndOfItems)));
					p!(pi: <i);
				}
				p!(pi: str(";") nl);

				let expr_comments = trivia_between(
					l.syntax().clone(),
					l.semi_token().map(Into::into).as_ref(),
					l.expr()
						.map(|e| e.syntax().clone())
						.map(Into::into)
						.as_ref(),
				);

				if expr_comments.should_start_with_newline {
					p!(pi: nl);
				}
				p!(pi: items(format_comments(&expr_comments.trivia, CommentLocation::AboveItem)));
				p!(pi: {l.expr()});
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
		let mut pi = p!(new:);
		let before = trivia_before(
			self.syntax().clone(),
			self.expr()
				.map(|e| e.syntax().clone())
				.map(Into::into)
				.as_ref(),
		);
		let after = trivia_after(
			self.syntax().clone(),
			self.expr()
				.map(|e| e.syntax().clone())
				.map(Into::into)
				.as_ref(),
		);
		p!(pi: items(format_comments(&before, CommentLocation::AboveItem)));
		p!(pi: {self.expr()} nl);
		p!(pi: items(format_comments(&after, CommentLocation::EndOfItems)));
		pi
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

		local ie = a[expr];

		local unary = !a;

		local
			//   I am comment
			singleLocalWithItemComment = 1,
		;

		// Comment between local and expression

		local
			a = 1, //   Inline
			// Comment above b
			b = 4,

			// c needs some space
			c = 5,

			// Comment after everything
		;


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

		  comments: {
			_: '',
			//     Plain comment
			a: '',

			#    Plain comment with empty line before
			b: '',
			/*Single-line multiline comment

			*/
			c: '',

			/**Single-line multiline doc comment

			*/
			c: '',

			/**multiline doc comment
			s
			*/
			c: '',

			/*

	Multi-line

	comment
			*/
			d: '',

			e: '', // Inline comment

			k: '',

			// Text after everything
		  },
		  comments2: {
			k: '',
			// Text after everything, but no newline above
		  },
		  k: if a         == b    then


		  2

		  else Template {},

		  compspecs: {
			obj_with_no_item: {for i in [1, 2, 3]},
			obj_with_2_items: {a:1, b:2, for i in [1,2,3]},
		  }

		} + Template


		// Comment after everything
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
