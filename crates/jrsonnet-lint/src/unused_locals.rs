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

/// An auto-fix for a diagnostic: the text range to delete.
#[derive(Clone, Debug)]
pub struct Fix {
	/// Source range to delete to apply the fix.
	pub range: TextRange,
}

/// A single lint diagnostic (message, source range, check id).
#[derive(Clone, Debug)]
pub struct Diagnostic {
	pub check: &'static str,
	pub message: String,
	pub range: TextRange,
	/// Auto-fix, if available.
	pub fix: Option<Fix>,
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

/// Apply auto-fixes from diagnostics to the source code.
/// Returns the modified code. Fixes are applied in reverse order to preserve offsets.
/// Each fix range is extended to also consume surrounding whitespace and commas.
pub fn apply_fixes(code: &str, diagnostics: &[Diagnostic]) -> String {
	let mut ranges: Vec<(usize, usize)> = diagnostics
		.iter()
		.filter_map(|d| {
			d.fix.as_ref().map(|f| {
				let start: usize = f.range.start().into();
				let end: usize = f.range.end().into();
				extended_fix_range(code, start, end)
			})
		})
		.collect();

	if ranges.is_empty() {
		return code.to_string();
	}

	// Sort by start in ascending order, then filter out overlapping ranges
	ranges.sort_by_key(|&(start, _)| start);
	let mut filtered: Vec<(usize, usize)> = Vec::new();
	let mut last_end = 0usize;
	for (start, end) in ranges {
		if start >= last_end {
			filtered.push((start, end));
			last_end = end;
		}
		// else: overlapping range, skip
	}

	// Apply from end to start to preserve earlier offsets
	let mut result = code.to_string();
	for &(start, end) in filtered.iter().rev() {
		result.replace_range(start..end, "");
	}
	result
}

/// Extends a raw fix range (exact syntax node range) to also consume:
/// - Leading whitespace (spaces/tabs only, stopping at newlines)
/// - Trailing comma (if present, skipping whitespace between node end and comma)
/// - Trailing whitespace and one newline after the comma (or directly after the node)
fn extended_fix_range(code: &str, start: usize, end: usize) -> (usize, usize) {
	let bytes = code.as_bytes();

	// Extend backward: consume leading spaces/tabs (not newlines)
	let mut new_start = start;
	while new_start > 0 && (bytes[new_start - 1] == b' ' || bytes[new_start - 1] == b'\t') {
		new_start -= 1;
	}

	// Extend forward: skip whitespace to find optional comma, then consume whitespace + newline
	let mut new_end = end;
	// Skip whitespace before possible comma
	let mut check = new_end;
	while check < bytes.len() && (bytes[check] == b' ' || bytes[check] == b'\t') {
		check += 1;
	}
	if check < bytes.len() && bytes[check] == b',' {
		// Consume the comma
		new_end = check + 1;
		// Skip whitespace after comma
		while new_end < bytes.len() && (bytes[new_end] == b' ' || bytes[new_end] == b'\t') {
			new_end += 1;
		}
	} else {
		// No comma; consume the whitespace we scanned over
		new_end = check;
	}

	// Consume trailing newline
	if new_end < bytes.len() && bytes[new_end] == b'\n' {
		new_end += 1;
	}

	(new_start, new_end)
}

#[derive(Clone, Debug)]
pub struct ParseError {
	pub message: String,
	pub range: TextRange,
}

/// Scope binding: tracks the identifier range, usage, and optional fix range.
struct ScopeBinding {
	/// Range of the identifier token (used for the diagnostic location).
	range: TextRange,
	used: bool,
	/// Range to delete to fix this unused local (None if auto-fix is not supported).
	fix_range: Option<TextRange>,
}

/// Scope: name -> binding info.
type Scope = HashMap<String, ScopeBinding>;

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
	fn push_scope(&mut self, bindings: Vec<(String, TextRange, Option<TextRange>)>) {
		self.push_scope_with_reporting(bindings, true);
	}

	/// Push a scope whose bindings are tracked for `mark_used` but never reported as unused.
	fn push_scope_silent(&mut self, bindings: Vec<(String, TextRange)>) {
		let bindings = bindings.into_iter().map(|(n, r)| (n, r, None)).collect();
		self.push_scope_with_reporting(bindings, false);
	}

	fn push_scope_with_reporting(
		&mut self,
		bindings: Vec<(String, TextRange, Option<TextRange>)>,
		report_unused: bool,
	) {
		let mut scope = Scope::new();
		for (name, range, fix_range) in bindings {
			scope.insert(
				name,
				ScopeBinding {
					range,
					used: false,
					fix_range,
				},
			);
		}
		self.scopes.push(ScopeEntry {
			bindings: scope,
			report_unused,
		});
	}

	fn mark_used(&mut self, name: &str) {
		for entry in self.scopes.iter_mut().rev() {
			if let Some(binding) = entry.bindings.get_mut(name) {
				binding.used = true;
				break;
			}
		}
	}

	fn pop_scope_and_report(&mut self) {
		if let Some(entry) = self.scopes.pop() {
			if entry.report_unused {
				for (name, binding) in entry.bindings {
					if !binding.used {
						self.diagnostics.push(Diagnostic {
							check: UNUSED_LOCALS,
							message: format!("unused local `{name}`"),
							range: binding.range,
							fix: binding.fix_range.map(|r| Fix { range: r }),
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

	/// Visit the RHS of a bind (for StmtLocal or ObjLocal). Handles BindFunction param scope when value is body-only.
	fn visit_bind_value(visitor: &mut Self, bind: &Bind) {
		match bind {
			Bind::BindFunction(b) => {
				if let Some(value) = b.value() {
					let is_full_function = value
						.expr_base()
						.map_or(false, |base| matches!(base, ExprFunction(_)));
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
				// Fix range: remove whole statement only when there's a single bind.
				// Multi-bind statements (local x = 1, y = 2;) are not auto-fixed.
				let bind_count = s.binds().count();
				let fix_range = if bind_count == 1 {
					Some(s.syntax().text_range())
				} else {
					None
				};
				let mut bindings: Vec<(String, TextRange, Option<TextRange>)> = Vec::new();
				for bind in s.binds() {
					for (name, range) in Self::collect_bind_names(&bind) {
						bindings.push((name, range, fix_range));
					}
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
				// ExprUnary's operand is parsed WITHOUT an EXPR wrapper, so direct children are
				// ExprBase nodes (e.g. EXPR_VAR for `!this`) and Suffix nodes (e.g. `.x` for
				// `!this.x`). Neither is an EXPR, so we must cast to ExprBase and Suffix explicitly.
				for child in e.syntax().children() {
					if let Some(base) = ExprBase::cast(child.clone()) {
						self.visit_expr_base(&base);
					} else if let Some(suffix) = Suffix::cast(child) {
						self.visit_suffix(&suffix);
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
			ExprString(_) | ExprNumber(_) | ExprLiteral(_) => {}
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
						let bindings: Vec<(String, TextRange, Option<TextRange>)> = f
							.bind()
							.map_or_else(Vec::new, |d| Self::collect_destruct_names(&d))
							.into_iter()
							.map(|(n, r)| (n, r, None))
							.collect();
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
			ExprImport(_) => {}
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

	fn visit_obj_body(&mut self, body: &ObjBody) {
		match body {
			ObjBody::ObjBodyMemberList(list) => {
				let mut push_count = 0usize;
				for member in list.members() {
					if let Member::MemberBindStmt(m) = &member {
						// Fix range: remove the whole MemberBindStmt (the comma is handled by the
						// fixer's extended_fix_range, which looks for a trailing comma in the text).
						let fix_range = Some(m.syntax().text_range());
						if let Some(obj_local) = m.obj_local() {
							if let Some(bind) = obj_local.bind() {
								let bindings: Vec<(String, TextRange, Option<TextRange>)> =
									Self::collect_bind_names(&bind)
										.into_iter()
										.map(|(n, r)| (n, r, fix_range))
										.collect();
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
						let fix_range = Some(m.syntax().text_range());
						if let Some(obj_local) = m.obj_local() {
							if let Some(bind) = obj_local.bind() {
								let bindings: Vec<(String, TextRange, Option<TextRange>)> =
									Self::collect_bind_names(&bind)
										.into_iter()
										.map(|(n, r)| (n, r, fix_range))
										.collect();
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
						let bindings: Vec<(String, TextRange, Option<TextRange>)> = f
							.bind()
							.map_or_else(Vec::new, |d| Self::collect_destruct_names(&d))
							.into_iter()
							.map(|(n, r)| (n, r, None))
							.collect();
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
