use std::string::String;

use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::{
	parenthesized,
	parse::{Parse, ParseStream},
	parse_macro_input,
	punctuated::Punctuated,
	spanned::Spanned,
	token::{self, Comma},
	Attribute, DeriveInput, Error, Expr, ExprClosure, FnArg, GenericArgument, Ident, ItemFn,
	LitStr, Pat, Path, PathArguments, Result, ReturnType, Token, Type,
};

fn parse_attr<A: Parse, I>(attrs: &[Attribute], ident: I) -> Result<Option<A>>
where
	Ident: PartialEq<I>,
{
	let attrs = attrs
		.iter()
		.filter(|a| a.path().is_ident(&ident))
		.collect::<Vec<_>>();
	if attrs.len() > 1 {
		return Err(Error::new(
			attrs[1].span(),
			"this attribute may be specified only once",
		));
	} else if attrs.is_empty() {
		return Ok(None);
	}
	let attr = attrs[0];
	let attr = attr.parse_args::<A>()?;

	Ok(Some(attr))
}
fn remove_attr<I>(attrs: &mut Vec<Attribute>, ident: I)
where
	Ident: PartialEq<I>,
{
	attrs.retain(|a| !a.path().is_ident(&ident));
}

fn path_is(path: &Path, needed: &str) -> bool {
	path.leading_colon.is_none()
		&& !path.segments.is_empty()
		&& path.segments.iter().last().unwrap().ident == needed
}

fn type_is_path<'ty>(ty: &'ty Type, needed: &str) -> Option<&'ty PathArguments> {
	match ty {
		Type::Path(path) if path.qself.is_none() && path_is(&path.path, needed) => {
			let args = &path.path.segments.iter().last().unwrap().arguments;
			Some(args)
		}
		_ => None,
	}
}

fn extract_type_from_option(ty: &Type) -> Result<Option<&Type>> {
	let Some(args) = type_is_path(ty, "Option") else {
		return Ok(None);
	};
	// It should have only on angle-bracketed param ("<String>"):
	let PathArguments::AngleBracketed(params) = args else {
		return Err(Error::new(args.span(), "missing option generic"));
	};
	let generic_arg = params.args.iter().next().unwrap();
	// This argument must be a type:
	let GenericArgument::Type(ty) = generic_arg else {
		return Err(Error::new(
			generic_arg.span(),
			"option generic should be a type",
		));
	};
	Ok(Some(ty))
}

struct Field {
	attrs: Vec<Attribute>,
	name: Ident,
	_colon: Token![:],
	ty: Type,
}
impl Parse for Field {
	fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
		Ok(Self {
			attrs: input.call(Attribute::parse_outer)?,
			name: input.parse()?,
			_colon: input.parse()?,
			ty: input.parse()?,
		})
	}
}

mod kw {
	syn::custom_keyword!(fields);
	syn::custom_keyword!(rename);
	syn::custom_keyword!(alias);
	syn::custom_keyword!(flatten);
	syn::custom_keyword!(add);
	syn::custom_keyword!(hide);
	syn::custom_keyword!(ok);
}

struct EmptyAttr;
impl Parse for EmptyAttr {
	fn parse(_input: ParseStream<'_>) -> Result<Self> {
		Ok(Self)
	}
}

struct BuiltinAttrs {
	fields: Vec<Field>,
}
impl Parse for BuiltinAttrs {
	fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
		if input.is_empty() {
			return Ok(Self { fields: Vec::new() });
		}
		input.parse::<kw::fields>()?;
		let fields;
		parenthesized!(fields in input);
		let p = Punctuated::<Field, Comma>::parse_terminated(&fields)?;
		Ok(Self {
			fields: p.into_iter().collect(),
		})
	}
}

enum Optionality {
	Required,
	Optional,
	Default(Expr),
}

enum ArgInfo {
	Normal {
		ty: Box<Type>,
		optionality: Optionality,
		name: Option<String>,
		cfg_attrs: Vec<Attribute>,
	},
	Lazy {
		is_option: bool,
		name: Option<String>,
	},
	Context,
	Location,
	This,
}

impl ArgInfo {
	fn parse(name: &str, arg: &mut FnArg) -> Result<Self> {
		let FnArg::Typed(arg) = arg else {
			unreachable!()
		};
		let ident = match &arg.pat as &Pat {
			Pat::Ident(i) => Some(i.ident.clone()),
			_ => None,
		};
		let ty = &arg.ty;
		if type_is_path(ty, "Context").is_some() {
			return Ok(Self::Context);
		} else if type_is_path(ty, "CallLocation").is_some() {
			return Ok(Self::Location);
		} else if type_is_path(ty, "Thunk").is_some() {
			return Ok(Self::Lazy {
				is_option: false,
				name: ident.map(|v| v.to_string()),
			});
		}

		match ty as &Type {
			Type::Reference(r) if type_is_path(&r.elem, name).is_some() => return Ok(Self::This),
			_ => {}
		}

		let default_attr = parse_attr::<_, _>(&arg.attrs, "default")?;
		#[allow(if_let_rescope, reason = "false-positive, this code is already fixed")]
		let (optionality, ty) = if let Some(default) = default_attr {
			remove_attr(&mut arg.attrs, "default");
			(Optionality::Default(default), ty.clone())
		} else if let Some(ty) = extract_type_from_option(ty)? {
			if type_is_path(ty, "Thunk").is_some() {
				return Ok(Self::Lazy {
					is_option: true,
					name: ident.map(|v| v.to_string()),
				});
			}

			(Optionality::Optional, Box::new(ty.clone()))
		} else {
			(Optionality::Required, ty.clone())
		};

		let cfg_attrs = arg
			.attrs
			.iter()
			.filter(|a| a.path().is_ident("cfg"))
			.cloned()
			.collect();

		Ok(Self::Normal {
			ty,
			optionality,
			name: ident.map(|v| v.to_string()),
			cfg_attrs,
		})
	}
}

#[proc_macro_attribute]
pub fn builtin(
	attr: proc_macro::TokenStream,
	item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
	let attr = parse_macro_input!(attr as BuiltinAttrs);
	let item_fn = parse_macro_input!(item as ItemFn);

	match builtin_inner(attr, item_fn) {
		Ok(v) => v.into(),
		Err(e) => e.into_compile_error().into(),
	}
}

#[allow(clippy::too_many_lines)]
fn builtin_inner(attr: BuiltinAttrs, mut fun: ItemFn) -> syn::Result<TokenStream> {
	let ReturnType::Type(_, result) = &fun.sig.output else {
		return Err(Error::new(
			fun.sig.span(),
			"builtin should return something",
		));
	};

	let name = fun.sig.ident.to_string();
	let args = fun
		.sig
		.inputs
		.iter_mut()
		.map(|arg| ArgInfo::parse(&name, arg))
		.collect::<Result<Vec<_>>>()?;

	let params_desc = args.iter().filter_map(|a| match a {
		ArgInfo::Normal {
			optionality,
			name,
			cfg_attrs,
			..
		} => {
			let name = name
				.as_ref()
				.map_or_else(|| quote! {None}, |n| quote! {ParamName::new_static(#n)});
			let default = match optionality {
				Optionality::Required => quote!(ParamDefault::None),
				Optionality::Optional => quote!(ParamDefault::Exists),
				Optionality::Default(e) => quote!(ParamDefault::Literal(stringify!(#e))),
			};
			Some(quote! {
				#(#cfg_attrs)*
				Param::new(#name, #default),
			})
		}
		ArgInfo::Lazy { is_option, name } => {
			let name = name
				.as_ref()
				.map_or_else(|| quote! {None}, |n| quote! {ParamName::new_static(#n)});
			Some(quote! {
				Param::new(#name, ParamDefault::exists(#is_option)),
			})
		}
		ArgInfo::Context | ArgInfo::Location | ArgInfo::This => None,
	});

	let mut id = 0usize;
	let pass = args
		.iter()
		.map(|a| match a {
			ArgInfo::Normal { .. } | ArgInfo::Lazy { .. } => {
				let cid = id;
				id += 1;
				(quote! {#cid}, a)
			}
			ArgInfo::Context | ArgInfo::Location | ArgInfo::This => {
				(quote! {compile_error!("should not use id")}, a)
			}
		})
		.map(|(id, a)| match a {
			ArgInfo::Normal {
				ty,
				optionality,
				name,
				cfg_attrs,
			} => {
				let name = name.as_ref().map_or("<unnamed>", String::as_str);
				let eval = quote! {jrsonnet_evaluator::in_description_frame(
					|| format!("argument <{}> evaluation", #name),
					|| <#ty as FromUntyped>::from_untyped(value.evaluate()?),
				)?};
				let value = match optionality {
					Optionality::Required => quote! {{
						let value = parsed[#id].as_ref().expect("args shape is checked");
						#eval
					},},
					Optionality::Optional => quote! {if let Some(value) = &parsed[#id] {
						Some(#eval)
					} else {
						None
					},},
					Optionality::Default(expr) => quote! {if let Some(value) = &parsed[#id] {
						#eval
					} else {
						let v: #ty = #expr;
						v
					},},
				};
				quote! {
					#(#cfg_attrs)*
					#value
				}
			}
			ArgInfo::Lazy { is_option, .. } => {
				if *is_option {
					quote! {if let Some(value) = &parsed[#id] {
						Some(value.as_thunk())
					} else {
						None
					},}
				} else {
					quote! {
						parsed[#id].as_ref().expect("args shape is correct").as_thunk(),
					}
				}
			}
			ArgInfo::Context => quote! {ctx.clone(),},
			ArgInfo::Location => quote! {location,},
			ArgInfo::This => quote! {self,},
		});

	let fields = attr.fields.iter().map(|field| {
		let attrs = &field.attrs;
		let name = &field.name;
		let ty = &field.ty;
		quote! {
			#(#attrs)*
			pub #name: #ty,
		}
	});

	let name = &fun.sig.ident;
	let vis = &fun.vis;
	let static_ext = if attr.fields.is_empty() {
		quote! {
			impl #name {
				pub const INST: &'static dyn StaticBuiltin = &#name {};
			}
			impl StaticBuiltin for #name {}
		}
	} else {
		quote! {}
	};
	let static_derive_copy = if attr.fields.is_empty() {
		quote! {, Copy}
	} else {
		quote! {}
	};

	Ok(quote! {
		#fun

		#[doc(hidden)]
		#[allow(non_camel_case_types)]
		#[derive(Clone, jrsonnet_gcmodule::Trace #static_derive_copy)]
		#vis struct #name {
			#(#fields)*
		}
		const _: () = {
			use ::jrsonnet_evaluator::{
				State, Val,
				function::{Param, Builtin, StaticBuiltin, ParamName, ParamDefault, CallLocation, macro_internal::{ArgsLike, parse_builtin_call}},
				Result, Context, typed::Typed,
				parser::Span,
			};
			const PARAMS: &'static [Param] = &[
				#(#params_desc)*
			];

			#static_ext
			impl Builtin for #name
			where
				Self: 'static
			{
				fn name(&self) -> &str {
					stringify!(#name)
				}
				fn params(&self) -> &[Param] {
					PARAMS
				}
				#[allow(unused_variables)]
				fn call(&self, ctx: &Context, location: CallLocation, args: &dyn ArgsLike) -> Result<Val> {
					let parsed = parse_builtin_call(ctx, &PARAMS, args, false)?;

					let result: #result = #name(#(#pass)*);
					<_ as IntoUntyped>::into_result(result)
				}
			}
		};
	})
}

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
	fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
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

	fn expand_parse(&self) -> TokenStream {
		if self.is_option {
			self.expand_parse_optional()
		} else {
			self.expand_parse_mandatory()
		}
	}

	fn expand_parse_optional(&self) -> TokenStream {
		let ident = &self.ident;
		let ty = &self.ty;

		// optional flatten is handled in same way as serde
		if self.attr.flatten {
			return quote! {
				#ident: <#ty as FromUntypedObj>::parse(&obj).ok(),
			};
		}

		let name = self.name().unwrap();
		let aliases = &self.attr.aliases;

		quote! {
			#ident: {
				let __value = if let Some(__v) = obj.get(#name.into())? {
					Some(__v)
				} #(else if let Some(__v) = obj.get(#aliases.into())? {
					Some(__v)
				})* else {
					None
				};

				__value.map(<#ty as FromUntyped>::from_untyped).transpose()?
			},
		}
	}

	fn expand_parse_mandatory(&self) -> TokenStream {
		let ident = &self.ident;
		let ty = &self.ty;

		// optional flatten is handled in same way as serde
		if self.attr.flatten {
			return quote! {
				#ident: <#ty as FromUntypedObj>::parse(&obj)?,
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

		quote! {
			#ident: {
				let __value = if let Some(__v) = obj.get(#name.into())? {
					__v
				} #(else if let Some(__v) = obj.get(#aliases.into())? {
					__v
				})* else {
					return Err(ErrorKind::NoSuchField(#error_text.into(), vec![]).into());
				};

				<#ty as FromUntyped>::from_untyped(__value)?
			},
		}
	}

	fn expand_serialize(&self) -> TokenStream {
		let ident = &self.ident;
		let ty = &self.ty;
		self.name().map_or_else(
			|| {
				if self.is_option {
					quote! {
						if let Some(value) = self.#ident {
							<#ty as IntoUntypedObj>::serialize(value, out)?;
						}
					}
				} else {
					quote! {
						<#ty as IntoUntypedObj>::serialize(self.#ident, out)?;
					}
				}
			},
			|name| {
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
						out.field(#name)
							#hide
							#add
							.try_thunk(<#ty as IntoUntyped>::into_lazy_untyped(value))?;
					}
				} else {
					quote! {
						out.field(#name)
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

#[proc_macro_derive(Typed, attributes(typed))]
pub fn derive_typed(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let input = parse_macro_input!(item as DeriveInput);

	match derive_typed_inner(input) {
		Ok(v) => v.into(),
		Err(e) => e.to_compile_error().into(),
	}
}
#[proc_macro_derive(IntoUntyped, attributes(typed))]
pub fn derive_into_untyped(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let input = parse_macro_input!(item as DeriveInput);

	match derive_into_untyped_inner(input) {
		Ok(v) => v.into(),
		Err(e) => e.to_compile_error().into(),
	}
}
#[proc_macro_derive(FromUntyped, attributes(typed))]
pub fn derive_from_untyped(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let input = parse_macro_input!(item as DeriveInput);

	match derive_from_untyped_inner(input) {
		Ok(v) => v.into(),
		Err(e) => e.to_compile_error().into(),
	}
}
#[allow(clippy::too_many_lines)]
fn derive_typed_inner(input: DeriveInput) -> Result<TokenStream> {
	let syn::Data::Struct(data) = &input.data else {
		return Err(Error::new(input.span(), "only structs supported"));
	};
	let ident = &input.ident;
	let fields = data
		.fields
		.iter()
		.map(TypedField::parse)
		.collect::<Result<Vec<_>>>()?;

	let fields = fields
		.iter()
		.filter_map(TypedField::expand_field)
		.collect::<Vec<_>>();

	let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
	Ok(quote! {
		const _: () = {
			use ::jrsonnet_evaluator::typed_macro_prelude::*;
			impl #impl_generics Typed for #ident #ty_generics #where_clause {
				const TYPE: &'static ComplexValType = &ComplexValType::ObjectRef(&[
					#(#fields,)*
				]);
			}
		};
	})
}

#[allow(clippy::too_many_lines)]
fn derive_from_untyped_inner(input: DeriveInput) -> Result<TokenStream> {
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

	let fields_parse = fields.iter().map(TypedField::expand_parse);

	let field_strings = fields.iter().filter_map(|f| {
		let ident = format_ident!("s_{}", f.ident);
		let name = f.name()?;
		Some(quote!(#ident: #name))
	});

	Ok(quote! {
		const _: () = {
			use ::jrsonnet_evaluator::typed_macro_prelude::*;

			strings! {
				#(#field_strings),*
			}

			impl #impl_generics FromUntyped for #ident #ty_generics #where_clause {
				fn from_untyped(value: Val) -> JrResult<Self> {
					Self::TYPE.check(&value)?;
					let obj = value.as_obj().expect("shape is correct");
					Self::parse(&obj)
				}
			}

			impl #impl_generics FromUntypedObj for #ident #ty_generics #where_clause {
				fn parse(obj: &ObjValue) -> JrResult<Self> {
					Ok(Self {
						#(#fields_parse)*
					})
				}
			}
		};
	})
}
#[allow(clippy::too_many_lines)]
fn derive_into_untyped_inner(input: DeriveInput) -> Result<TokenStream> {
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

	let typed = {
		quote! {
			impl #impl_generics IntoUntyped for #ident #ty_generics #where_clause {
				fn into_untyped(value: Self) -> JrResult<Val> {
					let mut out = ObjValueBuilder::new();
					value.serialize(&mut out)?;
					Ok(Val::Obj(out.build()))
				}
			}
		}
	};

	let fields_serialize = fields
		.iter()
		.map(TypedField::expand_serialize)
		.collect::<Vec<_>>();

	let field_strings = fields.iter().filter_map(|f| {
		let ident = format_ident!("s_{}", f.ident);
		let name = f.name()?;
		Some(quote!(#ident: #name))
	});

	Ok(quote! {
		const _: () = {
			use ::jrsonnet_evaluator::typed_macro_prelude::*;

			#typed

			strings! {
				#(#field_strings),*
			}

			impl #impl_generics IntoUntypedObj for #ident #ty_generics #where_clause {
				fn serialize(self, out: &mut ObjValueBuilder) -> JrResult<()> {
					#(#fields_serialize)*

					Ok(())
				}
			}
		};
	})
}
/*#[allow(clippy::too_many_lines)]
fn derive_obj_layer(input: DeriveInput) -> Result<TokenStream> {
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

	let typed = {
		quote! {
			impl #impl_generics IntoUntyped for #ident #ty_generics #where_clause {
				fn from_untyped(value: Val) -> JrResult<Self> {
					let obj = value.as_obj().expect("shape is correct");
					Self::parse(&obj)
				}

				fn into_untyped(value: Self) -> JrResult<Val> {
					let mut out = ObjValueBuilder::new();
					value.serialize(&mut out)?;
					Ok(Val::Obj(out.build()))
				}

			}
		}
	};

	let field_strings = fields.iter().filter_map(|f| {
		let ident = format_ident!("s_{}", f.ident);
		let name = f.name()?;
		Some(quote!(#ident: #name))
	});

	let fields_enum_inner = itertools::intersperse(
		fields.iter().map(|f| {
			let s_ident = format_ident!("s_{}", f.ident);
			let ident = &f.ident;
			let v = if f.attr.hide {
				quote!(Visibility::Hidden)
			} else {
				quote!(Visibility::Normal)
			};
			// TODO: flatten(ok)
			if f.attr.flatten {
				let t = &f.ty;
				// FIXME: In case of empty flattened object, field_index is still incremented.
				// I think it is ok, as nothing depends on field_index being without gaps, yet it would
				// be better to avoid it.
				quote! {
					if !<#t as TypedObj>::enum_fields_inner(&self.#ident, field_index, super_depth, handler) {
						return false;
					}
				}
			} else {
				quote! {
					if !handler (*super_depth, field_index, #s_ident(), #v) {
						return false;
					}
				}
			}
		}),
		quote!(field_index.next();),
	)
	.collect_vec();

	let field_idents = fields.iter().map(|f| {
		let ident = format_ident!("s_{}", f.ident);
		quote!(#ident())
	});
	let field_visibility = fields.iter().map(|f| {
		let ident = format_ident!("s_{}", f.ident);
		let v = if f.attr.hide {
			quote!(Visibility::Hidden)
		} else {
			quote!(Visibility::Normal)
		};
		quote!(if field == #ident() {
			return Some(#v);
		})
	});
	let get = fields.iter().map(|f| {
		let ident = format_ident!("s_{}", f.ident);
		let i = &f.ident;
		quote!(if key == #ident() {
			return Ok(Some((IntoVal::into_untyped_val(&self.#i)?, ValueProcess {
				// TODO: Addable typed
				add: false,
			})))
		})
	});
	// .interleave(quote!(field_index.next();));

	Ok(quote! {
		const _: () = {
			use ::jrsonnet_evaluator::typed_macro_prelude::*;

			#typed

			strings! {
				#(#field_strings),*
			}

			impl #impl_generics TypedObj for #ident #ty_generics #where_clause {
				#[inline]
				fn enum_fields_inner(
					&self,
					field_index: &mut FieldIndex,
					super_depth: &mut SuperDepth,
					handler: &mut EnumFieldsHandler<'_>,
				) -> bool {
					#(#fields_enum_inner)*

					true
				}
				// fn parse(obj: &ObjValue) -> JrResult<Self> {
				// 	Ok(Self {
				// 		#(#fields_parse)*
				// 	})
				// }
			}

			impl #impl_generics ObjectLayer for #ident #ty_generics #where_clause {
				fn enum_fields_core(
					&self,
					super_depth: &mut SuperDepth,
					handler: &mut EnumFieldsHandler<'_>,
				) -> bool {
					let mut field_index = FieldIndex::default();
					<Self as TypedObj>::enum_fields_inner(self, &mut field_index, super_depth, handler)
				}
				fn run_assertions_raw(&self, _sup_this: SupThis) -> Result<()> {
					Ok(())
				}
				fn has_field_include_hidden(&self, name: IStr) -> bool {
					#(if name == #field_idents {
						return true
					})*
					false
				}

				fn get_for(
					&self,
					key: IStr,
					_sup_this: SupThis,
					_do_cache: &mut bool,
				) -> Result<Option<(Val, ValueProcess)>> {
					Ok({
						#(#get)*
						None
					})
				}

				fn field_visibility(&self, field: IStr) -> Option<Visibility> {
					#(#field_visibility)*
					None
				}
			}
		};
	})
}*/

struct FormatInput {
	formatting: LitStr,
	arguments: Vec<Expr>,
}
impl Parse for FormatInput {
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		let formatting = input.parse()?;
		let mut arguments = Vec::new();

		while input.peek(Token![,]) {
			input.parse::<Token![,]>()?;
			if input.is_empty() {
				// Trailing comma
				break;
			}
			let expr = input.parse()?;
			arguments.push(expr);
		}

		if !input.is_empty() {
			return Err(syn::Error::new(input.span(), "unexpected trailing input"));
		}

		Ok(Self {
			formatting,
			arguments,
		})
	}
}
fn is_format_str(i: &str) -> bool {
	let mut is_plain = true;
	// -1 = {
	// +1 = }
	let mut is_bracket = 0i8;
	for ele in i.chars() {
		match ele {
			'{' if is_bracket == -1 => {
				is_bracket = 0;
			}
			'}' if is_bracket == -1 => {
				is_plain = false;
				break;
			}
			'}' if is_bracket == 1 => {
				is_bracket = 0;
			}
			'{' if is_bracket == 1 => {
				is_plain = false;
				break;
			}
			'{' => {
				is_bracket = -1;
			}
			'}' => {
				is_bracket = 1;
			}
			_ if is_bracket != 0 => {
				is_plain = false;
				break;
			}
			_ => {}
		}
	}
	!is_plain || is_bracket != 0
}
impl FormatInput {
	fn expand(self) -> TokenStream {
		let format = self.formatting;
		if is_format_str(&format.value()) {
			let args = self.arguments;
			quote! {
				::jrsonnet_evaluator::IStr::from(format!(#format #(, #args)*))
			}
		} else {
			if let Some(first) = self.arguments.first() {
				return syn::Error::new(
					first.span(),
					"string has no formatting codes, it should not have the arguments",
				)
				.into_compile_error();
			}
			quote! {
				::jrsonnet_evaluator::IStr::from(#format)
			}
		}
	}
}

/// `IStr` formatting helper
///
/// Using `format!("literal with no codes").into()` is slower than just `"literal with no codes".into()`
/// This macro looks for formatting codes in the input string, and uses
/// `format!()` only when necessary
#[proc_macro]
pub fn format_istr(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let input = parse_macro_input!(input as FormatInput);
	input.expand().into()
}

/// Create Thunk using closure syntax
#[proc_macro]
#[allow(non_snake_case)]
pub fn Thunk(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let input = parse_macro_input!(input as ExprClosure);

	let span = input.inputs.span();
	let move_check = input.capture.is_none().then(|| {
		quote_spanned! {span => {
			compile_error!("Thunk! needs to be called with move closure");
		}}
	});

	let (env, closure, args) = syn_dissect_closure::split_env(input);

	let trace_check = args.iter().map(|el| {
		let span = el.span();
		quote_spanned! {span => ::jrsonnet_evaluator::gc::assert_trace(&#el);}
	});

	quote! {{
		#move_check
		#(#trace_check)*
		::jrsonnet_evaluator::Thunk::new_cached(::jrsonnet_evaluator::val::ThunkValueClosure::new(#env, #closure))
	}}.into()
}
