use quote::{quote, quote_spanned};
use syn::{
	parenthesized, parse::Parse, parse_macro_input, punctuated::Punctuated, spanned::Spanned,
	token::Comma, DeriveInput, FnArg, GenericArgument, Ident, ItemFn, Pat, PatType, Path,
	PathArguments, Token, Type,
};

fn is_location_arg(t: &PatType) -> bool {
	t.attrs.iter().any(|a| a.path.is_ident("location"))
}
fn is_self_arg(t: &PatType) -> bool {
	t.attrs.iter().any(|a| a.path.is_ident("self"))
}

trait RetainHad<T> {
	fn retain_had(&mut self, h: impl FnMut(&T) -> bool) -> bool;
}
impl<T> RetainHad<T> for Vec<T> {
	fn retain_had(&mut self, h: impl FnMut(&T) -> bool) -> bool {
		let before = self.len();
		self.retain(h);
		let after = self.len();
		before != after
	}
}

fn extract_type_from_option(ty: &Type) -> Option<&Type> {
	fn path_is_option(path: &Path) -> bool {
		path.leading_colon.is_none()
			&& path.segments.len() == 1
			&& path.segments.iter().next().unwrap().ident == "Option"
	}

	match ty {
		Type::Path(typepath) if typepath.qself.is_none() && path_is_option(&typepath.path) => {
			// Get the first segment of the path (there is only one, in fact: "Option"):
			let type_params = &typepath.path.segments.iter().next().unwrap().arguments;
			// It should have only on angle-bracketed param ("<String>"):
			let generic_arg = match type_params {
				PathArguments::AngleBracketed(params) => params.args.iter().next().unwrap(),
				_ => panic!("missing option generic"),
			};
			// This argument must be a type:
			match generic_arg {
				GenericArgument::Type(ty) => Some(ty),
				_ => panic!("option generic should be a type"),
			}
		}
		_ => None,
	}
}

struct Field {
	name: Ident,
	_colon: Token![:],
	ty: Type,
}
impl Parse for Field {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		Ok(Self {
			name: input.parse()?,
			_colon: input.parse()?,
			ty: input.parse()?,
		})
	}
}

mod kw {
	syn::custom_keyword!(fields);
}

struct BuiltinAttrs {
	fields: Vec<Field>,
}
impl Parse for BuiltinAttrs {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
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

#[proc_macro_attribute]
pub fn builtin(
	attr: proc_macro::TokenStream,
	item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
	let attrs = parse_macro_input!(attr as BuiltinAttrs);
	let mut fun: ItemFn = parse_macro_input!(item);

	let result = match fun.sig.output {
		syn::ReturnType::Default => {
			return quote_spanned! { fun.sig.span() =>
				compile_error!("builtins should return something");
			}
			.into()
		}
		syn::ReturnType::Type(_, ref ty) => ty.clone(),
	};

	let params = fun
		.sig
		.inputs
		.iter()
		.map(|i| match i {
			FnArg::Receiver(_) => unreachable!(),
			FnArg::Typed(t) => t,
		})
		.filter(|a| !is_location_arg(a) && !is_self_arg(a))
		.map(|t| {
			let ident = match &t.pat as &Pat {
				Pat::Ident(i) => i.ident.to_string(),
				_ => {
					return quote_spanned! { t.pat.span() =>
						compile_error!("args should be plain identifiers")
					}
					.into()
				}
			};
			let optional = extract_type_from_option(&t.ty).is_some();
			quote! {
				BuiltinParam {
					name: std::borrow::Cow::Borrowed(#ident),
					has_default: #optional,
				}
			}
		})
		.collect::<Vec<_>>();

	let args = fun
		.sig
		.inputs
		.iter_mut()
		.map(|i| match i {
			FnArg::Receiver(_) => unreachable!(),
			FnArg::Typed(t) => t,
		})
		.map(|t| {
			if t.attrs.retain_had(|a| !a.path.is_ident("location")) {
				quote! {{
					loc
				}}
			} else if t.attrs.retain_had(|a| !a.path.is_ident("self")) {
				quote! {{
					self
				}}
			} else {
				let ident = match &t.pat as &Pat {
					Pat::Ident(i) => i.ident.to_string(),
					_ => {
						return quote_spanned! { t.pat.span() =>
							compile_error!("args should be plain identifiers")
						}
						.into()
					}
				};
				let ty = &t.ty;
				if let Some(opt_ty) = extract_type_from_option(&t.ty) {
					quote! {{
						if let Some(value) = parsed.get(#ident) {
							Some(::jrsonnet_evaluator::push_description_frame(
								|| format!("argument <{}> evaluation", #ident),
								|| <#opt_ty>::try_from(value.evaluate()?),
							)?)
						} else {
							None
						}
					}}
				} else {
					quote! {{
						let value = parsed.get(#ident).unwrap();

						::jrsonnet_evaluator::push_description_frame(
							|| format!("argument <{}> evaluation", #ident),
							|| <#ty>::try_from(value.evaluate()?),
						)?
					}}
				}
			}
		})
		.collect::<Vec<_>>();

	let fields = attrs.fields.iter().map(|field| {
		let name = &field.name;
		let ty = &field.ty;
		quote! {
			pub #name: #ty,
		}
	});

	let name = &fun.sig.ident;
	let vis = &fun.vis;
	let static_ext = if attrs.fields.is_empty() {
		quote! {
			impl #name {
				pub const INST: &'static dyn StaticBuiltin = &#name {};
			}
			impl StaticBuiltin for #name {}
		}
	} else {
		quote! {}
	};
	let static_derive_copy = if attrs.fields.is_empty() {
		quote! {, Copy}
	} else {
		quote! {}
	};

	(quote! {
		#fun
		#[doc(hidden)]
		#[allow(non_camel_case_types)]
		#[derive(Clone, gcmodule::Trace #static_derive_copy)]
		#vis struct #name {
			#(#fields)*
		}
		const _: () = {
			use ::jrsonnet_evaluator::{
				function::{Builtin, StaticBuiltin, BuiltinParam, ArgsLike, parse_builtin_call},
				error::Result, Context,
				parser::ExprLocation,
			};
			const PARAMS: &'static [BuiltinParam] = &[
				#(#params),*
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
				fn call(&self, context: Context, loc: Option<&ExprLocation>, args: &dyn ArgsLike) -> Result<Val> {
					let parsed = parse_builtin_call(context, &PARAMS, args, false)?;

					let result: #result = #name(#(#args),*);
					let result = result?;
					result.try_into()
				}
			}
		};
	})
	.into()
}

#[proc_macro_derive(Typed)]
pub fn derive_typed(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let input = parse_macro_input!(item as DeriveInput);
	let data = match &input.data {
		syn::Data::Struct(s) => s,
		_ => {
			return syn::Error::new(input.span(), "only structs supported")
				.to_compile_error()
				.into()
		}
	};

	let ident = &input.ident;

	let fields_def = data.fields.iter().map(|f| {
		let name = f
			.ident
			.as_ref()
			.expect("only named fields supported")
			.to_string();
		let ty = &f.ty;
		quote! {
			(#name, #ty::TYPE),
		}
	});
	let fields_parse = data.fields.iter().map(|f| {
		let ident = f.ident.as_ref().unwrap();
		let name = ident.to_string();
		let ty = &f.ty;
		quote! {
			#ident: #ty::try_from(obj.get(#name.into())?.expect("shape is correct"))?,
		}
	});
	let fields_serialize = data.fields.iter().map(|f| {
		let ident = f.ident.as_ref().unwrap();
		let name = ident.to_string();
		quote! {
			out.member(#name.into()).value(self.#ident.try_into()?);
		}
	});
	let field_count = data.fields.len();

	quote! {
		const _: () = {
			use ::jrsonnet_evaluator::{
				typed::{ComplexValType, Typed, CheckType},
				Val,
				error::LocError,
				obj::ObjValueBuilder,
			};

			const ITEMS: [(&'static str, &'static ComplexValType); #field_count] = [
				#(#fields_def)*
			];
			impl Typed for #ident {
				const TYPE: &'static ComplexValType = &ComplexValType::ObjectRef(&ITEMS);
			}

			impl TryFrom<Val> for #ident {
				type Error = LocError;
				fn try_from(value: Val) -> Result<Self, Self::Error> {
					<Self as Typed>::TYPE.check(&value)?;
					let obj = value.as_obj().expect("shape is correct");

					Ok(Self {
						#(#fields_parse)*
					})
				}
			}
			impl TryInto<Val> for #ident {
				type Error = LocError;
				fn try_into(self) -> Result<Val, Self::Error> {
					let mut out = ObjValueBuilder::new();
					#(#fields_serialize)*
					Ok(Val::Obj(out.build()))
				}
			}
			()
		};
	}
	.into()
}
