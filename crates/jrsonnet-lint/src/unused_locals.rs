//! Unused locals check: report local bindings that are never read.

use std::collections::HashMap;

use jrsonnet_rowan_parser::nodes::{
	Bind, CompSpec, Destruct, DestructArrayPart, DestructRest, Expr, ExprBase, FieldName, Member,
	MemberComp, Name, ObjBody, SourceFile, Stmt, Suffix,
};
use jrsonnet_rowan_parser::nodes::{ExprBase::*, Suffix::*};
use jrsonnet_rowan_parser::{parse, AstNode};
use rowan::TextRange;

use crate::checks::UNUSED_LOCALS;
use crate::config::LintConfig;

/// A single lint diagnostic (message, source range, check id).
#[derive(Clone, Debug)]
pub struct Diagnostic {
	pub check: &'static str,
	pub message: String,
	pub range: TextRange,
}

/// Run all enabled lint checks on a snippet. Returns parse errors first if any,
/// then lint diagnostics. Does not run the evaluator.
pub fn lint_snippet(code: &str, config: &LintConfig) -> (Vec<Diagnostic>, Vec<ParseError>) {
	let (source_file, parse_errors) = parse(code);
	let parse_errs: Vec<ParseError> = parse_errors
		.into_iter()
		.map(|e| ParseError {
			message: format!("{:?}", e.error),
			range: e.range,
		})
		.collect();
	if !parse_errs.is_empty() {
		return (vec![], parse_errs);
	}
	let mut diagnostics = Vec::new();
	if config.unused_locals {
		let unused = check_unused_locals(code, &source_file);
		diagnostics.extend(unused);
	}
	(diagnostics, parse_errs)
}

#[derive(Clone, Debug)]
pub struct ParseError {
	pub message: String,
	pub range: TextRange,
}

/// Scope: name -> (definition range, `was_used`).
type Scope = HashMap<String, (TextRange, bool)>;

struct ScopeEntry {
	bindings: Scope,
	/// If false, unused bindings in this scope are not reported (e.g. function parameters).
	report_unused: bool,
}

struct UnusedLocalsVisitor {
	scopes: Vec<ScopeEntry>,
	diagnostics: Vec<Diagnostic>,
}

impl UnusedLocalsVisitor {
	fn push_scope(&mut self, bindings: Vec<(String, TextRange)>) {
		self.push_scope_with_reporting(bindings, true);
	}

	/// Push a scope whose bindings are tracked for `mark_used` but never reported as unused.
	fn push_scope_silent(&mut self, bindings: Vec<(String, TextRange)>) {
		self.push_scope_with_reporting(bindings, false);
	}

	fn push_scope_with_reporting(
		&mut self,
		bindings: Vec<(String, TextRange)>,
		report_unused: bool,
	) {
		let mut scope = Scope::new();
		for (name, range) in bindings {
			scope.insert(name, (range, false));
		}
		self.scopes.push(ScopeEntry {
			bindings: scope,
			report_unused,
		});
	}

	fn mark_used(&mut self, name: &str) {
		for entry in self.scopes.iter_mut().rev() {
			if let Some((_, used)) = entry.bindings.get_mut(name) {
				*used = true;
				break;
			}
		}
	}

	fn pop_scope_and_report(&mut self) {
		if let Some(entry) = self.scopes.pop() {
			if entry.report_unused {
				for (name, (range, used)) in entry.bindings {
					if !used {
						self.diagnostics.push(Diagnostic {
							check: UNUSED_LOCALS,
							message: format!("unused local `{name}`"),
							range,
						});
					}
				}
			}
		}
	}

	fn name_text_and_range(name: &Name) -> Option<(String, TextRange)> {
		name.ident_lit()
			.map(|t| (t.text().to_string(), t.text_range()))
	}

	fn collect_bind_names(bind: &Bind) -> Vec<(String, TextRange)> {
		match bind {
			Bind::BindFunction(b) => {
				if let Some(name) = b.name() {
					if let Some(pair) = Self::name_text_and_range(&name) {
						return vec![pair];
					}
				}
			}
			Bind::BindDestruct(b) => {
				if let Some(d) = b.into() {
					return Self::collect_destruct_names(&d);
				}
			}
		}
		vec![]
	}

	fn collect_destruct_names(d: &Destruct) -> Vec<(String, TextRange)> {
		match d {
			Destruct::DestructFull(n) => {
				if let Some(name) = n.name() {
					if let Some(pair) = Self::name_text_and_range(&name) {
						return vec![pair];
					}
				}
			}
			Destruct::DestructSkip(_) => {}
			Destruct::DestructArray(a) => {
				let mut out = Vec::new();
				for part in a.destruct_array_parts() {
					match part {
						DestructArrayPart::DestructArrayElement(el) => {
							if let Some(d) = el.destruct() {
								out.extend(Self::collect_destruct_names(&d));
							}
						}
						DestructArrayPart::DestructRest(rest) => {
							if let Some(name) = DestructRest::into(&rest) {
								if let Some(pair) = Self::name_text_and_range(&name) {
									out.push(pair);
								}
							}
						}
					}
				}
				return out;
			}
			Destruct::DestructObject(o) => {
				let mut out = Vec::new();
				for field in o.destruct_object_fields() {
					if let Some(d) = field.destruct() {
						out.extend(Self::collect_destruct_names(&d));
					} else if let Some(name) = field.field() {
						if let Some(pair) = Self::name_text_and_range(&name) {
							out.push(pair);
						}
					}
				}
				if let Some(rest) = o.destruct_rest() {
					if let Some(name) = DestructRest::into(&rest) {
						if let Some(pair) = Self::name_text_and_range(&name) {
							out.push(pair);
						}
					}
				}
				return out;
			}
		}
		vec![]
	}

	/// Visit the RHS of a bind (for `StmtLocal` or `ObjLocal`). Handles `BindFunction` param scope when value is body-only.
	fn visit_bind_value(visitor: &mut Self, bind: &Bind) {
		match bind {
			Bind::BindFunction(b) => {
				if let Some(value) = b.value() {
					let is_full_function = value
						.expr_base()
						.is_some_and(|base| matches!(base, ExprFunction(_)));
					let param_bindings = if is_full_function {
						vec![]
					} else {
						b.params()
							.map(|params| {
								params
									.params()
									.flat_map(|p| {
										p.destruct().map_or_else(Vec::new, |d| {
											Self::collect_destruct_names(&d)
										})
									})
									.collect::<Vec<_>>()
							})
							.unwrap_or_default()
					};
					let has_param_scope = !param_bindings.is_empty();
					if has_param_scope {
						visitor.push_scope_silent(param_bindings);
					}
					visitor.visit_expr(&value);
					if has_param_scope {
						visitor.pop_scope_and_report();
					}
				}
			}
			Bind::BindDestruct(b) => {
				if let Some(value) = b.value() {
					visitor.visit_expr(&value);
				}
			}
		}
	}

	fn visit_expr(&mut self, expr: &Expr) {
		// Push scopes for all local statements (same order as scope visibility)
		let mut push_count = 0usize;
		for stmt in expr.stmts() {
			if let Stmt::StmtLocal(s) = stmt {
				let mut bindings = Vec::new();
				for bind in s.binds() {
					bindings.extend(Self::collect_bind_names(&bind));
				}
				if !bindings.is_empty() {
					self.push_scope(bindings);
					push_count += 1;
				}
			}
		}
		// Visit RHS of each local bind. BindFunction's value may be body-only; if so, push param scope first.
		for stmt in expr.stmts() {
			if let Stmt::StmtLocal(s) = stmt {
				for bind in s.binds() {
					Self::visit_bind_value(self, &bind);
				}
			}
		}
		// Visit body and suffixes
		if let Some(base) = expr.expr_base() {
			self.visit_expr_base(&base);
		}
		for suffix in expr.suffixs() {
			self.visit_suffix(&suffix);
		}
		// Pop and report (one per scope we pushed)
		for _ in 0..push_count {
			self.pop_scope_and_report();
		}
	}

	#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
	fn visit_expr_base(&mut self, base: &ExprBase) {
		match base {
			ExprBinary(e) => {
				// lhs() and rhs() both use support::child; rowan wraps only the base node in the lhs
				// EXPR, leaving its suffixes as direct children of EXPR_BINARY. Visit Suffix children too.
				for child in e.syntax().children() {
					if let Some(expr) = Expr::cast(child.clone()) {
						self.visit_expr(&expr);
					} else if let Some(suffix) = Suffix::cast(child) {
						self.visit_suffix(&suffix);
					}
				}
			}
			ExprUnary(e) => {
				// Visit all Expr children (rhs; unary op is a token) so we don't miss uses.
				for child in e.syntax().children() {
					if let Some(expr) = Expr::cast(child) {
						self.visit_expr(&expr);
					}
				}
			}
			ExprObjExtend(e) => {
				// Same as ExprBinary: lhs suffixes may be direct children of EXPR_OBJ_EXTEND.
				for child in e.syntax().children() {
					if let Some(expr) = Expr::cast(child.clone()) {
						self.visit_expr(&expr);
					} else if let Some(suffix) = Suffix::cast(child) {
						self.visit_suffix(&suffix);
					}
				}
			}
			ExprParened(e) => {
				if let Some(expr) = e.expr() {
					self.visit_expr(&expr);
				}
			}
			ExprString(_) | ExprNumber(_) | ExprLiteral(_) | ExprImport(_) => {}
			ExprArray(e) => {
				for expr in e.exprs() {
					self.visit_expr(&expr);
				}
			}
			ExprObject(e) => {
				if let Some(body) = e.obj_body() {
					self.visit_obj_body(&body);
				}
			}
			ExprArrayComp(e) => {
				// Push for-scopes first so the output expression can reference loop vars
				let mut for_push_count = 0usize;
				for spec in e.comp_specs() {
					if let CompSpec::ForSpec(f) = spec {
						let bindings = f
							.bind()
							.map_or_else(Vec::new, |d| Self::collect_destruct_names(&d));
						if !bindings.is_empty() {
							self.push_scope(bindings);
							for_push_count += 1;
						}
					}
				}
				if let Some(expr) = e.expr() {
					self.visit_expr(&expr);
				}
				for spec in e.comp_specs() {
					self.visit_comp_spec_exprs_only(&spec);
				}
				for _ in 0..for_push_count {
					self.pop_scope_and_report();
				}
			}
			ExprVar(e) => {
				if let Some(name) = e.name() {
					if let Some((text, _)) = Self::name_text_and_range(&name) {
						self.mark_used(&text);
					}
				}
			}
			ExprIfThenElse(e) => {
				if let Some(c) = e.cond() {
					self.visit_expr(&c);
				}
				if let Some(t) = e.then() {
					if let Some(expr) = t.expr() {
						self.visit_expr(&expr);
					}
				}
				if let Some(else_) = e.else_() {
					if let Some(expr) = else_.expr() {
						self.visit_expr(&expr);
					}
				}
			}
			ExprFunction(e) => {
				let param_bindings = e
					.params_desc()
					.map(|params| {
						params
							.params()
							.flat_map(|p| {
								p.destruct()
									.map_or_else(Vec::new, |d| Self::collect_destruct_names(&d))
							})
							.collect::<Vec<_>>()
					})
					.unwrap_or_default();
				let has_param_bindings = !param_bindings.is_empty();
				if has_param_bindings {
					self.push_scope_silent(param_bindings);
				}
				if let Some(expr) = e.expr() {
					self.visit_expr(&expr);
				}
				if has_param_bindings {
					self.pop_scope_and_report();
				}
			}
			ExprError(e) => {
				if let Some(expr) = e.expr() {
					self.visit_expr(&expr);
				}
			}
		}
	}

	fn visit_suffix(&mut self, suffix: &jrsonnet_rowan_parser::nodes::Suffix) {
		match suffix {
			SuffixIndex(_) => {
				// Index is a Name (field), not an Expr - nothing to visit
			}
			SuffixIndexExpr(s) => {
				if let Some(expr) = s.index() {
					self.visit_expr(&expr);
				}
			}
			SuffixSlice(s) => {
				if let Some(slice) = s.slice_desc() {
					if let Some(from) = slice.from() {
						self.visit_expr(&from);
					}
					if let Some(end) = slice.end().and_then(|e| e.expr()) {
						self.visit_expr(&end);
					}
					if let Some(step) = slice.step().and_then(|s| s.expr()) {
						self.visit_expr(&step);
					}
				}
			}
			SuffixApply(s) => {
				if let Some(args) = s.args_desc() {
					for arg in args.args() {
						if let Some(expr) = arg.expr() {
							self.visit_expr(&expr);
						}
					}
				}
			}
		}
	}

	#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
	fn visit_obj_body(&mut self, body: &ObjBody) {
		match body {
			ObjBody::ObjBodyMemberList(list) => {
				let mut push_count = 0usize;
				for member in list.members() {
					if let Member::MemberBindStmt(m) = &member {
						if let Some(obj_local) = m.obj_local() {
							if let Some(bind) = obj_local.bind() {
								let bindings = Self::collect_bind_names(&bind);
								if !bindings.is_empty() {
									self.push_scope(bindings);
									push_count += 1;
								}
							}
						}
					}
				}
				// Visit RHS of each object-local bind (so locals used in other locals' values are marked).
				for member in list.members() {
					if let Member::MemberBindStmt(m) = &member {
						if let Some(obj_local) = m.obj_local() {
							if let Some(bind) = obj_local.bind() {
								Self::visit_bind_value(self, &bind);
							}
						}
					}
				}
				for member in list.members() {
					match &member {
						Member::MemberBindStmt(_) => {}
						Member::MemberAssertStmt(a) => {
							if let Some(assertion) = a.assertion() {
								if let Some(c) = assertion.condition() {
									self.visit_expr(&c);
								}
								if let Some(m) = assertion.message() {
									self.visit_expr(&m);
								}
							}
						}
						Member::MemberFieldNormal(f) => {
							if let Some(field_name) = f.field_name() {
								self.visit_field_name(&field_name);
							}
							if let Some(expr) = f.expr() {
								self.visit_expr(&expr);
							}
						}
						Member::MemberFieldMethod(f) => {
							if let Some(field_name) = f.field_name() {
								self.visit_field_name(&field_name);
							}
							// Visit all Expr children (param defaults + body); expr() returns first only.
							for child in f.syntax().children() {
								if let Some(expr) = Expr::cast(child) {
									self.visit_expr(&expr);
								}
							}
						}
					}
				}
				for _ in 0..push_count {
					self.pop_scope_and_report();
				}
			}
			ObjBody::ObjBodyComp(comp) => {
				let mut push_count = 0usize;
				for member in comp.member_comps() {
					if let MemberComp::MemberBindStmt(m) = &member {
						if let Some(obj_local) = m.obj_local() {
							if let Some(bind) = obj_local.bind() {
								let bindings = Self::collect_bind_names(&bind);
								if !bindings.is_empty() {
									self.push_scope(bindings);
									push_count += 1;
								}
							}
						}
					}
				}
				for member in comp.member_comps() {
					if let MemberComp::MemberBindStmt(m) = &member {
						if let Some(obj_local) = m.obj_local() {
							if let Some(bind) = obj_local.bind() {
								Self::visit_bind_value(self, &bind);
							}
						}
					}
				}
				// Push for-scopes before visiting member output so loop variables are in scope when we mark uses.
				let mut for_push_count = 0usize;
				for spec in comp.comp_specs() {
					if let CompSpec::ForSpec(f) = spec {
						let bindings = f
							.bind()
							.map_or_else(Vec::new, |d| Self::collect_destruct_names(&d));
						if !bindings.is_empty() {
							self.push_scope(bindings);
							for_push_count += 1;
						}
					}
				}
				for member in comp.member_comps() {
					match &member {
						MemberComp::MemberBindStmt(_) => {}
						MemberComp::MemberFieldNormal(f) => {
							if let Some(field_name) = f.field_name() {
								self.visit_field_name(&field_name);
							}
							if let Some(expr) = f.expr() {
								self.visit_expr(&expr);
							}
						}
						MemberComp::MemberFieldMethod(f) => {
							if let Some(field_name) = f.field_name() {
								self.visit_field_name(&field_name);
							}
							for child in f.syntax().children() {
								if let Some(expr) = Expr::cast(child) {
									self.visit_expr(&expr);
								}
							}
						}
					}
				}
				for spec in comp.comp_specs() {
					self.visit_comp_spec_exprs_only(&spec);
				}
				for _ in 0..for_push_count {
					self.pop_scope_and_report();
				}
				for _ in 0..push_count {
					self.pop_scope_and_report();
				}
			}
		}
	}

	fn visit_field_name(&mut self, field_name: &FieldName) {
		match field_name {
			FieldName::FieldNameFixed(_) => {}
			FieldName::FieldNameDynamic(d) => {
				if let Some(expr) = d.expr() {
					self.visit_expr(&expr);
				}
			}
		}
	}

	/// Visit only the expressions inside a comp spec (for array comp: we already pushed for-scopes).
	fn visit_comp_spec_exprs_only(&mut self, spec: &CompSpec) {
		match spec {
			CompSpec::ForSpec(f) => {
				if let Some(expr) = f.expr() {
					self.visit_expr(&expr);
				}
			}
			CompSpec::IfSpec(i) => {
				if let Some(expr) = i.expr() {
					self.visit_expr(&expr);
				}
			}
		}
	}
}

fn check_unused_locals(_code: &str, source_file: &SourceFile) -> Vec<Diagnostic> {
	let mut visitor = UnusedLocalsVisitor {
		scopes: Vec::new(),
		diagnostics: Vec::new(),
	};
	if let Some(expr) = source_file.expr() {
		visitor.visit_expr(&expr);
	}
	visitor.diagnostics
}
