use std::{
	any::type_name,
	fs,
	io::{self, Write},
	path::PathBuf,
	process,
};

use children::{children_between, trivia_before};
use clap::Parser;
use dprint_core::formatting::{PrintItems, PrintOptions};
use jrsonnet_rowan_parser::{
	nodes::{
		ArgsDesc, Assertion, BinaryOperator, Bind, CompSpec, Destruct, DestructArrayPart,
		DestructRest, Expr, ExprBase, FieldName, ForSpec, IfSpec, ImportKind, Literal, Member,
		Name, Number, ObjBody, ObjLocal, ParamsDesc, SliceDesc, SourceFile, Stmt, Suffix, Text,
		UnaryOperator, Visibility,
	},
	AstNode, AstToken, SyntaxToken,
};

use crate::{
	children::trivia_after,
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
			Member::MemberFieldMethod(m) => {
				p!(new: {m.field_name()} {m.params_desc()} {m.visibility()} str(" ") {m.expr()})
			}
		}
	}
}

impl Printable for ObjBody {
	fn print(&self) -> PrintItems {
		match self {
			ObjBody::ObjBodyComp(l) => {
				let (children, mut end_comments) = children_between::<Member>(
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
					None,
				);
				let trailing_for_comp = end_comments.extract_trailing();
				let mut pi = p!(new: str("{") >i nl);
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
					Some(trailing_for_comp),
				);
				for mem in compspecs.into_iter() {
					if mem.should_start_with_newline {
						p!(pi: nl);
					}
					p!(pi: items(format_comments(&mem.before_trivia, CommentLocation::AboveItem)));
					p!(pi: {mem.value});
					p!(pi: items(format_comments(&mem.inline_trivia, CommentLocation::ItemInline)));
				}
				if end_comments.should_start_with_newline {
					p!(pi: nl);
				}
				p!(pi: items(format_comments(&end_comments.trivia, CommentLocation::EndOfItems)));

				p!(pi: nl <i str("}"));
				pi
			}
			ObjBody::ObjBodyMemberList(l) => {
				let (children, end_comments) = children_between::<Member>(
					l.syntax().clone(),
					l.l_brace_token().map(Into::into).as_ref(),
					l.r_brace_token().map(Into::into).as_ref(),
					None,
				);
				if children.is_empty() && end_comments.is_empty() {
					return p!(new: str("{ }"));
				}
				let mut pi = p!(new: str("{") >i nl);
				for (i, mem) in children.into_iter().enumerate() {
					if mem.should_start_with_newline && i != 0 {
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
				p!(new: {f.name()} {f.params()} str(" = ") {f.value()})
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
		let mut o = p!(new:);
		let (stmts, ending) = children_between::<Stmt>(
			self.syntax().clone(),
			None,
			self.expr_base()
				.as_ref()
				.map(ExprBase::syntax)
				.cloned()
				.map(Into::into)
				.as_ref(),
			None,
		);
		for stmt in stmts {
			p!(o: {stmt.value});
		}
		p!(o: {self.expr_base()});
		let (suffixes, ending) = children_between::<Suffix>(
			self.syntax().clone(),
			self.expr_base()
				.as_ref()
				.map(ExprBase::syntax)
				.cloned()
				.map(Into::into)
				.as_ref(),
			None,
			None,
		);
		for suffix in suffixes {
			p!(o: {suffix.value});
		}
		o
	}
}
impl Printable for Suffix {
	fn print(&self) -> PrintItems {
		let mut o = p!(new:);
		match self {
			Suffix::SuffixIndex(i) => {
				if i.question_mark_token().is_some() {
					p!(o: str("?"));
				}
				p!(o: str(".") {i.index()});
			}
			Suffix::SuffixIndexExpr(e) => {
				if e.question_mark_token().is_some() {
					p!(o: str(".?"));
				}
				p!(o: str("[") {e.index()} str("]"))
			}
			Suffix::SuffixSlice(d) => {
				p!(o: {d.slice_desc()})
			}
			Suffix::SuffixApply(a) => {
				p!(o: {a.args_desc()})
			}
		}
		o
	}
}
impl Printable for Stmt {
	fn print(&self) -> PrintItems {
		match self {
			Stmt::StmtLocal(l) => {
				let mut pi = p!(new:);
				let (binds, end_comments) = children_between::<Bind>(
					l.syntax().clone(),
					l.local_kw_token().map(Into::into).as_ref(),
					l.semi_token().map(Into::into).as_ref(),
					None,
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
						p!(pi: {bind.value} str(","));
						p!(pi: items(format_comments(&bind.inline_trivia, CommentLocation::ItemInline)) nl);
					}
					if end_comments.should_start_with_newline {
						p!(pi: nl)
					}
					p!(pi: items(format_comments(&end_comments.trivia, CommentLocation::EndOfItems)));
					p!(pi: <i);
				}
				p!(pi: str(";") nl);
				pi
			}
			Stmt::StmtAssert(a) => {
				p!(new: {a.assertion()} str(";") nl)
			}
		}
	}
}
impl Printable for ExprBase {
	fn print(&self) -> PrintItems {
		match self {
			Self::ExprBinary(b) => {
				p!(new: {b.lhs_work()} str(" ") {b.binary_operator()} str(" ") {b.rhs_work()})
			}
			Self::ExprUnary(u) => p!(new: {u.unary_operator()} {u.rhs()}),
			// Self::ExprSlice(s) => {
			// 	p!(new: {s.expr()} {s.slice_desc()})
			// }
			// Self::ExprIndex(i) => {
			// 	p!(new: {i.expr()} str(".") {i.index()})
			// }
			// Self::ExprIndexExpr(i) => p!(new: {i.base()} str("[") {i.index()} str("]")),
			// Self::ExprApply(a) => {
			// 	let mut pi = p!(new: {a.expr()} {a.args_desc()});
			// 	if a.tailstrict_kw_token().is_some() {
			// 		p!(pi: str(" tailstrict"));
			// 	}
			// 	pi
			// }
			Self::ExprObjExtend(ex) => {
				p!(new: {ex.lhs_work()} str(" ") {ex.rhs_work()})
			}
			Self::ExprParened(p) => {
				p!(new: str("(") {p.expr()} str(")"))
			}
			Self::ExprString(s) => p!(new: {s.text()}),
			Self::ExprNumber(n) => p!(new: {n.number()}),
			Self::ExprArray(a) => {
				let mut pi = p!(new: str("[") >i nl);
				for el in a.exprs() {
					p!(pi: {el} str(",") nl);
				}
				p!(pi: <i str("]"));
				pi
			}
			Self::ExprObject(obj) => {
				p!(new: {obj.obj_body()})
			}
			Self::ExprArrayComp(arr) => {
				let mut pi = p!(new: str("[") {arr.expr()});
				for spec in arr.comp_specs() {
					p!(pi: str(" ") {spec});
				}
				p!(pi: str("]"));
				pi
			}
			Self::ExprImport(v) => {
				p!(new: {v.import_kind()} str(" ") {v.text()})
			}
			Self::ExprVar(n) => p!(new: {n.name()}),
			// Self::ExprLocal(l) => {
			// }
			Self::ExprIfThenElse(ite) => {
				let mut pi =
					p!(new: str("if ") {ite.cond()} str(" then ") {ite.then().map(|t| t.expr())});
				if ite.else_kw_token().is_some() || ite.else_().is_some() {
					p!(pi: str(" else ") {ite.else_().map(|t| t.expr())})
				}
				pi
			}
			Self::ExprFunction(f) => p!(new: str("function") {f.params_desc()} nl {f.expr()}),
			// Self::ExprAssert(a) => p!(new: {a.assertion()} str("; ") {a.expr()}),
			Self::ExprError(e) => p!(new: str("error ") {e.expr()}),
			Self::ExprLiteral(l) => {
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

struct FormatOptions {
	// 0 for hard tabs
	indent: u8,
}
fn format(input: &str, opts: &FormatOptions) -> Option<String> {
	let (parsed, errors) = jrsonnet_rowan_parser::parse(input);
	if !errors.is_empty() {
		let mut builder = hi_doc::SnippetBuilder::new(input);
		for error in errors {
			builder
				.error(hi_doc::Text::single(
					format!("{:?}", error.error).chars(),
					Default::default(),
				))
				.range(
					error.range.start().into()
						..=(usize::from(error.range.end()) - 1).max(error.range.start().into()),
				)
				.build();
		}
		let snippet = builder.build();
		let ansi = hi_doc::source_to_ansi(&snippet);
		eprintln!("{ansi}");
		// It is possible to recover from this failure, but the output may be broken, as formatter is free to skip
		// ERROR rowan nodes.
		// Recovery needs to be enabled for LSP, though.
		//
		// TODO: Verify how formatter interacts in cases of missing positional values, i.e `if cond then /*missing Expr*/ else residual`.
		return None;
	}
	Some(dprint_core::formatting::format(
		|| parsed.print(),
		PrintOptions {
			indent_width: if opts.indent == 0 {
				// Reasonable max length for both 2 and 4 space sized tabs.
				3
			} else {
				opts.indent
			},
			max_width: 100,
			use_tabs: opts.indent == 0,
			new_line_text: "\n",
		},
	))
}

#[derive(Parser)]
struct Opts {
	/// Treat input as code, reformat it instead of reading file.
	#[clap(long, short = 'e')]
	exec: bool,
	/// Path to be reformatted if `--exec` if unset, otherwise code itself.
	input: String,
	/// Replace code with formatted in-place, instead of printing it to stdout.
	/// Only applicable if `--exec` is unset.
	#[clap(long, short = 'i')]
	in_place: bool,

	/// Exit with error if formatted does not match input
	#[arg(long)]
	test: bool,
	/// Number of spaces to indent with
	///
	/// 0 for guess from input (default), and use hard tabs if unable to guess.
	#[arg(long, default_value = "0")]
	indent: u8,
	/// Force hard tab for indentation
	#[arg(long)]
	hard_tabs: bool,

	/// Debug option: how many times to call reformatting in case of unstable dprint output resolution.
	///
	/// 0 for not retrying to reformat.
	#[arg(long, default_value = "0")]
	conv_limit: usize,
}

#[derive(thiserror::Error, Debug)]
enum Error {
	#[error("--in-place is incompatible with --exec")]
	InPlaceExec,
	#[error("io: {0}")]
	Io(#[from] io::Error),
	#[error("persist: {0}")]
	Persist(#[from] tempfile::PersistError),
	#[error("parsing failed, refusing to reformat corrupted input")]
	ParseError,
}

fn main_result() -> Result<(), Error> {
	eprintln!("jrsonnet-fmt is a prototype of a jsonnet code formatter, do not expect it to produce meaningful results right now.");
	eprintln!("It is not expected for its output to match other implementations, it will be completly separate implementation with maybe different name.");
	let mut opts = Opts::parse();
	let input = if opts.exec {
		if opts.in_place {
			return Err(Error::InPlaceExec);
		}
		opts.input.clone()
	} else {
		fs::read_to_string(&opts.input)?
	};

	if opts.indent == 0 {
		// Sane default.
		// TODO: Implement actual guessing.
		opts.hard_tabs = true;
	}

	let mut iteration = 0;
	let mut formatted = input.clone();
	let mut tmp;
	// https://github.com/dprint/dprint/pull/423
	loop {
		let Some(reformatted) = format(
			&formatted,
			&FormatOptions {
				indent: if opts.indent == 0 || opts.hard_tabs {
					0
				} else {
					opts.indent
				},
			},
		) else {
			return Err(Error::ParseError);
		};
		tmp = reformatted.trim().to_owned();
		if formatted == tmp {
			break;
		}
		formatted = tmp;
		if opts.conv_limit == 0 {
			break;
		}
		iteration += 1;
		if iteration > opts.conv_limit {
			panic!("formatting not converged");
		}
	}
	formatted.push('\n');
	if opts.test && formatted != input {
		process::exit(1);
	}
	if opts.in_place {
		let path = PathBuf::from(opts.input);
		let mut temp = tempfile::NamedTempFile::new_in(path.parent().expect(
			"not failed during read, this path is not a directory, and there is a parent",
		))?;
		temp.write_all(formatted.as_bytes())?;
		temp.flush()?;
		temp.persist(&path)?;
	} else {
		print!("{formatted}")
	}
	Ok(())
}

fn main() {
	if let Err(e) = main_result() {
		eprintln!("{e}");
		process::exit(1);
	}
}
