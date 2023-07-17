use proc_macro2::TokenStream;
use quote::quote;
use syn::{
	parenthesized,
	parse::{self, Parse, ParseStream},
	parse_macro_input, parse_quote,
	punctuated::Punctuated,
	spanned::Spanned,
	token::{self, Comma},
	visit_mut::VisitMut,
	Attribute, DeriveInput, Error, Expr, ExprBlock, ExprCall, ExprStruct, FnArg, GenericArgument,
	Ident, ItemFn, LitStr, Pat, Path, PathArguments, Result, ReturnType, Token, Type,
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
	let Some(args) = type_is_path(ty, "Option") else {
		return Ok(None)
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
		))
	};
	Ok(Some(ty))
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
	syn::custom_keyword!(ok);

	syn::custom_keyword!(ctx);
	syn::custom_keyword!(val);
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
			.filter(|a| a.path.is_ident("cfg"))
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
		))
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
				.map(|n| quote! {Some(std::borrow::Cow::Borrowed(#n))})
				.unwrap_or_else(|| quote! {None});
			Some(quote! {
				#(#cfg_attrs)*
				BuiltinParam {
					name: #name,
					has_default: #is_option,
				},
			})
		}
		ArgInfo::Lazy { is_option, name } => {
			let name = name
				.as_ref()
				.map(|n| quote! {Some(std::borrow::Cow::Borrowed(#n))})
				.unwrap_or_else(|| quote! {None});
			Some(quote! {
				BuiltinParam {
					name: #name,
					has_default: #is_option,
				},
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
		#item

		#[doc(hidden)]
		#[allow(non_camel_case_types)]
		#[derive(Clone, jrsonnet_gcmodule::Trace #static_derive_copy)]
		#vis struct #name {
			#(#fields)*
		}
		const _: () = {
			use ::jrsonnet_evaluator::{
				State, Val,
				function::{builtin::{Builtin, StaticBuiltin, BuiltinParam}, CallLocation, ArgsLike, parse::parse_builtin_call},
				error::Result, Context, typed::Typed,
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
			if self.is_option {
				quote! {
					if let Some(value) = self.#ident {
						out.member(#name.into()).value(<#ty as Typed>::into_untyped(value)?)?;
					}
				}
			} else {
				quote! {
					out.member(#name.into()).value(<#ty as Typed>::into_untyped(self.#ident)?)?;
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

				fn from_untyped(value: Val) -> Result<Self> {
					let obj = value.as_obj().expect("shape is correct");
					Self::parse(&obj)
				}

				fn into_untyped(value: Self) -> Result<Val> {
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

			impl TypedObj for #ident {
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

#[derive(Default)]
struct OutCollector {
	out_vals: Vec<Ident>,
	out_ctxs: Vec<Ident>,
}
impl syn::visit_mut::VisitMut for OutCollector {
	fn visit_expr_call_mut(&mut self, call: &mut ExprCall) {
		if call.args.len() != 1 {
			return;
		}
		let Expr::Path(p) = call.args.iter().next().unwrap() else {
			return;
		};
		let Some(def) = p.path.get_ident() else {
			return;
		};
		match &mut *call.func {
			Expr::Path(p) if p.path.is_ident("val") => {
				self.out_vals.push(def.clone());
				p.path = parse_quote!(core::convert::identity)
			}
			Expr::Path(p) if p.path.is_ident("ctx") => {
				self.out_ctxs.push(def.clone());
				p.path = parse_quote!(core::convert::identity)
			}
			_ => return,
		}
	}
}
enum TcoItem {
	InsertCtx(Ident, Expr),
	InsertVal(Ident, Expr),
	Apply {
		val: ExprStruct,
		out_vals: Vec<Ident>,
		out_ctxs: Vec<Ident>,
	},
}
impl Parse for TcoItem {
	fn parse(input: ParseStream) -> Result<Self> {
		if input.peek(kw::ctx) {
			input.parse::<kw::ctx>()?;
			let item;
			parenthesized!(item in input);
			let ident = item.parse()?;
			item.parse::<Token![,]>()?;
			let expr = item.parse()?;
			Ok(Self::InsertCtx(ident, expr))
		} else if input.peek(kw::val) {
			input.parse::<kw::val>()?;
			let item;
			parenthesized!(item in input);
			let ident = item.parse()?;
			item.parse::<Token![,]>()?;
			let expr = item.parse()?;
			Ok(Self::InsertVal(ident, expr))
		} else {
			let mut val: ExprStruct = input.parse()?;
			let mut collector = OutCollector::default();
			collector.visit_expr_struct_mut(&mut val);
			let OutCollector { out_vals, out_ctxs } = collector;
			Ok(Self::Apply {
				val,
				out_ctxs,
				out_vals,
			})
		}
	}
}
impl TcoItem {
	fn expand_ops_rev(self, init_rev: &mut Vec<TcoOp>, out: &mut Vec<TcoOp>) {
		use TcoOp::*;
		match self {
			TcoItem::InsertCtx(n, v) => {
				init_rev.push(DeclCtx(n.clone()));
				init_rev.push(SetCtx(n, v));
			}
			TcoItem::InsertVal(n, v) => {
				init_rev.push(DeclVal(n.clone()));
				init_rev.push(SetVal(n, v));
			}
			TcoItem::Apply {
				val,
				out_vals,
				out_ctxs,
			} => {
				for n in out_vals.iter() {
					init_rev.push(DeclVal(n.clone()));
				}
				for n in out_ctxs.iter() {
					init_rev.push(DeclCtx(n.clone()));
				}
				out.push(AddApply(val))
			}
		}
	}
}

enum TcoOp {
	DeclVal(Ident),
	DeclCtx(Ident),
	SetVal(Ident, Expr),
	SetCtx(Ident, Expr),
	AddApply(ExprStruct),
}
impl TcoOp {
	fn expand(&self, out: &mut Vec<TokenStream>) {
		out.push(match self {
			TcoOp::DeclVal(v) => quote! {
				let #v = crate::val_tag(stringify!(#v));
			},
			TcoOp::DeclCtx(v) => quote! {
				let #v = crate::ctx_tag(stringify!(#v));
			},
			TcoOp::SetVal(v, e) => quote! {{
				tcvm.vals.push(#e, #v.clone());
			}},
			TcoOp::SetCtx(v, e) => quote! {{
				tcvm.ctxs.push(#e, #v.clone());
			}},
			TcoOp::AddApply(apply) => quote! {{
				tcvm.apply.push(crate::TailCallApply::#apply, crate::apply_tag());
			}},
		})
	}
}

struct TcoInput {
	items: Vec<TcoItem>,
}
impl Parse for TcoInput {
	fn parse(input: ParseStream) -> Result<Self> {
		let mut items = Vec::new();
		loop {
			if input.is_empty() {
				break;
			}
			let i: TcoItem = input.parse()?;
			items.push(i);
			if input.peek(Token![,]) {
				input.parse::<Token![,]>()?;
				continue;
			}
			break;
		}
		if !input.is_empty() {
			return Err(Error::new(input.span(), "unknown statement after input"));
		}
		Ok(TcoInput { items })
	}
}
impl TcoInput {
	fn expand(self, cont: TokenStream) -> TokenStream {
		let mut init = Vec::new();
		let mut out = Vec::new();

		for i in self.items.into_iter().rev() {
			i.expand_ops_rev(&mut init, &mut out);
		}

		let mut vals = 0usize;
		let mut ctxs = 0usize;
		let mut applys = 0usize;
		for v in init.iter().chain(out.iter()) {
			match v {
				TcoOp::DeclVal(_) => vals += 1,
				TcoOp::DeclCtx(_) => ctxs += 1,
				TcoOp::SetVal(_, _) => {}
				TcoOp::SetCtx(_, _) => {}
				TcoOp::AddApply(_) => applys += 1,
			}
		}

		let mut run = Vec::new();
		if vals != 0 {
			run.push(quote! {tcvm.vals.reserve(#vals);});
		}
		if ctxs != 0 {
			run.push(quote! {tcvm.ctxs.reserve(#ctxs);});
		}
		if applys != 0 {
			run.push(quote! {tcvm.apply.reserve(#applys);});
		}
		for i in init.iter() {
			i.expand(&mut run);
		}
		for i in out.iter() {
			i.expand(&mut run);
		}

		quote!({
			#(#run;)*;
			#cont
		})
	}
}

#[proc_macro]
pub fn tco(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let item: TcoInput = match syn::parse(item) {
		Ok(v) => v,
		Err(e) => return e.to_compile_error().into(),
	};
	item.expand(quote! {continue;}).into()
}
#[proc_macro]
pub fn tcr(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let item: TcoInput = match syn::parse(item) {
		Ok(v) => v,
		Err(e) => return e.to_compile_error().into(),
	};
	item.expand(quote! {}).into()
}
#[proc_macro]
pub fn tcok(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let item: TcoInput = match syn::parse(item) {
		Ok(v) => v,
		Err(e) => return e.to_compile_error().into(),
	};
	item.expand(quote! {return Ok(())}).into()
}
