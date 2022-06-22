use std::collections::{BTreeSet, HashMap};

use proc_macro2::TokenStream;
use quote::format_ident;
use ungrammar::{Grammar, Rule};

use super::{
	util::{pluralize, to_lower_snake_case},
	KindsSrc,
};

impl AstNodeSrc {
	pub fn remove_field(&mut self, to_remove: Vec<usize>) {
		to_remove.into_iter().rev().for_each(|idx| {
			self.fields.remove(idx);
		});
	}
}

#[allow(dead_code)]
#[derive(Default, Debug)]
pub struct AstSrc {
	pub nodes: Vec<AstNodeSrc>,
	pub enums: Vec<AstEnumSrc>,
	pub token_enums: Vec<AstTokenEnumSrc>,
}
#[derive(Debug)]
pub struct AstNodeSrc {
	pub doc: Vec<String>,
	pub name: String,
	pub traits: Vec<String>,
	pub fields: Vec<Field>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Field {
	Token(String),
	Node {
		name: String,
		ty: String,
		cardinality: Cardinality,
	},
}

#[derive(Debug, Eq, PartialEq)]
pub enum Cardinality {
	/// This field may not exist in code
	Optional,
	/// This field should exist in correctly parsed code
	Required,
	/// There may be multiple field values of this kind
	Many,
}

#[derive(Debug, Clone)]
pub struct AstEnumSrc {
	pub doc: Vec<String>,
	pub name: String,
	pub traits: Vec<String>,
	pub variants: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AstTokenEnumSrc {
	pub doc: Vec<String>,
	pub name: String,
	pub variants: Vec<String>,
}

impl Field {
	pub fn is_many(&self) -> bool {
		matches!(
			self,
			Field::Node {
				cardinality: Cardinality::Many,
				..
			}
		)
	}

	pub fn token_name(&self) -> Option<String> {
		match self {
			Field::Token(token) => Some(token.clone()),
			_ => None,
		}
	}
	pub fn token_kind(&self, kinds: &KindsSrc) -> Option<TokenStream> {
		match self {
			Field::Token(token) => Some(kinds.token(token).expect("token exists").reference()),
			_ => None,
		}
	}
	pub fn is_token_enum(&self, grammar: &AstSrc) -> bool {
		match self {
			Field::Node { ty, .. } => grammar.token_enums.iter().any(|e| &e.name == ty),
			_ => false,
		}
	}

	pub fn method_name(&self, kinds: &KindsSrc) -> proc_macro2::Ident {
		match self {
			Field::Token(name) => kinds.token(name).expect("token exists").method_name(),
			Field::Node { name, .. } => {
				format_ident!("{}", name)
			}
		}
	}
	pub fn ty(&self) -> proc_macro2::Ident {
		match self {
			Field::Token(_) => format_ident!("SyntaxToken"),
			Field::Node { ty, .. } => format_ident!("{}", ty),
		}
	}
}

pub fn lower(kinds: &KindsSrc, grammar: &Grammar) -> AstSrc {
	let mut res = AstSrc {
		// tokens,
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
			None => match lower_token_enum(grammar, rule) {
				Some(variants) => {
					let tokens_enum_src = AstTokenEnumSrc {
						doc: Vec::new(),
						name,
						variants,
					};
					res.token_enums.push(tokens_enum_src);
				}
				None => {
					let mut fields = Vec::new();
					lower_rule(&mut fields, grammar, None, rule, false);
					let mut types = HashMap::new();
					for field in fields.iter().filter(|f| f.token_name().is_none()) {
						if let Some(old) = types.insert(field.ty(), field.method_name(kinds)) {
							panic!("{name}.{} has same type as {name}.{}, resolve conflict by wrapping one field: {}", old, field.method_name(kinds), field.ty());
						}
						// TODO: check for assignable field types, i.e you can have
						// ```
						// SomeEnum =
						//     SomeItem
						// |   SomeOtherItem
						// ```
						// And check above will fail to detect conflict in
						// ```
						// SomeStruct =
						//     SomeEnum
						//     SomeItem
						// ```
						// Despite generating getters, which will both return SomeEnum
					}
					res.nodes.push(AstNodeSrc {
						doc: Vec::new(),
						name,
						traits: Vec::new(),
						fields,
					});
				}
			},
		}
	}

	deduplicate_fields(&mut res);
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
fn lower_token_enum(grammar: &Grammar, rule: &Rule) -> Option<Vec<String>> {
	let alternatives = match rule {
		Rule::Alt(it) => it,
		_ => return None,
	};
	let mut variants = Vec::new();
	for alternative in alternatives {
		match alternative {
			Rule::Token(it) => variants.push(grammar[*it].name.clone()),
			_ => return None,
		}
	}
	Some(variants)
}

fn lower_rule(
	acc: &mut Vec<Field>,
	grammar: &Grammar,
	label: Option<&String>,
	rule: &Rule,
	in_optional: bool,
) {
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
				cardinality: if in_optional {
					Cardinality::Optional
				} else {
					Cardinality::Required
				},
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
			lower_rule(acc, grammar, Some(l), rule, in_optional);
		}
		Rule::Seq(rules) | Rule::Alt(rules) => {
			for rule in rules {
				lower_rule(acc, grammar, label, rule, in_optional)
			}
		}
		Rule::Opt(rule) => lower_rule(acc, grammar, label, rule, true),
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
							panic!("could not find struct {var} for enum {}::{var}", enm.name)
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
