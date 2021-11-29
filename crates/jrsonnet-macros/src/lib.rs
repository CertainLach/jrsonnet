use proc_macro2::Span;
use quote::quote;
use syn::{parse_macro_input, FnArg, Ident, ItemFn, Pat};

#[proc_macro_attribute]
pub fn builtin(
	_attr: proc_macro::TokenStream,
	item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
	// syn::ItemFn::parse(input)
	let fun: ItemFn = parse_macro_input!(item);

	let inner_name = Ident::new("inner", Span::call_site());
	let mut inner_fun = fun.clone();
	inner_fun.sig.ident = inner_name.clone();
	let result = match fun.sig.output {
		syn::ReturnType::Default => panic!("builtin should return something"),
		syn::ReturnType::Type(_, ty) => ty,
	};

	let params = fun
		.sig
		.inputs
		.iter()
		.map(|i| match i {
			FnArg::Receiver(_) => unreachable!(),
			FnArg::Typed(t) => t,
		})
		.map(|t| {
			let ident = match &t.pat as &Pat {
				Pat::Ident(i) => i.ident.to_string(),
				_ => panic!("only idents supported yet"),
			};
			// TODO: Check if ty == Option<_>
			let optional = false;
			quote! {
				BuiltinParam {
					name: #ident,
					has_default: #optional,
				}
			}
		});

	let args = fun
		.sig
		.inputs
		.iter()
		.map(|i| match i {
			FnArg::Receiver(_) => unreachable!(),
			FnArg::Typed(t) => t,
		})
		.map(|t| {
			let ident = match &t.pat as &Pat {
				Pat::Ident(i) => i.ident.to_string(),
				_ => panic!("only idents supported yet"),
			};
			let ty = &t.ty;
			quote! {{
				let value = parsed.get(#ident).unwrap();

				jrsonnet_evaluator::push_description_frame(
					|| format!("argument <{}> evaluation", #ident),
					|| <#ty>::try_from(value.evaluate()?),
				)?
			}}
		});

	let attrs = &fun.attrs;
	let vis = &fun.vis;
	let name = &fun.sig.ident;
	(quote! {
		#(#attrs)*
		#vis fn #name(context: Context, _loc: &ExprLocation, args: &ArgsDesc) -> Result<Val> {
			#inner_fun
			use jrsonnet_evaluator::function::BuiltinParam;
			const PARAMS: &'static [BuiltinParam] = &[
				#(#params),*
			];
			let parsed = jrsonnet_evaluator::function::parse_builtin_call(context, &PARAMS, args, false)?;

			let result: #result = #inner_name(#(#args),*);
			let result = result?;
			result.try_into()
		}
	})
	.into()
}
