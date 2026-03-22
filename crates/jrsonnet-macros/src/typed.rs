use crate::names::Names;
use crate::{extract_type_from_option, kw, parse_attr, type_is_path};
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned as _;
use syn::{parenthesized, token, DeriveInput, Error, Ident, LitStr, Result, Token, Type};

#[derive(Default)]
#[allow(clippy::struct_excessive_bools)]
struct TypedAttr {
	rename: Option<String>,
	aliases: Vec<String>,
	flatten: bool,
	/// flatten(ok) strategy for flattened optionals
	/// field would be None in case of any parsing error (as in serde)
	flatten_ok: bool,
	// Should it be `field+:` instead of `field:`
	add: bool,
	// Should it be `field::` instead of `field:`
	hide: bool,
}
impl Parse for TypedAttr {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut out = Self::default();
		loop {
			let lookahead = input.lookahead1();
			if lookahead.peek(kw::rename) {
				input.parse::<kw::rename>()?;
				input.parse::<Token![=]>()?;
				let name = input.parse::<LitStr>()?;
				if out.rename.is_some() {
					return Err(Error::new(
						name.span(),
						"rename attribute may only be specified once",
					));
				}
				out.rename = Some(name.value());
			} else if lookahead.peek(kw::alias) {
				input.parse::<kw::alias>()?;
				input.parse::<Token![=]>()?;
				let alias = input.parse::<LitStr>()?;
				out.aliases.push(alias.value());
			} else if lookahead.peek(kw::flatten) {
				input.parse::<kw::flatten>()?;
				out.flatten = true;
				if input.peek(token::Paren) {
					let content;
					parenthesized!(content in input);
					let lookahead = content.lookahead1();
					if lookahead.peek(kw::ok) {
						content.parse::<kw::ok>()?;
						out.flatten_ok = true;
					} else {
						return Err(lookahead.error());
					}
				}
			} else if lookahead.peek(kw::add) {
				input.parse::<kw::add>()?;
				out.add = true;
			} else if lookahead.peek(kw::hide) {
				input.parse::<kw::hide>()?;
				out.hide = true;
			} else if input.is_empty() {
				break;
			} else {
				return Err(lookahead.error());
			}
			if input.peek(Token![,]) {
				input.parse::<Token![,]>()?;
			} else {
				break;
			}
		}
		Ok(out)
	}
}
struct TypedField {
	attr: TypedAttr,
	ident: Ident,
	ty: Type,
	is_option: bool,
	is_lazy: bool,
}
impl TypedField {
	fn parse(field: &syn::Field) -> Result<Self> {
		let attr = parse_attr::<TypedAttr, _>(&field.attrs, "typed")?.unwrap_or_default();
		let Some(ident) = field.ident.clone() else {
			return Err(Error::new(
				field.span(),
				"this field should appear in output object, but it has no visible name",
			));
		};
		let (is_option, ty) = extract_type_from_option(&field.ty)?
			.map_or_else(|| (false, field.ty.clone()), |ty| (true, ty.clone()));
		if is_option && attr.flatten {
			if !attr.flatten_ok {
				return Err(Error::new(
					field.span(),
					"strategy should be set when flattening Option",
				));
			}
		} else if attr.flatten_ok {
			return Err(Error::new(
				field.span(),
				"flatten(ok) is only useable on optional fields",
			));
		}

		let is_lazy = type_is_path(&ty, "Thunk").is_some();

		Ok(Self {
			attr,
			ident,
			ty,
			is_option,
			is_lazy,
		})
	}
	/// None if this field is flattened in jsonnet output
	fn name(&self) -> Option<String> {
		if self.attr.flatten {
			return None;
		}
		Some(
			self.attr
				.rename
				.clone()
				.unwrap_or_else(|| self.ident.to_string()),
		)
	}

	fn expand_field(&self) -> Option<TokenStream> {
		if self.is_option {
			return None;
		}
		let name = self.name()?;
		let ty = &self.ty;
		Some(quote! {
			(#name, <#ty as Typed>::TYPE)
		})
	}

	fn expand_parse(&self, names: &mut Names) -> TokenStream {
		if self.is_option {
			self.expand_parse_optional(names)
		} else {
			self.expand_parse_mandatory(names)
		}
	}

	fn expand_parse_optional(&self, names: &mut Names) -> TokenStream {
		let ident = &self.ident;
		let ty = &self.ty;

		// optional flatten is handled in same way as serde
		if self.attr.flatten {
			return quote! {
				#ident: <#ty as TypedObj>::parse(&obj).ok(),
			};
		}

		let name = names.intern(self.name().unwrap());
		let aliases = self
			.attr
			.aliases
			.iter()
			.map(|name| names.intern(name))
			.collect::<Vec<_>>();

		quote! {
			#ident: {
				let __value = if let Some(__v) = obj.get(__names[#name].clone())? {
					Some(__v)
				} #(else if let Some(__v) = obj.get(__names[#aliases].clone())? {
					Some(__v)
				})* else {
					None
				};

				__value.map(<#ty as FromUntyped>::from_untyped).transpose()?
			},
		}
	}

	fn expand_parse_mandatory(&self, names: &mut Names) -> TokenStream {
		let ident = &self.ident;
		let ty = &self.ty;

		// optional flatten is handled in same way as serde
		if self.attr.flatten {
			return quote! {
				#ident: <#ty as TypedObj>::parse(&obj)?,
			};
		}

		let name = self.name().unwrap();
		let aliases = &self.attr.aliases;

		let error_text = if aliases.is_empty() {
			// clippy does not understand name variable usage in quote! macro
			#[allow(clippy::redundant_clone)]
			name.clone()
		} else {
			format!("{name} (alias {})", aliases.join(", "))
		};

		let error_text = names.intern(error_text);
		let name = names.intern(name);
		let aliases = aliases.iter().map(|alias| names.intern(alias));

		quote! {
			#ident: {
				let __value = if let Some(__v) = obj.get(__names[#name].clone())? {
					__v
				} #(else if let Some(__v) = obj.get(__names[#aliases].clone())? {
					__v
				})* else {
					return Err(ErrorKind::NoSuchField(__names[#error_text].clone(), vec![]).into());
				};

				<#ty as FromUntyped>::from_untyped(__value)?
			},
		}
	}

	fn expand_serialize(&self, names: &mut Names) -> TokenStream {
		let ident = &self.ident;
		let ty = &self.ty;
		self.name().map_or_else(
			|| {
				if self.is_option {
					quote! {
						if let Some(value) = self.#ident {
							<#ty as TypedObj>::serialize(value, out)?;
						}
					}
				} else {
					quote! {
						<#ty as TypedObj>::serialize(self.#ident, out)?;
					}
				}
			},
			|name| {
				let name = names.intern(name);
				let hide = if self.attr.hide {
					quote! {.hide()}
				} else {
					quote! {}
				};
				let add = if self.attr.add {
					quote! {.add()}
				} else {
					quote! {}
				};
				let value = if self.is_lazy {
					quote! {
						out.field(__names[#name].clone())
							#hide
							#add
							.try_thunk(<#ty as IntoUntyped>::into_lazy_untyped(value))?;
					}
				} else {
					quote! {
						out.field(__names[#name].clone())
							#hide
							#add
							.try_value(<#ty as IntoUntyped>::into_untyped(value)?)?;
					}
				};
				if self.is_option {
					quote! {
						if let Some(value) = self.#ident {
							#value
						}
					}
				} else {
					quote! {
						{
							let value = self.#ident;
							#value
						}
					}
				}
			},
		)
	}
}

pub fn derive_typed_inner(input: DeriveInput) -> Result<TokenStream> {
	let syn::Data::Struct(data) = &input.data else {
		return Err(Error::new(input.span(), "only structs supported"));
	};

	let ident = &input.ident;
	let fields = data
		.fields
		.iter()
		.map(TypedField::parse)
		.collect::<Result<Vec<_>>>()?;

	let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

	let capacity = fields.len();

	let typed = {
		let fields = fields
			.iter()
			.filter_map(TypedField::expand_field)
			.collect::<Vec<_>>();
		quote! {
			impl #impl_generics Typed for #ident #ty_generics #where_clause {
				const TYPE: &'static ComplexValType = &ComplexValType::ObjectRef(&[
					#(#fields,)*
				]);
			}

			impl #impl_generics FromUntyped for #ident #ty_generics #where_clause {
				fn from_untyped(value: Val) -> JrResult<Self> {
					let obj = value.as_obj().expect("shape is correct");
					Self::parse(&obj)
				}
			}

			impl #impl_generics IntoUntyped for #ident #ty_generics #where_clause {
				fn into_untyped(value: Self) -> JrResult<Val> {
					let mut out = ObjValueBuilder::with_capacity(#capacity);
					value.serialize(&mut out)?;
					Ok(Val::Obj(out.build()))
				}
			}
		}
	};

	let mut names = Names::default();

	let fields_parse = fields
		.iter()
		.map(|f| f.expand_parse(&mut names))
		.collect::<Vec<_>>();
	let fields_serialize = fields
		.iter()
		.map(|f| f.expand_serialize(&mut names))
		.collect::<Vec<_>>();

	let names_expanded = names.expand();
	Ok(quote! {
		const _: () = {
			use ::jrsonnet_evaluator::{
				typed::{ComplexValType, Typed, IntoUntyped, FromUntyped, TypedObj, CheckType},
				Val, State,
				error::{ErrorKind, Result as JrResult},
				ObjValueBuilder, ObjValue, IStr,
			};

			#typed

			#names_expanded

			impl #impl_generics TypedObj for #ident #ty_generics #where_clause {
				fn serialize(self, out: &mut ObjValueBuilder) -> JrResult<()> {
					NAMES.with(|__names| {
						#(#fields_serialize)*

						Ok(())
					})
				}
				fn parse(obj: &ObjValue) -> JrResult<Self> {
					NAMES.with(|__names| Ok(Self {
						#(#fields_parse)*
					}))
				}
			}
		};
	})
}
