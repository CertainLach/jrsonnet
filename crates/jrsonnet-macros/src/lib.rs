use quote::quote;
use syn::{
	parse_macro_input, FnArg, GenericArgument, ItemFn, Pat, PatType, Path, PathArguments, Type,
};

fn is_location_arg(t: &PatType) -> bool {
	t.attrs.iter().any(|a| a.path.is_ident("location"))
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

#[proc_macro_attribute]
pub fn builtin(
	_attr: proc_macro::TokenStream,
	item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
	// syn::ItemFn::parse(input)
	let mut fun: ItemFn = parse_macro_input!(item);

	let result = match fun.sig.output {
		syn::ReturnType::Default => panic!("builtin should return something"),
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
		.filter(|a| !is_location_arg(a))
		.map(|t| {
			let ident = match &t.pat as &Pat {
				Pat::Ident(i) => i.ident.to_string(),
				_ => panic!("only idents supported yet"),
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
			let is_location = t.attrs.retain_had(|a| !a.path.is_ident("location"));
			if is_location {
				quote! {{
					loc
				}}
			} else {
				let ident = match &t.pat as &Pat {
					Pat::Ident(i) => i.ident.to_string(),
					_ => panic!("only idents supported yet"),
				};
				let ty = &t.ty;
				if let Some(opt_ty) = extract_type_from_option(&t.ty) {
					quote! {{
						if let Some(value) = parsed.get(#ident) {
							Some(jrsonnet_evaluator::push_description_frame(
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

						jrsonnet_evaluator::push_description_frame(
							|| format!("argument <{}> evaluation", #ident),
							|| <#ty>::try_from(value.evaluate()?),
						)?
					}}
				}
			}
		})
		.collect::<Vec<_>>();

	let name = &fun.sig.ident;
	let vis = &fun.vis;
	(quote! {
		#fun
		#[doc(hidden)]
		#[allow(non_camel_case_types)]
		#[derive(Clone, Copy, gcmodule::Trace)]
		#vis struct #name {}
		const _: () = {
			use jrsonnet_evaluator::function::{Builtin, StaticBuiltin, BuiltinParam, ArgsLike};
			const PARAMS: &'static [BuiltinParam] = &[
				#(#params),*
			];

			impl #name {
				pub const INST: &'static dyn StaticBuiltin = &#name {};
			}
			impl StaticBuiltin for #name {}
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
					let parsed = jrsonnet_evaluator::function::parse_builtin_call(context, &PARAMS, args, false)?;

					let result: #result = #name(#(#args),*);
					let result = result?;
					result.try_into()
				}
			}
		};
	})
	.into()
}
