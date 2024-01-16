use proc_macro2::TokenStream;
use quote::quote;
use syn::{
	parenthesized,
	parse::{Parse, ParseStream},
	parse_macro_input,
	punctuated::Punctuated,
	spanned::Spanned,
	token::{self, Comma},
	Attribute, DeriveInput, Error, Expr, FnArg, GenericArgument, Ident, ItemFn, LitStr, Pat, Path,
	PathArguments, Result, ReturnType, Token, Type,
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
	fn parse(input: ParseStream) -> syn::Result<Self> {
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
	syn::custom_keyword!(flatten);
	syn::custom_keyword!(add);
	syn::custom_keyword!(hide);
	syn::custom_keyword!(ok);
}

struct EmptyAttr;
impl Parse for EmptyAttr {
	fn parse(_input: ParseStream) -> Result<Self> {
		Ok(Self)
	}
}

struct BuiltinAttrs {
	fields: Vec<Field>,
}
impl Parse for BuiltinAttrs {
	fn parse(input: ParseStream) -> syn::Result<Self> {
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

enum ArgInfo {
	Normal {
		ty: Box<Type>,
		is_option: bool,
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
	fn parse(name: &str, arg: &FnArg) -> Result<Self> {
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

		let (is_option, ty) = if let Some(ty) = extract_type_from_option(ty)? {
			if type_is_path(ty, "Thunk").is_some() {
				return Ok(Self::Lazy {
					is_option: true,
					name: ident.map(|v| v.to_string()),
				});
			}

			(true, Box::new(ty.clone()))
		} else {
			(false, ty.clone())
		};

		let cfg_attrs = arg
			.attrs
			.iter()
			.filter(|a| a.path().is_ident("cfg"))
			.cloned()
			.collect();

		Ok(Self::Normal {
			ty,
			is_option,
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
	let item_fn = item.clone();
	let item_fn: ItemFn = parse_macro_input!(item_fn);

	match builtin_inner(attr, item_fn, item.into()) {
		Ok(v) => v.into(),
		Err(e) => e.into_compile_error().into(),
	}
}

fn builtin_inner(
	attr: BuiltinAttrs,
	fun: ItemFn,
	item: proc_macro2::TokenStream,
) -> syn::Result<TokenStream> {
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
		.iter()
		.map(|arg| ArgInfo::parse(&name, arg))
		.collect::<Result<Vec<_>>>()?;

	let params_desc = args.iter().flat_map(|a| match a {
		ArgInfo::Normal {
			is_option,
			name,
			cfg_attrs,
			..
		} => {
			let name = name
				.as_ref()
				.map(|n| quote! {ParamName::new_static(#n)})
				.unwrap_or_else(|| quote! {None});
			Some(quote! {
				#(#cfg_attrs)*
				BuiltinParam::new(#name, #is_option),
			})
		}
		ArgInfo::Lazy { is_option, name } => {
			let name = name
				.as_ref()
				.map(|n| quote! {ParamName::new_static(#n)})
				.unwrap_or_else(|| quote! {None});
			Some(quote! {
				BuiltinParam::new(#name, #is_option),
			})
		}
		ArgInfo::Context => None,
		ArgInfo::Location => None,
		ArgInfo::This => None,
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
				is_option,
				name,
				cfg_attrs,
			} => {
				let name = name.as_ref().map(|v| v.as_str()).unwrap_or("<unnamed>");
				let eval = quote! {jrsonnet_evaluator::State::push_description(
					|| format!("argument <{}> evaluation", #name),
					|| <#ty>::from_untyped(value.evaluate()?),
				)?};
				let value = if *is_option {
					quote! {if let Some(value) = &parsed[#id] {
						Some(#eval)
					} else {
						None
					},}
				} else {
					quote! {{
						let value = parsed[#id].as_ref().expect("args shape is checked");
						#eval
					},}
				};
				quote! {
					#(#cfg_attrs)*
					#value
				}
			}
			ArgInfo::Lazy { is_option, .. } => {
				if *is_option {
					quote! {if let Some(value) = &parsed[#id] {
						Some(value.clone())
					} else {
						None
					}}
				} else {
					quote! {
						parsed[#id].as_ref().expect("args shape is correct").clone(),
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
		// FIXME: Make possible to implement Copy on type with no destructor
		// in boa_gc
		quote! {}
	} else {
		quote! {}
	};

	Ok(quote! {
		#item

		#[doc(hidden)]
		#[allow(non_camel_case_types)]
		#[derive(Clone, boa_gc::Trace, boa_gc::Finalize #static_derive_copy)]
		#vis struct #name {
			#(#fields)*
		}
		const _: () = {
			use ::jrsonnet_evaluator::{
				State, Val,
				function::{builtin::{Builtin, StaticBuiltin, BuiltinParam, ParamName}, CallLocation, ArgsLike, parse::parse_builtin_call},
				Result, Context, typed::Typed,
				parser::ExprLocation,
			};
			const PARAMS: &'static [BuiltinParam] = &[
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
				fn params(&self) -> &[BuiltinParam] {
					PARAMS
				}
				fn call(&self, ctx: Context, location: CallLocation, args: &dyn ArgsLike) -> Result<Val> {
					let parsed = parse_builtin_call(ctx.clone(), &PARAMS, args, false)?;

					let result: #result = #name(#(#pass)*);
					<_ as Typed>::into_result(result)
				}
				fn as_any(&self) -> &dyn ::std::any::Any {
					self
				}
			}
		};
	})
}

#[derive(Default)]
struct TypedAttr {
	rename: Option<String>,
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
		let (is_option, ty) = if let Some(ty) = extract_type_from_option(&field.ty)? {
			(true, ty.clone())
		} else {
			(false, field.ty.clone())
		};
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

		Ok(Self {
			attr,
			ident,
			ty,
			is_option,
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
		let ident = &self.ident;
		let ty = &self.ty;
		if self.attr.flatten {
			// optional flatten is handled in same way as serde
			return if self.is_option {
				quote! {
					#ident: <#ty as TypedObj>::parse(&obj).ok(),
				}
			} else {
				quote! {
					#ident: <#ty as TypedObj>::parse(&obj)?,
				}
			};
		};

		let name = self.name().unwrap();
		let value = if self.is_option {
			quote! {
				if let Some(value) = obj.get(#name.into())? {
					Some(<#ty as Typed>::from_untyped(value)?)
				} else {
					None
				}
			}
		} else {
			quote! {
				<#ty as Typed>::from_untyped(obj.get(#name.into())?.ok_or_else(|| ErrorKind::NoSuchField(#name.into(), vec![]))?)?
			}
		};

		quote! {
			#ident: #value,
		}
	}
	fn expand_serialize(&self) -> Result<TokenStream> {
		let ident = &self.ident;
		let ty = &self.ty;
		Ok(if let Some(name) = self.name() {
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
			if self.is_option {
				quote! {
					if let Some(value) = self.#ident {
						out.field(#name)
							#hide
							#add
							.try_value(<#ty as Typed>::into_untyped(value)?)?;
					}
				}
			} else {
				quote! {
					out.field(#name)
						#hide
						#add
						.try_value(<#ty as Typed>::into_untyped(self.#ident)?)?;
				}
			}
		} else if self.is_option {
			quote! {
				if let Some(value) = self.#ident {
					<#ty as TypedObj>::serialize(value, out)?;
				}
			}
		} else {
			quote! {
				<#ty as TypedObj>::serialize(self.#ident, out)?;
			}
		})
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

	let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

	let typed = {
		let fields = fields
			.iter()
			.flat_map(TypedField::expand_field)
			.collect::<Vec<_>>();
		quote! {
			impl #impl_generics Typed for #ident #ty_generics #where_clause {
				const TYPE: &'static ComplexValType = &ComplexValType::ObjectRef(&[
					#(#fields,)*
				]);

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

	let fields_parse = fields.iter().map(TypedField::expand_parse);
	let fields_serialize = fields
		.iter()
		.map(TypedField::expand_serialize)
		.collect::<Result<Vec<_>>>()?;

	Ok(quote! {
		const _: () = {
			use ::jrsonnet_evaluator::{
				typed::{ComplexValType, Typed, TypedObj, CheckType},
				Val, State,
				error::{ErrorKind, Result as JrResult},
				ObjValueBuilder, ObjValue,
			};

			#typed

			impl #impl_generics TypedObj for #ident #ty_generics #where_clause {
				fn serialize(self, out: &mut ObjValueBuilder) -> JrResult<()> {
					#(#fields_serialize)*

					Ok(())
				}
				fn parse(obj: &ObjValue) -> JrResult<Self> {
					Ok(Self {
						#(#fields_parse)*
					})
				}
			}
		};
	})
}

struct FormatInput {
	formatting: LitStr,
	arguments: Vec<Expr>,
}
impl Parse for FormatInput {
	fn parse(input: ParseStream) -> Result<Self> {
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

/// IStr formatting helper
///
/// Using `format!("literal with no codes").into()` is slower than just `"literal with no codes".into()`
/// This macro looks for formatting codes in the input string, and uses
/// `format!()` only when necessary
#[proc_macro]
pub fn format_istr(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let input = parse_macro_input!(input as FormatInput);
	input.expand().into()
}
