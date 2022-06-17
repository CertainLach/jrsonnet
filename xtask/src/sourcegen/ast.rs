use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::{escape_token_macro, KindsSrc};

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
	pub tokens: Vec<String>,
	pub nodes: Vec<AstNodeSrc>,
	pub enums: Vec<AstEnumSrc>,
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
	Optional,
	Many,
}

#[derive(Debug, Clone)]
pub struct AstEnumSrc {
	pub doc: Vec<String>,
	pub name: String,
	pub traits: Vec<String>,
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
	pub fn token_kind(&self) -> Option<TokenStream> {
		match self {
			Field::Token(token) => {
				let token: TokenStream = escape_token_macro(token);
				Some(quote! { T![#token] })
			}
			_ => None,
		}
	}

	pub fn method_name(&self, kinds: &KindsSrc) -> proc_macro2::Ident {
		match self {
			Field::Token(name) => {
				if let Some(punct_name) = kinds.get_punct_name(name) {
					format_ident!("{}_token", punct_name.to_lowercase())
				} else {
					format_ident!("{}_token", name.to_lowercase())
				}
			}
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
