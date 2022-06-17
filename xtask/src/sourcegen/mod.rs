use std::{
	collections::{BTreeSet, HashSet},
	path::PathBuf,
};

use anyhow::Result;
use ast::{AstEnumSrc, AstNodeSrc, AstSrc, Cardinality, Field};
use itertools::Itertools;
use proc_macro2::{Punct, Spacing, TokenStream};
use quote::{format_ident, quote};
use ungrammar::{Grammar, Rule};
use util::{
	ensure_file_contents, pluralize, reformat, to_lower_snake_case, to_pascal_case,
	to_upper_snake_case,
};

mod ast;
mod util;

pub fn generate_ungrammar() -> Result<()> {
	let grammar: Grammar = include_str!(concat!(
		env!("CARGO_MANIFEST_DIR"),
		"/../crates/jrsonnet-rowan-parser/jsonnet.ungram"
	))
	.parse()?;

	let mut kinds: KindsSrc = KindsSrc {
		punct: puncts![
			"||" => "OR";
			"&&" => "AND";
			"|" => "BIT_OR";
			"^" => "BIT_XOR";
			"&" => "BIT_AND";
			"==" => "EQ";
			"!=" => "NE";
			"<" => "LT";
			">" => "GT";
			"<=" => "LE";
			">=" => "GE";
			"<<" => "LHS";
			">>" => "RHS";
			"+" => "PLUS";
			"-" => "MINUS";
			"*" => "MUL";
			"/" => "DIV";
			"%" => "MODULO";
			"!" => "NOT";
			"~" => "BIT_NOT";
			"[" => "L_BRACK";
			"]" => "R_BRACK";
			"(" => "L_PAREN";
			")" => "R_PAREN";
			"{" => "L_BRACE";
			"}" => "R_BRACE";
			":" => "COLON";
			"::" => "COLONCOLON";
			":::" => "COLONCOLONCOLON";
			";" => "SEMI";
			"." => "DOT";
			"..." => "DOTDOTDOT";
			"," => "COMMA";
			"$" => "DOLLAR";
			"=" => "ASSIGN";
			"?" => "QUESTION_MARK";
			"$intrinsicThisFile" => "INTRINSIC_THIS_FILE";
			"$intrinsicId" => "INTRINSIC_ID";
			"$intrinsic" => "INTRINSIC";
		],
		keywords: vec![],
		literals: literals![
			"NUMBER" => r"(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?";
			"STRING_DOUBLE" => "\"(?s:[^\"\\\\]|\\\\.)*\"";
			"STRING_SINGLE" => "'(?s:[^'\\\\]|\\\\.)*'";
			"STRING_DOUBLE_VERBATIM" => "@\"(?:[^\"]|\"\")*\"";
			"STRING_SINGLE_VERBATIM" => "@'(?:[^']|'')*'";
			"STRING_BLOCK" => r"\|\|\|";

			"IDENT" => r"[_a-zA-Z][_a-zA-Z0-9]*";
			"WHITESPACE" => r"[ \t\n\r]+";
			"SINGLE_LINE_SLASH_COMMENT" => r"//[^\r\n]*(\r\n|\n)?";
			"SINGLE_LINE_HASH_COMMENT" => r"#[^\r\n]*(\r\n|\n)?";
			"MULTI_LINE_COMMENT" => r"/\*([^*]|\*[^/])*\*/";
		],
		nodes: vec![],
	};

	let ast = lower(&kinds, &grammar);

	for node in &ast.nodes {
		let name = to_upper_snake_case(&node.name);
		if !kinds.is_literal(&name) {
			kinds.nodes.push(name);
		}
	}
	for enum_ in &ast.enums {
		let name = to_upper_snake_case(&enum_.name);
		if !kinds.is_literal(&name) {
			kinds.nodes.push(name);
		}
	}
	for token in grammar.tokens() {
		let token = &grammar[token];
		let token = &token.name.clone();
		let name = to_upper_snake_case(token);
		if !kinds.is_punct(token) && !kinds.is_literal(&name) {
			kinds.keywords.push(token.to_owned());
		}
	}

	let syntax_kinds = generate_syntax_kinds(&kinds)?;

	let tokens = generate_tokens(&ast)?;

	let nodes = generate_nodes(&kinds, &ast)?;
	ensure_file_contents(
		&PathBuf::from(concat!(
			env!("CARGO_MANIFEST_DIR"),
			"/../crates/jrsonnet-rowan-parser/src/generated/syntax_kinds.rs",
		)),
		&syntax_kinds,
	)?;
	ensure_file_contents(
		&PathBuf::from(concat!(
			env!("CARGO_MANIFEST_DIR"),
			"/../crates/jrsonnet-rowan-parser/src/generated/tokens.rs",
		)),
		&tokens,
	)?;
	ensure_file_contents(
		&PathBuf::from(concat!(
			env!("CARGO_MANIFEST_DIR"),
			"/../crates/jrsonnet-rowan-parser/src/generated/nodes.rs",
		)),
		&nodes,
	)?;
	Ok(())
}

fn generate_tokens(grammar: &AstSrc) -> Result<String> {
	let tokens = grammar.tokens.iter().map(|token| {
		let name = format_ident!("{}", token);
		let kind = format_ident!("{}", to_upper_snake_case(token));
		quote! {
			#[derive(Debug, Clone, PartialEq, Eq, Hash)]
			pub struct #name {
				pub(crate) syntax: SyntaxToken,
			}
			impl std::fmt::Display for #name {
				fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
					std::fmt::Display::fmt(&self.syntax, f)
				}
			}
			impl AstToken for #name {
				fn can_cast(kind: SyntaxKind) -> bool { kind == #kind }
				fn cast(syntax: SyntaxToken) -> Option<Self> {
					if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
				}
				fn syntax(&self) -> &SyntaxToken { &self.syntax }
			}
		}
	});

	Ok(reformat(
		&quote! {
			use crate::{SyntaxKind::{self, *}, SyntaxToken, ast::AstToken};
			#(#tokens)*
		}
		.to_string(),
	)?
	.replace("#[derive", "\n#[derive"))
}

fn generate_syntax_kinds(grammar: &KindsSrc) -> Result<String> {
	let (single_byte_tokens_values, single_byte_tokens): (Vec<_>, Vec<_>) = grammar
		.punct
		.iter()
		.filter(|(token, _name)| token.len() == 1)
		.map(|(token, name)| (token.chars().next().unwrap(), format_ident!("{}", name)))
		.unzip();

	let punctuation_values = grammar
		.punct
		.iter()
		.map(|(token, _name)| escape_token_macro(token));
	let punctuation = grammar
		.punct
		.iter()
		.map(|(_token, name)| format_ident!("{}", name))
		.collect::<Vec<_>>();
	let punctuation_enum = grammar
		.punct
		.iter()
		.map(|(token, name)| {
			let id = format_ident!("{}", name);
			quote! {
				#[token(#token)]
				#id
			}
		})
		.collect::<Vec<_>>();

	let x = |name: &str| format_ident!("{}_KW", to_upper_snake_case(name));
	let full_keywords_values = &grammar.keywords;
	let full_keywords = full_keywords_values.iter().map(|s| x(s.as_str()));

	let all_keywords_values = grammar.keywords.to_vec();
	let all_keywords_idents = all_keywords_values.iter().map(|kw| format_ident!("{}", kw));
	let all_keywords = all_keywords_values
		.iter()
		.map(|s| x(&**s))
		.collect::<Vec<_>>();
	let all_keywords_enum = all_keywords_values
		.iter()
		.map(|s| {
			let id = x(&**s);
			quote! {
				#[token(#s)]
				#id
			}
		})
		.collect::<Vec<_>>();

	let tokens_enum = grammar
		.literals
		.iter()
		.map(|l| {
			let regex = &l.regex;
			let id = format_ident!("{}", l.name);
			let lexer = l
				.lexer
				.as_ref()
				.map(|l| {
					let id: TokenStream = l.parse().expect("path");
					quote! {
						, #id
					}
				})
				.unwrap_or_else(|| quote! {});
			quote! {
				#[regex(#regex #lexer)]
				#id
			}
		})
		.collect::<Vec<_>>();

	let nodes = grammar
		.nodes
		.iter()
		.map(|name| format_ident!("{}", name))
		.collect::<Vec<_>>();

	let ast = quote! {
		#![allow(bad_style, missing_docs, unreachable_pub)]
		use logos::Logos;

		/// The kind of syntax node, e.g. `IDENT`, `USE_KW`, or `STRUCT`.
		#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Logos)]
		#[repr(u16)]
		pub enum SyntaxKind {
			#[doc(hidden)]
			TOMBSTONE,
			#[doc(hidden)]
			EOF,
			#(#punctuation_enum,)*
			#(#all_keywords_enum,)*
			#(#tokens_enum,)*
			#[error]
			ERROR,
			#(#nodes,)*
			#[doc(hidden)]
			__LAST,
		}
		use self::SyntaxKind::*;

		impl SyntaxKind {
			pub fn is_keyword(self) -> bool {
				match self {
					#(#all_keywords)|* => true,
					_ => false,
				}
			}

			pub fn is_punct(self) -> bool {
				match self {
					#(#punctuation)|* => true,
					_ => false,
				}
			}

			pub fn from_keyword(ident: &str) -> Option<SyntaxKind> {
				let kw = match ident {
					#(#full_keywords_values => #full_keywords,)*
					_ => return None,
				};
				Some(kw)
			}

			pub fn from_char(c: char) -> Option<SyntaxKind> {
				let tok = match c {
					#(#single_byte_tokens_values => #single_byte_tokens,)*
					_ => return None,
				};
				Some(tok)
			}

			pub fn from_raw(r: u16) -> Self {
				assert!(r < Self::__LAST as u16);
				unsafe { std::mem::transmute(r) }
			}
			pub fn into_raw(self) -> u16 {
				self as u16
			}
		}

		#[macro_export]
		macro_rules! T {
			#([#punctuation_values] => { $crate::SyntaxKind::#punctuation };)*
			#([#all_keywords_idents] => { $crate::SyntaxKind::#all_keywords };)*
			[lifetime_ident] => { $crate::SyntaxKind::LIFETIME_IDENT };
			[ident] => { $crate::SyntaxKind::IDENT };
			[shebang] => { $crate::SyntaxKind::SHEBANG };
		}
		pub use T;
	};

	reformat(&ast.to_string())
}

pub struct KindsSrc {
	pub punct: Vec<(String, String)>,
	pub keywords: Vec<String>,
	pub literals: Vec<LiteralKind>,
	pub nodes: Vec<String>,
}

pub struct LiteralKind {
	name: String,
	regex: String,
	lexer: Option<String>,
}

#[macro_export]
macro_rules! literals {
	($($name:expr => $regex:expr $(, $lexer:expr)?);* $(;)?) => {
		vec![
			$(LiteralKind {
				name: $name.to_owned(),
				regex: $regex.to_owned(),
				lexer: None $(.or_else(|| Some($lexer.to_string())))?,
			}),*
		]
	};
}

#[macro_export]
macro_rules! puncts {
	($($tok:expr => $name:expr);* $(;)?) => {
		vec![
			$(($tok.to_owned(), $name.to_owned())),*
		]
	};
}
use crate::{literals, puncts};

impl KindsSrc {
	pub fn is_punct(&self, tok: &str) -> bool {
		self.punct.iter().any(|(t, _)| *t == tok)
	}
	pub fn is_literal(&self, tok: &str) -> bool {
		self.literals.iter().any(|l| l.name == tok)
	}

	fn get_punct_name(&self, tok: &str) -> Option<&str> {
		self.punct
			.iter()
			.find(|(t, _)| *t == tok)
			.map(|(_, n)| n.as_str())
	}
}

fn generate_nodes(kinds: &KindsSrc, grammar: &AstSrc) -> Result<String> {
	let (node_defs, node_boilerplate_impls): (Vec<_>, Vec<_>) = grammar
		.nodes
		.iter()
		.map(|node| {
			let name = format_ident!("{}", node.name);
			let kind = format_ident!("{}", to_upper_snake_case(&node.name));
			let traits = node.traits.iter().map(|trait_name| {
				let trait_name = format_ident!("{}", trait_name);
				quote!(impl ast::#trait_name for #name {})
			});

			let methods = node.fields.iter().map(|field| {
				let method_name = field.method_name(kinds);
				let ty = field.ty();

				if field.is_many() {
					quote! {
						pub fn #method_name(&self) -> AstChildren<#ty> {
							support::children(&self.syntax)
						}
					}
				} else if let Some(token_kind) = field.token_kind() {
					quote! {
						pub fn #method_name(&self) -> Option<#ty> {
							support::token(&self.syntax, #token_kind)
						}
					}
				} else {
					quote! {
						pub fn #method_name(&self) -> Option<#ty> {
							support::child(&self.syntax)
						}
					}
				}
			});
			(
				quote! {
					#[pretty_doc_comment_placeholder_workaround]
					#[derive(Debug, Clone, PartialEq, Eq, Hash)]
					pub struct #name {
						pub(crate) syntax: SyntaxNode,
					}

					#(#traits)*

					impl #name {
						#(#methods)*
					}
				},
				quote! {
					impl AstNode for #name {
						fn can_cast(kind: SyntaxKind) -> bool {
							kind == #kind
						}
						fn cast(syntax: SyntaxNode) -> Option<Self> {
							if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
						}
						fn syntax(&self) -> &SyntaxNode { &self.syntax }
					}
				},
			)
		})
		.unzip();

	let (enum_defs, enum_boilerplate_impls): (Vec<_>, Vec<_>) = grammar
		.enums
		.iter()
		.map(|en| {
			let variants: Vec<_> = en
				.variants
				.iter()
				.map(|var| format_ident!("{}", var))
				.collect();
			let name = format_ident!("{}", en.name);
			let kinds: Vec<_> = variants
				.iter()
				.map(|name| format_ident!("{}", to_upper_snake_case(&name.to_string())))
				.collect();
			let traits = en.traits.iter().map(|trait_name| {
				let trait_name = format_ident!("{}", trait_name);
				quote!(impl ast::#trait_name for #name {})
			});

			let ast_node = quote! {
				impl AstNode for #name {
					fn can_cast(kind: SyntaxKind) -> bool {
						match kind {
							#(#kinds)|* => true,
							_ => false,
						}
					}
					fn cast(syntax: SyntaxNode) -> Option<Self> {
						let res = match syntax.kind() {
							#(
							#kinds => #name::#variants(#variants { syntax }),
							)*
							_ => return None,
						};
						Some(res)
					}
					fn syntax(&self) -> &SyntaxNode {
						match self {
							#(
							#name::#variants(it) => &it.syntax,
							)*
						}
					}
				}
			};

			(
				quote! {
					#[pretty_doc_comment_placeholder_workaround]
					#[derive(Debug, Clone, PartialEq, Eq, Hash)]
					pub enum #name {
						#(#variants(#variants),)*
					}

					#(#traits)*
				},
				quote! {
					#(
						impl From<#variants> for #name {
							fn from(node: #variants) -> #name {
								#name::#variants(node)
							}
						}
					)*
					#ast_node
				},
			)
		})
		.unzip();

	let (any_node_defs, any_node_boilerplate_impls): (Vec<_>, Vec<_>) = grammar
		.nodes
		.iter()
		.flat_map(|node| node.traits.iter().map(move |t| (t, node)))
		.into_group_map()
		.into_iter()
		.sorted_by_key(|(k, _)| *k)
		.map(|(trait_name, nodes)| {
			let name = format_ident!("Any{}", trait_name);
			let trait_name = format_ident!("{}", trait_name);
			let kinds: Vec<_> = nodes
				.iter()
				.map(|name| format_ident!("{}", to_upper_snake_case(&name.name.to_string())))
				.collect();

			(
				quote! {
					#[pretty_doc_comment_placeholder_workaround]
					#[derive(Debug, Clone, PartialEq, Eq, Hash)]
					pub struct #name {
						pub(crate) syntax: SyntaxNode,
					}
					impl ast::#trait_name for #name {}
				},
				quote! {
					impl #name {
						#[inline]
						pub fn new<T: ast::#trait_name>(node: T) -> #name {
							#name {
								syntax: node.syntax().clone()
							}
						}
					}
					impl AstNode for #name {
						fn can_cast(kind: SyntaxKind) -> bool {
							match kind {
								#(#kinds)|* => true,
								_ => false,
							}
						}
						fn cast(syntax: SyntaxNode) -> Option<Self> {
							Self::can_cast(syntax.kind()).then(|| #name { syntax })
						}
						fn syntax(&self) -> &SyntaxNode {
							&self.syntax
						}
					}
				},
			)
		})
		.unzip();

	let enum_names = grammar.enums.iter().map(|it| &it.name);
	let node_names = grammar.nodes.iter().map(|it| &it.name);

	let display_impls = enum_names
		.chain(node_names.clone())
		.map(|it| format_ident!("{}", it))
		.map(|name| {
			quote! {
				impl std::fmt::Display for #name {
					fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
						std::fmt::Display::fmt(self.syntax(), f)
					}
				}
			}
		});

	let defined_nodes: HashSet<_> = node_names.collect();

	for node in kinds
		.nodes
		.iter()
		.map(|kind| to_pascal_case(kind))
		.filter(|name| !defined_nodes.iter().any(|&it| it == name))
	{
		drop(node)
		// FIXME: restore this
		// eprintln!("Warning: node {} not defined in ast source", node);
	}

	let ast = quote! {
		#![allow(non_snake_case)]
		use crate::{
			SyntaxNode, SyntaxToken, SyntaxKind::{self, *},
			ast::{self, AstNode, AstChildren, support},
			T,
		};

		#(#node_defs)*
		#(#enum_defs)*
		#(#any_node_defs)*
		#(#node_boilerplate_impls)*
		#(#enum_boilerplate_impls)*
		#(#any_node_boilerplate_impls)*
		#(#display_impls)*
	};

	let ast = ast.to_string().replace("T ! [", "T![");

	let mut res = String::with_capacity(ast.len() * 2);

	let mut docs = grammar
		.nodes
		.iter()
		.map(|it| &it.doc)
		.chain(grammar.enums.iter().map(|it| &it.doc));

	for chunk in ast.split("# [pretty_doc_comment_placeholder_workaround] ") {
		res.push_str(chunk);
		if let Some(doc) = docs.next() {
			write_doc_comment(doc, &mut res);
		}
	}

	let res = reformat(&res)?;
	Ok(res.replace("#[derive", "\n#[derive"))
}

fn write_doc_comment(contents: &[String], dest: &mut String) {
	use std::fmt::Write;
	for line in contents {
		writeln!(dest, "///{}", line).unwrap();
	}
}

fn lower(kinds: &KindsSrc, grammar: &Grammar) -> AstSrc {
	let tokens = "Whitespace Comment String StringVerbantim StringBlock Number Ident"
		.split_ascii_whitespace()
		.map(|it| it.to_string())
		.collect::<Vec<_>>();

	let mut res = AstSrc {
		tokens,
		..Default::default()
	};

	let nodes = grammar.iter().collect::<Vec<_>>();

	for &node in &nodes {
		let name = grammar[node].name.clone();
		let rule = &grammar[node].rule;
		match lower_enum(grammar, rule) {
			Some(variants) => {
				let enum_src = AstEnumSrc {
					doc: Vec::new(),
					name,
					traits: Vec::new(),
					variants,
				};
				res.enums.push(enum_src);
			}
			None => {
				let mut fields = Vec::new();
				lower_rule(&mut fields, grammar, None, rule);
				res.nodes.push(AstNodeSrc {
					doc: Vec::new(),
					name,
					traits: Vec::new(),
					fields,
				});
			}
		}
	}

	deduplicate_fields(&mut res);
	extract_enums(&mut res);
	extract_struct_traits(kinds, &mut res);
	extract_enum_traits(&mut res);
	res
}

fn lower_enum(grammar: &Grammar, rule: &Rule) -> Option<Vec<String>> {
	let alternatives = match rule {
		Rule::Alt(it) => it,
		_ => return None,
	};
	let mut variants = Vec::new();
	for alternative in alternatives {
		match alternative {
			Rule::Node(it) => variants.push(grammar[*it].name.clone()),
			Rule::Token(it) if grammar[*it].name == ";" => (),
			_ => return None,
		}
	}
	Some(variants)
}

fn lower_rule(acc: &mut Vec<Field>, grammar: &Grammar, label: Option<&String>, rule: &Rule) {
	if lower_comma_list(acc, grammar, label, rule) {
		return;
	}

	match rule {
		Rule::Node(node) => {
			let ty = grammar[*node].name.clone();
			let name = label.cloned().unwrap_or_else(|| to_lower_snake_case(&ty));
			let field = Field::Node {
				name,
				ty,
				cardinality: Cardinality::Optional,
			};
			acc.push(field);
		}
		Rule::Token(token) => {
			assert!(label.is_none(), "uexpected label: {:?}", label);
			let name = grammar[*token].name.clone();
			let field = Field::Token(name);
			acc.push(field);
		}
		Rule::Rep(inner) => {
			if let Rule::Node(node) = &**inner {
				let ty = grammar[*node].name.clone();
				let name = label
					.cloned()
					.unwrap_or_else(|| pluralize(&to_lower_snake_case(&ty)));
				let field = Field::Node {
					name,
					ty,
					cardinality: Cardinality::Many,
				};
				acc.push(field);
				return;
			}
			todo!("unsupported repitition: {:?}", rule)
		}
		Rule::Labeled { label: l, rule } => {
			assert!(label.is_none());
			lower_rule(acc, grammar, Some(l), rule);
		}
		Rule::Seq(rules) | Rule::Alt(rules) => {
			for rule in rules {
				lower_rule(acc, grammar, label, rule)
			}
		}
		Rule::Opt(rule) => lower_rule(acc, grammar, label, rule),
	}
}

// (T (',' T)* ','?)
fn lower_comma_list(
	acc: &mut Vec<Field>,
	grammar: &Grammar,
	label: Option<&String>,
	rule: &Rule,
) -> bool {
	let rule = match rule {
		Rule::Seq(it) => it,
		_ => return false,
	};
	let (node, repeat, trailing_comma) = match rule.as_slice() {
		[Rule::Node(node), Rule::Rep(repeat), Rule::Opt(trailing_comma)] => {
			(node, repeat, trailing_comma)
		}
		_ => return false,
	};
	let repeat = match &**repeat {
		Rule::Seq(it) => it,
		_ => return false,
	};
	match repeat.as_slice() {
		[comma, Rule::Node(n)] if comma == &**trailing_comma && n == node => (),
		_ => return false,
	}
	let ty = grammar[*node].name.clone();
	let name = label
		.cloned()
		.unwrap_or_else(|| pluralize(&to_lower_snake_case(&ty)));
	let field = Field::Node {
		name,
		ty,
		cardinality: Cardinality::Many,
	};
	acc.push(field);
	true
}

fn deduplicate_fields(ast: &mut AstSrc) {
	for node in &mut ast.nodes {
		let mut i = 0;
		'outer: while i < node.fields.len() {
			for j in 0..i {
				let f1 = &node.fields[i];
				let f2 = &node.fields[j];
				if f1 == f2 {
					node.fields.remove(i);
					continue 'outer;
				}
			}
			i += 1;
		}
	}
}

fn extract_enums(ast: &mut AstSrc) {
	for node in &mut ast.nodes {
		for enm in &ast.enums {
			let mut to_remove = Vec::new();
			for (i, field) in node.fields.iter().enumerate() {
				let ty = field.ty().to_string();
				if enm.variants.iter().any(|it| it == &ty) {
					to_remove.push(i);
				}
			}
			if to_remove.len() == enm.variants.len() {
				node.remove_field(to_remove);
				let ty = enm.name.clone();
				let name = to_lower_snake_case(&ty);
				node.fields.push(Field::Node {
					name,
					ty,
					cardinality: Cardinality::Optional,
				});
			}
		}
	}
}

fn extract_struct_traits(kinds: &KindsSrc, ast: &mut AstSrc) {
	// TODO: add common accessor traits here.
	let traits: &[(&str, &[&str])] = &[];

	for node in &mut ast.nodes {
		for (name, methods) in traits {
			extract_struct_trait(kinds, node, name, methods);
		}
	}
}

fn extract_struct_trait(
	kinds: &KindsSrc,
	node: &mut AstNodeSrc,
	trait_name: &str,
	methods: &[&str],
) {
	let mut to_remove = Vec::new();
	for (i, field) in node.fields.iter().enumerate() {
		let method_name = field.method_name(kinds).to_string();
		if methods.iter().any(|&it| it == method_name) {
			to_remove.push(i);
		}
	}
	if to_remove.len() == methods.len() {
		node.traits.push(trait_name.to_string());
		node.remove_field(to_remove);
	}
}

fn extract_enum_traits(ast: &mut AstSrc) {
	let enums = ast.enums.clone();
	for enm in &mut ast.enums {
		if enm.name == "Stmt" {
			continue;
		}
		let nodes = &ast.nodes;

		let mut variant_traits = enm.variants.iter().map(|var| {
			nodes
				.iter()
				.find_map(|node| {
					if &node.name != var {
						return None;
					}
					Some(node.traits.iter().cloned().collect::<BTreeSet<_>>())
				})
				.unwrap_or_else(|| {
					enums
						.iter()
						.find_map(|node| {
							if &node.name != var {
								return None;
							}
							Some(node.traits.iter().cloned().collect::<BTreeSet<_>>())
						})
						.unwrap_or_else(|| {
							panic!("{}", {
								&format!(
									"Could not find a struct `{}` for enum `{}::{}`",
									var, enm.name, var
								)
							})
						})
				})
		});

		let mut enum_traits = match variant_traits.next() {
			Some(it) => it,
			None => continue,
		};
		for traits in variant_traits {
			enum_traits = enum_traits.intersection(&traits).cloned().collect();
		}
		enm.traits = enum_traits.into_iter().collect();
	}
}

pub fn escape_token_macro(token: &str) -> TokenStream {
	if "{}[]()$".contains(token) {
		let c = token.chars().next().unwrap();
		quote! { #c }
	} else if token.contains('$') {
		quote! { #token }
	} else {
		let cs = token.chars().map(|c| Punct::new(c, Spacing::Joint));
		quote! { #(#cs)* }
	}
}
