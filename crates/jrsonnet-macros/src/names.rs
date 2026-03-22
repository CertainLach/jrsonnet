use proc_macro2::TokenStream;
use quote::quote;
use std::cell::RefCell;

#[derive(Default)]
pub struct Names {
	names: Vec<String>,
}

impl Names {
	pub fn intern(&mut self, s: impl AsRef<str>) -> usize {
		let s = s.as_ref();
		if let Some(pos) = self.names.iter().position(|v| v == s) {
			return pos;
		}
		let pos = self.names.len();
		self.names.push(s.to_owned());
		pos
	}

	pub fn expand(&self) -> TokenStream {
		let len = self.names.len();
		let name = self.names.iter();
		quote! {
			thread_local! {
				static NAMES: [::jrsonnet_evaluator::IStr; #len] = [
					#(::jrsonnet_evaluator::IStr::from(#name),)*
				];
			}
		}
	}
}
