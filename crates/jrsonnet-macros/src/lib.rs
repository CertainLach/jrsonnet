use proc_macro2::TokenStream;
use quote::quote;
use syn::{
	parenthesized,
	parse::{Parse, ParseStream},
	parse_macro_input,
	punctuated::Punctuated,
	spanned::Spanned,
	token::Comma,
	Attribute, DeriveInput, Error, FnArg, GenericArgument, Ident, ItemFn, LitStr, Pat, Path,
	PathArguments, Result, ReturnType, Token, Type,
};

fn parse_attr<A: Parse, I>(attrs: &[Attribute], ident: I) -> Result<Option<A>>
where
	Ident: PartialEq<I>,
{
	let attrs = attrs
		.iter()
		.filter(|a| a.path.is_ident(&ident))
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
	Ok(if let Some(args) = type_is_path(ty, "Option") {
		// It should have only on angle-bracketed param ("<String>"):
		let generic_arg = match args {
			PathArguments::AngleBracketed(params) => params.args.iter().next().unwrap(),
			_ => return Err(Error::new(args.span(), "missing option generic")),
		};
		// This argument must be a type:
		match generic_arg {
			GenericArgument::Type(ty) => Some(ty),
			_ => {
				return Err(Error::new(
					generic_arg.span(),
					"option generic should be a type",
				))
			}
		}
	} else {
		None
	})
}

struct Field {
	name: Ident,
	_colon: Token![:],
	ty: Type,
}
impl Parse for Field {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		Ok(Self {
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
		name: String,
		cfg_attrs: Vec<Attribute>,
		// ident: Ident,
	},
	Lazy {
		is_option: bool,
		name: String,
	},
	State,
	Location,
	This,
}

impl ArgInfo {
	fn parse(arg: &FnArg) -> Result<Self> {
		let arg = match arg {
			FnArg::Receiver(_) => unreachable!(),
			FnArg::Typed(a) => a,
		};
		let ident = match &arg.pat as &Pat {
			Pat::Ident(i) => i.ident.clone(),
			_ => return Err(Error::new(arg.pat.span(), "arg should be plain identifier")),
		};
		let ty = &arg.ty;
		if type_is_path(ty, "State").is_some() {
			return Ok(Self::State);
		} else if type_is_path(ty, "CallLocation").is_some() {
			return Ok(Self::Location);
		} else if type_is_path(ty, "Self").is_some() {
			return Ok(Self::This);
		} else if type_is_path(ty, "LazyVal").is_some() {
			return Ok(Self::Lazy {
				is_option: false,
				name: ident.to_string(),
			});
		}

		let (is_option, ty) = if let Some(ty) = extract_type_from_option(ty)? {
			if type_is_path(ty, "LazyVal").is_some() {
				return Ok(Self::Lazy {
					is_option: true,
					name: ident.to_string(),
				});
			}

			(true, Box::new(ty.clone()))
		} else {
			(false, ty.clone())
		};

		let cfg_attrs = arg
			.attrs
			.iter()
			.filter(|a| a.path.is_ident("cfg"))
			.cloned()
			.collect();

		Ok(Self::Normal {
			ty,
			is_option,
			name: ident.to_string(),
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
	let item: ItemFn = parse_macro_input!(item);

	match builtin_inner(attr, item) {
		Ok(v) => v.into(),
		Err(e) => e.into_compile_error().into(),
	}
}

fn builtin_inner(attr: BuiltinAttrs, fun: ItemFn) -> syn::Result<TokenStream> {
	let result = match fun.sig.output {
		ReturnType::Default => {
			return Err(Error::new(
				fun.sig.span(),
				"builtin should return something",
			))
		}
		ReturnType::Type(_, ref ty) => ty.clone(),
	};
	let result_inner = if let Some(args) = type_is_path(&result, "Result") {
		let generic_arg = match args {
			PathArguments::AngleBracketed(params) => params.args.iter().next().unwrap(),
			_ => return Err(Error::new(args.span(), "missing result generic")),
		};
		// This argument must be a type:
		match generic_arg {
			GenericArgument::Type(ty) => ty,
			_ => {
				return Err(Error::new(
					generic_arg.span(),
					"option generic should be a type",
				))
			}
		}
	} else {
		return Err(Error::new(result.span(), "return value should be result"));
	};

	let args = fun
		.sig
		.inputs
		.iter()
		.map(ArgInfo::parse)
		.collect::<Result<Vec<_>>>()?;

	let params_desc = args.iter().flat_map(|a| match a {
		ArgInfo::Normal {
			is_option,
			name,
			cfg_attrs,
			..
		} => Some(quote! {
			#(#cfg_attrs)*
			BuiltinParam {
				name: std::borrow::Cow::Borrowed(#name),
				has_default: #is_option,
			},
		}),
		ArgInfo::Lazy { is_option, name } => Some(quote! {
			BuiltinParam {
				name: std::borrow::Cow::Borrowed(#name),
				has_default: #is_option,
			},
		}),
		ArgInfo::State => None,
		ArgInfo::Location => None,
		ArgInfo::This => None,
	});

	let pass = args.iter().map(|a| match a {
		ArgInfo::Normal {
			ty,
			is_option,
			name,
			cfg_attrs,
		} => {
			let eval = quote! {s.push_description(
				|| format!("argument <{}> evaluation", #name),
				|| <#ty>::from_untyped(value.evaluate(s.clone())?, s.clone()),
			)?};
			let value = if *is_option {
				quote! {if let Some(value) = parsed.get(#name) {
					Some(#eval)
				} else {
					None
				},}
			} else {
				quote! {{
					let value = parsed.get(#name).expect("args shape is checked");
					#eval
				},}
			};
			quote! {
				#(#cfg_attrs)*
				#value
			}
		}
		ArgInfo::Lazy { is_option, name } => {
			if *is_option {
				quote! {if let Some(value) = parsed.get(#name) {
					Some(value.clone())
				} else {
					None
				}}
			} else {
				quote! {
					parsed.get(#name).expect("args shape is correct").clone(),
				}
			}
		}
		ArgInfo::State => quote! {s.clone(),},
		ArgInfo::Location => quote! {location,},
		ArgInfo::This => quote! {self,},
	});

	let fields = attr.fields.iter().map(|field| {
		let name = &field.name;
		let ty = &field.ty;
		quote! {
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
		#[derive(Clone, gcmodule::Trace #static_derive_copy)]
		#vis struct #name {
			#(#fields)*
		}
		const _: () = {
			use ::jrsonnet_evaluator::{
				State,
				function::{Builtin, CallLocation, StaticBuiltin, BuiltinParam, ArgsLike, parse_builtin_call},
				error::Result, Context,
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
				fn call(&self, s: State, ctx: Context, location: CallLocation, args: &dyn ArgsLike) -> Result<Val> {
					let parsed = parse_builtin_call(s.clone(), ctx, &PARAMS, args, false)?;

					let result: #result = #name(#(#pass)*);
					let result = result?;
					<#result_inner>::into_untyped(result, s)
				}
			}
		};
	})
}

#[derive(Default)]
struct TypedAttr {
	rename: Option<String>,
	flatten: bool,
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
		// input.parse::<kw::rename>()?;
		// input.parse::<Token![=]>()?;
		// let rename = input.parse::<LitStr>()?.value();
		Ok(out)
	}
}

struct TypedField<'f>(&'f syn::Field, TypedAttr);
impl<'f> TypedField<'f> {
	fn try_new(field: &'f syn::Field) -> Result<Self> {
		let attr = parse_attr::<TypedAttr, _>(&field.attrs, "typed")?.unwrap_or_default();
		if field.ident.is_none() {
			return Err(Error::new(
				field.span(),
				"this field should appear in output object, but it has no visible name",
			));
		}
		Ok(Self(field, attr))
	}
	fn ident(&self) -> Ident {
		self.0
			.ident
			.clone()
			.expect("constructor disallows fields without name")
	}
	/// None if this field is flattened in jsonnet output
	fn name(&self) -> Option<String> {
		if self.1.flatten {
			return None;
		}
		Some(
			self.1
				.rename
				.clone()
				.unwrap_or_else(|| self.ident().to_string()),
		)
	}

	fn expand_field(&self) -> Option<TokenStream> {
		if self.is_option() {
			return None;
		}
		let name = self.name()?;
		let ty = &self.0.ty;
		Some(quote! {
			(#name, <#ty>::TYPE)
		})
	}
	fn expand_parse(&self) -> TokenStream {
		let ident = self.ident();
		let ty = &self.0.ty;
		if self.1.flatten {
			// optional flatten is handled in same way as serde
			return if self.is_option() {
				quote! {
					#ident: <#ty>::parse(&obj).ok(),
				}
			} else {
				quote! {
					#ident: <#ty>::parse(&obj)?,
				}
			};
		};

		let name = self.name().unwrap();
		let value = if let Some(ty) = self.as_option() {
			quote! {
				if let Some(value) = obj.get(#name.into())? {
					Some(<#ty>::try_from(vakue)?)
				} else {
					None
				}
			}
		} else {
			quote! {
				<#ty>::try_from(obj.get(#name.into())?.ok_or_else(|| Error::NoSuchField(#name.into()))?)?
			}
		};

		quote! {
			#ident: #value,
		}
	}
	fn expand_serialize(&self) -> TokenStream {
		let ident = self.ident();
		if let Some(name) = self.name() {
			if self.is_option() {
				quote! {
					if let Some(value) = self.#ident {
						out.member(#name.into()).value(value.try_into()?)?;
					}
				}
			} else {
				quote! {
					out.member(#name.into()).value(self.#ident.try_into()?)?;
				}
			}
		} else if self.is_option() {
			quote! {
				if let Some(value) = self.#ident {
					value.serialize(out)?;
				}
			}
		} else {
			quote! {
				self.#ident.serialize(out)?;
			}
		}
	}

	fn as_option(&self) -> Option<&Type> {
		extract_type_from_option(&self.0.ty).unwrap()
	}
	fn is_option(&self) -> bool {
		self.as_option().is_some()
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
	let data = match &input.data {
		syn::Data::Struct(s) => s,
		_ => return Err(Error::new(input.span(), "only structs supported")),
	};

	let ident = &input.ident;
	let fields = data
		.fields
		.iter()
		.map(TypedField::try_new)
		.collect::<Result<Vec<_>>>()?;

	let typed = {
		let fields = fields
			.iter()
			.flat_map(TypedField::expand_field)
			.collect::<Vec<_>>();
		let len = fields.len();
		quote! {
			const ITEMS: [(&'static str, &'static ComplexValType); #len] = [
				#(#fields,)*
			];
			impl Typed for #ident {
				const TYPE: &'static ComplexValType = &ComplexValType::ObjectRef(&ITEMS);

				fn from_untyped(value: Val, s: State) -> Result<Self> {
					let obj = value.as_obj().expect("shape is correct");
					Self::parse(&obj)
				}

				fn into_untyped(value: Self, s: State) -> Result<Val> {
					let mut out = ObjValueBuilder::new();
					value.serialize(&mut out)?;
					Ok(Val::Obj(out.build()))
				}

			}
		}
	};

	let fields_parse = fields.iter().map(TypedField::expand_parse);
	let fields_serialize = fields.iter().map(TypedField::expand_serialize);

	Ok(quote! {
		const _: () = {
			use ::jrsonnet_evaluator::{
				typed::{ComplexValType, Typed, TypedObj, CheckType},
				Val,
				error::{LocError, Error},
				ObjValueBuilder, ObjValue,
			};

			#typed

			impl #ident {
				fn serialize(self, out: &mut ObjValueBuilder) -> Result<(), LocError> {
					#(#fields_serialize)*

					Ok(())
				}
				fn parse(obj: &ObjValue) -> Result<Self, LocError> {
					Ok(Self {
						#(#fields_parse)*
					})
				}
			}
		};
	})
}
