use std::path::PathBuf;

use dprint_core::formatting::{PrintItems, PrintOptions, Signal};
use jrsonnet_parser::{
	ArgsDesc, BinaryOpType, BindSpec, Expr, FieldName, LocExpr, Member, ObjBody, Param, ParamsDesc,
	ParserSettings, Visibility,
};

pub trait Printable {
	fn print(&self) -> PrintItems;
}

macro_rules! pi {
	(@i; $($t:tt)*) => {{
		let mut o = PrintItems::new();
		pi!(@s; o: $($t)*);
		o
	}};
	(@s; $o:ident: str($e:expr) $($t:tt)*) => {{
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

impl Printable for FieldName {
	fn print(&self) -> PrintItems {
		match self {
			FieldName::Fixed(f) => {
				p!(new: str(&f))
			}
			FieldName::Dyn(_) => todo!(),
		}
	}
}

impl Printable for Visibility {
	fn print(&self) -> PrintItems {
		match self {
			Visibility::Normal => p!(new: str(":")),
			Visibility::Hidden => p!(new: str("::")),
			Visibility::Unhide => p!(new: str(":::")),
		}
	}
}

impl Printable for BinaryOpType {
	fn print(&self) -> PrintItems {
		let o = self.to_string();
		p!(new: str(&o))
	}
}

impl<T: Printable> Printable for Option<T> {
	fn print(&self) -> PrintItems {
		if let Some(v) = self {
			v.print()
		} else {
			PrintItems::new()
		}
	}
}

impl Printable for Param {
	fn print(&self) -> PrintItems {
		p!(new:
			str(&self.0)
			if(self.1.is_some())(str(" = ") {self.1})
		)
	}
}

impl Printable for ParamsDesc {
	fn print(&self) -> PrintItems {
		let mut out = PrintItems::new();
		for (i, item) in self.0.iter().enumerate() {
			if i != 0 {
				p!(out: str(", "));
			}
			out.extend(item.print());
		}
		out
	}
}

impl Printable for ArgsDesc {
	fn print(&self) -> PrintItems {
		let mut out = PrintItems::new();
		let mut first = Some(());
		for u in self.unnamed.iter() {
			if first.take().is_none() {
				p!(out: str(", "));
			}
			p!(out: {u})
		}
		for (n, u) in self.named.iter() {
			if first.take().is_none() {
				p!(out: str(", "));
			}
			p!(out: str(&n) str(" = ") {u})
		}

		out
	}
}

impl Printable for BindSpec {
	fn print(&self) -> PrintItems {
		p!(new: str(&self.name) if(self.params.is_some())(str("(") {self.params} str(")")) str(" = ") {self.value})
	}
}

struct StrExpr<'s>(&'s str);

impl<'s> Printable for StrExpr<'s> {
	fn print(&self) -> PrintItems {
		todo!()
	}
}

impl Printable for ObjBody {
	fn print(&self) -> PrintItems {
		let mut pi = PrintItems::new();
		p!(pi: str("{"));
		match self {
			ObjBody::MemberList(m) => {
				if !m.is_empty() {
					p!(pi: nl > i);
					for m in m {
						match m {
							Member::Field(f) => {
								p!(pi:
									{f.name} {f.params}
									if(f.plus)(str("+"))
									{f.visibility} str(" ")
									{f.value}
									str(",") nl
								);
							}
							Member::BindStmt(s) => {
								p!(pi: str("local ") {s} str(",") nl)
							}
							Member::AssertStmt(a) => p!(pi: str("assert ") {a.0} if(a.1.is_some())(
								str(" : ") {a.1}
							) str(",") nl),
						}
					}
					p!(pi: <i);
				} else {
					p!(pi: str(" "))
				}
			}
			ObjBody::ObjComp(_) => todo!(),
		}
		p!(pi: str("}"));
		pi
	}
}

impl Printable for Expr {
	fn print(&self) -> PrintItems {
		let mut pi = PrintItems::new();
		match self {
			Expr::Literal(l) => match l {
				jrsonnet_parser::LiteralType::This => p!(pi: str("self")),
				jrsonnet_parser::LiteralType::Super => p!(pi: str("super")),
				jrsonnet_parser::LiteralType::Dollar => p!(pi: str("$")),
				jrsonnet_parser::LiteralType::Null => p!(pi: str("null")),
				jrsonnet_parser::LiteralType::True => p!(pi: str("true")),
				jrsonnet_parser::LiteralType::False => p!(pi: str("false")),
			},
			Expr::Str(s) => {
				p!(pi: str("\"") str(s) str("\""))
			}
			Expr::Num(n) => {
				let n = n.to_string();
				p!(pi: str(&n));
			}
			Expr::Var(v) => p!(pi: str(&v)),
			Expr::Arr(a) => {
				p!(pi: str("["));
				for (i, v) in a.iter().enumerate() {
					if i != 0 {
						p!(pi: str(", "));
					}
					p!(pi: {v})
				}
				p!(pi: str("]"));
			}
			Expr::ArrComp(_, _) => todo!(),
			Expr::Obj(o) => {
				p!(pi: {o});
			}
			Expr::ObjExtend(a, b) => p!(pi: {a} str(" ") {b}),
			Expr::Parened(v) => {
				if let Expr::Parened(_) = &v.0 as &Expr {
					p!(pi: {v})
				} else {
					p!(pi: str("(") {v} str(")"))
				}
			}
			Expr::UnaryOp(_, _) => todo!(),
			Expr::BinaryOp(a, o, b) => {
				p!(pi:
					{a} str(" ") if(!matches!(&b.0 as &Expr, Expr::Obj(_)))({o} str(" ")) {b}
				)
			}
			Expr::AssertExpr(_, _) => todo!(),
			Expr::LocalExpr(s, v) => {
				p!(pi:
					str("local") nl >i
				);
				for spec in s.iter() {
					p!(pi: {spec} str(";") nl)
				}
				p!(pi:
					<i
					{v}
				);
			}
			Expr::Import(i) => {
				let v = i.to_str().unwrap();
				p!(pi: str("import \"") str(&v) str("\""));
			}
			Expr::ImportStr(_) => todo!(),
			Expr::ErrorStmt(_) => todo!(),
			Expr::Apply(f, a, t) => p!(pi:
				{f} str("(") {a} str(")") if(*t)(str("tailstrict"))
			),
			Expr::Index(a, b) => p!(pi: {a} str("[") {b} str("]")),
			Expr::Function(_, _) => todo!(),
			Expr::Intrinsic(_) => todo!(),
			Expr::IfElse {
				cond,
				cond_then,
				cond_else,
			} => p!(pi:
				str("if ") {cond.0} str(" then") ifelse(cond_else.is_some())(
					nl >i
						{cond_then} nl
					<i str("else") nl >i
						{cond_else}
					<i
				)(str(" ") {cond_then})
			),
			Expr::Slice(v, d) => {
				p!(pi:
					{v}
					str("[") {d.start} str(":") {d.end}
					if(d.step.is_some())(
						str(":")
						{d.step}
					)
					str("]")
				)
			}
		}
		pi
	}
}

impl Printable for LocExpr {
	fn print(&self) -> PrintItems {
		self.0.print()
	}
}

fn main() {
	let parsed = jrsonnet_parser::parse(
		r#"
	
	
		# Edit me!
		local b = import "b.libsonnet";  # comment
		local a = import "a.libsonnet";
		
			 local f(x,y)=x+y;
		
		
		local Template = {z: "foo"};
		
		Template + {
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
		}
		
	
"#,
		&ParserSettings {
			file_name: PathBuf::from("example").into(),
		},
	)
	.unwrap();

	let o = dprint_core::formatting::format(
		|| {
			let print_items = parsed.print();
			print_items
		},
		PrintOptions {
			indent_width: 2,
			max_width: 100,
			use_tabs: false,
			new_line_text: "\n",
		},
	);
	println!("{}", o);
}
