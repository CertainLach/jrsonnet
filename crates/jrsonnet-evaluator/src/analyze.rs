// {
//		a: $, // Is an equivalent of super, making field `a` object-dependent, we can't cache it per-object
//		b: {
//			a: $, // Field `a` is not object-dependent, because object `b` itself is object-dependent, but every field in it aren't bound to the top object,
//					// This is the fact that `b` itself will be created once per top-level object.
//		},
// }
//
// Same thing with locals. Should $ be handled as local instead of this magic?

use drop_bomb::DropBomb;
use hi_doc::{Formatting, SnippetBuilder, Text};
use jrsonnet_interner::IStr;
use jrsonnet_parser::{
	AssertStmt, BindSpec, Destruct, Expr, LiteralType, LocExpr, ObjBody, Param, SliceDesc, Source,
	Span, Spanned,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{ContextBuilder, ContextInitializer, State};

#[derive(Debug, Clone, Copy)]
#[must_use]
struct AnalysisResult {
	// Highest object, on which identity the value is dependent. u32::MAX = not dependent at all
	object_dependent_depth: u32,
	// Highest local, on which this value depends. u32::MAX = not dependent at all
	local_dependent_depth: u32,
}
impl AnalysisResult {
	fn depend_on_object(&mut self, object: u32) -> bool {
		if object < self.object_dependent_depth {
			self.object_dependent_depth = object;
			true
		} else {
			false
		}
	}
	fn depend_on_local(&mut self, local: u32) -> bool {
		if local < self.local_dependent_depth {
			self.local_dependent_depth = local;
			true
		} else {
			false
		}
	}
	fn taint_by(&mut self, result: &AnalysisResult) -> bool {
		self.depend_on_object(result.object_dependent_depth)
	}
}
struct LocalDefinition {
	name: Spanned<IStr>,
	// At which tree depth this local was defined
	defined_at_depth: u32,
	/// Min depth, at which this local was used. `u32::MAX` = not used at all.
	/// This check won't catch unused argument closures, i.e:
	/// ```jsonnet
	/// local
	///     a = b,
	///     b = a,
	/// ; 2 + 2
	///
	/// ```
	/// Both `a` and `b` here are "used", but the whole closure was not used here.
	used_at_depth: u32,
	/// Used as part of closure
	/// TODO: Store indirect analysis separately
	used_by_current_frame: bool,
	// Analysys result for value of this local
	analysis: AnalysisResult,
	// For sanity checking, locals are initialized in batchs, use first_uninitialized_local
	analyzed: bool,
	// During walk over uninitialized vars, we can't refer to analysis results of other locals,
	// but we need to. To make that work, for each variable in variable frame we capture its closure,
	// by looking at referenced variables.
	referened: bool,
}
impl LocalDefinition {
	fn use_at(&mut self, depth: u32) {
		if depth == self.defined_at_depth {
			// TODO: Don't ignore self-uses, also see comment about indirect analysis
			self.used_by_current_frame = true;
			return;
		}
		self.used_at_depth = self.used_at_depth.min(depth);
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LocalId(usize);
impl LocalId {
	fn defined_before(self, other: Self) -> bool {
		self.0 < other.0
	}
	fn local_before(self) -> LocalId {
		LocalId(self.0 - 1)
	}
}

struct AnalysisStack {
	local_defs: Vec<LocalDefinition>,
	// Shadowing isn't used in jsonnet much, 2 because SmallVec allows to store 2 ptr-sized without overhead.
	// TODO: Add test for this assumption (sizeof(SmallVec<[usize; 1]>) == sizeof(SmallVec<[usize; 2]>))
	local_by_name: FxHashMap<jrsonnet_interner::IStr, smallvec::SmallVec<[LocalId; 2]>>,
	// Locals in jsonnet are mutually-recursive, some locals aren't analyzable before dependencies are analyzed,
	// and dependencies may depend on local itself. To fix this issue - unanalyzed locals are not analyzed the same way as normal.
	first_uninitialized_local: LocalId,

	// How deep we are recursed into expression tree.
	depth: u32,
	// Last depth, at which object has appeared. u32::MAX = not appeared at all
	last_object_depth: u32,
	// First depth, at which object has appeared. u32::MAX = not appeared at all
	// $ refers to this object.
	first_object_depth: u32,

	result: hi_doc::SnippetBuilder,
	errored: bool,
}
impl AnalysisStack {
	fn new(src: &str) -> Self {
		Self {
			local_defs: vec![],
			local_by_name: FxHashMap::default(),
			first_uninitialized_local: LocalId(0),
			depth: 0,
			last_object_depth: u32::MAX,
			first_object_depth: u32::MAX,
			result: SnippetBuilder::new(src),
			errored: false,
		}
	}
	fn first_object(&self) -> u32 {
		assert!(
			self.first_object_depth != u32::MAX,
			"$ used outside of object"
		);
		self.first_object_depth
	}
	fn last_object(&self) -> u32 {
		assert!(
			self.last_object_depth != u32::MAX,
			"this/super used outside of object"
		);
		self.last_object_depth
	}
	fn local(&mut self, name: &IStr, span: Span) -> Option<(&mut LocalDefinition, bool)> {
		let Some(local_id) = self.local_by_name.get(name) else {
			self.result
				.error(Text::single(
					format!("local is not defined: {name}").chars(),
					Formatting::default(),
				))
				.range(span.1 as usize..=(span.2 - 1) as usize)
				.build();
			self.errored = true;
			return None;
		};
		let local_id = *local_id
			.last()
			.expect("local not defined (maybe defined deeper)");
		Some((
			&mut self.local_defs[local_id.0],
			local_id.defined_before(self.first_uninitialized_local),
		))
	}
	fn use_local(&mut self, name: &IStr, span: Span, taint: &mut AnalysisResult) {
		let depth = self.depth;
		let errored = self.errored;
		let Some((local, initialized)) = self.local(name, span) else {
			return;
		};
		local.use_at(depth);
		if dbg!(initialized) {
			// It is ok for errored output to do that
			if !errored {
				assert!(
					local.analyzed,
					"sanity: initialized == true => analyzed == true, two markers should match for {name}"
				);
			}
			taint.taint_by(&local.analysis);
		} else {
			println!("local referenced!");
			local.referened = true;
		}
		taint.depend_on_local(local.defined_at_depth);
	}
	// TODO: Is'nt exacly correct that it is in PendingUsage state, there should be yet another one, to prevent from
	// using it before closures are finished processing... Or maybe it should be done at the same time as
	// `finish_local_initializations`?
	fn local_uses_local(&mut self, local: LocalId, uses: &LocalId) -> bool {
		dbg!(local, uses);
		let mut changed = false;
		let user_used_at_depth = self.local_defs[local.0].used_at_depth;

		let uses = &mut self.local_defs[uses.0];
		let defined_at_depth = uses.defined_at_depth;
		let analysis = uses.analysis;

		if dbg!(user_used_at_depth) < dbg!(uses.used_at_depth) {
			changed = true;
			uses.used_at_depth = user_used_at_depth;
		}

		let user = &mut self.local_defs[local.0];
		assert_eq!(
			user.defined_at_depth, defined_at_depth,
			"local_uses_local is only intended to be used at the sibling locals"
		);
		// TODO: Store indirect analysis in different field? Currently all indirect analysis are propagated as
		// analysis field

		changed |= user.analysis.taint_by(&analysis);
		changed |= user.analysis.depend_on_local(defined_at_depth);
		changed
	}
	fn ensure_no_unitialized_locals(&self) {
		assert_eq!(
			self.first_uninitialized_local,
			self.next_local_id(),
			"unexpected uninitialized locals"
		);
	}
	fn next_local_id(&self) -> LocalId {
		LocalId(self.local_defs.len())
	}
	fn start_local_deconstructions(&self) -> PendingDeconstructions {
		self.ensure_no_unitialized_locals();
		PendingDeconstructions {
			first_in_frame: self.next_local_id(),
			bomb: DropBomb::new(
				"after locals are defined, you need to pass DupeCheckMarker to new_locals_list",
			),
		}
	}
	/// # Panics
	///
	/// If locale is already defined
	fn define_external_local(&mut self, name: IStr) {
		self.ensure_no_unitialized_locals();
		let next_local_id = self.next_local_id();
		let found = self.local_by_name.entry(name.clone()).or_default();
		// Empty by-names are preserved
		if let Some(id) = found.last() {
			panic!("external locals should not be redefined");
		};
		found.push(next_local_id);
		self.local_defs.push(LocalDefinition {
			name: Spanned(
				name,
				Span(Source::new_virtual("UNNAMED".into(), "".into()), 0, 0),
			),
			defined_at_depth: 0,
			analysis: AnalysisResult::default(),
			used_at_depth: 0,
			analyzed: false,
			referened: false,
			used_by_current_frame: false,
		});
		// External local is always initialized
		self.first_uninitialized_local = self.next_local_id();
		eprintln!("First uninit = {:?}", self.first_uninitialized_local);
	}
	#[must_use]
	fn define_local(&mut self, dupe: &PendingDeconstructions, name: Spanned<IStr>) -> Option<()> {
		let next_local_id = self.next_local_id();
		let found = self.local_by_name.entry(name.0.clone()).or_default();
		// Empty by-names are preserved
		if let Some(id) = found.last() {
			if !id.defined_before(dupe.first_in_frame) {
				self.result
					.error(Text::single(
						format!("variable redeclared: {}", name.0).chars(),
						Formatting::default(),
					))
					.range(name.1.range())
					.build();
				return None;
			}
		};
		found.push(next_local_id);
		self.local_defs.push(LocalDefinition {
			name,
			defined_at_depth: self.depth,
			analysis: AnalysisResult::default(),
			used_at_depth: u32::MAX,
			analyzed: false,
			referened: false,
			used_by_current_frame: false,
		});
		Some(())
	}
	fn finish_local_deconstructions(
		&mut self,
		PendingDeconstructions {
			first_in_frame,
			mut bomb,
		}: PendingDeconstructions,
	) -> PendingInitialization {
		bomb.defuse();
		for ele in &self.local_defs[first_in_frame.0..] {
			assert_eq!(
				ele.defined_at_depth, self.depth,
				"sanity: depth was changed during deconstructions"
			);
			assert_eq!(
				ele.used_at_depth,
				u32::MAX,
				"should not use locals before deconstructions finished"
			);
		}
		let first_after_frame = self.next_local_id();
		assert_ne!(
			first_in_frame, first_after_frame,
			"no locals were defined during deconstruction"
		);
		PendingInitialization {
			first_in_frame,
			first_after_frame,
			bomb: DropBomb::new(
				"after you done with initialization - pass to finish_local_initializations",
			),
		}
	}
	fn initialize_local(
		&mut self,
		pending: &PendingInitialization,
		id: LocalId,
		analysis: AnalysisResult,
		taint: &mut AnalysisResult,
	) {
		let local = &mut self.local_defs[id.0];
		assert!(!local.analyzed, "sanity: already initialized");
		pending.ensure_pending(id);
		taint.taint_by(&analysis);
		local.analysis = analysis;

		local.analyzed = true;
	}
	fn finish_local_initializations(
		&mut self,
		PendingInitialization {
			first_in_frame,
			first_after_frame,
			mut bomb,
		}: PendingInitialization,
	) -> PendingUsage {
		bomb.defuse();
		assert_eq!(
			first_after_frame,
			self.next_local_id(),
			"during local initialization, there were unfinished locals"
		);
		self.first_uninitialized_local = self.next_local_id();
		eprintln!("First uninit = {:?}", self.first_uninitialized_local);

		for ele in &self.local_defs[first_in_frame.0..first_after_frame.0] {
			assert!(ele.analyzed, "sanity: not initialized");
			assert!(
				!ele.referened,
				"sanity: referenced field was not resed, local closure isn't fully captured"
			);
		}

		PendingUsage {
			first_in_frame,
			first_after_frame,
			bomb: DropBomb::new("after you done with usage - pass to finish_local_usages"),
		}
	}
	fn finish_local_usages(
		&mut self,
		closures: &Closures,
		PendingUsage {
			first_in_frame,
			first_after_frame,
			mut bomb,
		}: PendingUsage,
	) {
		bomb.defuse();
		self.ensure_no_unitialized_locals();
		assert_eq!(
			first_after_frame,
			self.next_local_id(),
			"unfinished locals stack found"
		);

		{
			// FIXME: It should only handle local uses (used_at), data about local data themselves should be processed
			// before handle_inside
			let mut changed = true;
			while changed {
				changed = false;
				closures.process(|closure| {
					for uses in closure.references_locals {
						changed |= self.local_uses_local(closure.local, uses);
					}
				});
			}
		}

		let mut expected_idx = first_after_frame;
		for ele in self.local_defs.drain(first_in_frame.0..).rev() {
			expected_idx = expected_idx.local_before();
			let id = self
				.local_by_name
				.get_mut(&ele.name.0)
				.expect("exists")
				.pop()
				.expect("exists");
			assert_eq!(id, expected_idx, "sanity: by name map correctness");
			assert!(
				ele.used_at_depth >= self.depth,
				"sanity: lower expression can't reach upper"
			);
			assert_eq!(ele.defined_at_depth, self.depth, "sanity: depth was not decreased/decreased too much after finishing working with locals");
			assert!(ele.analyzed);
			if ele.used_at_depth == u32::MAX {
				self.result
					.warning(Text::single(
						format!("local was not used: {}", ele.name.0).chars(),
						Formatting::default(),
					))
					.range(ele.name.1.range())
					.build();
			}
			if dbg!(ele.used_at_depth) == dbg!(ele.defined_at_depth) {
				self.result
					.warning(Text::single(
						format!(
							"local was not used (only in closure, which wasn't referenced): {0}",
							ele.name.0
						)
						.chars(),
						Formatting::default(),
					))
					.range(ele.name.1.range())
					.build();
			}
			if ele.analysis.local_dependent_depth < ele.defined_at_depth
				|| ele.analysis.object_dependent_depth < ele.defined_at_depth
			{
				self.result
					.warning(Text::single(
						format!(
							"local is only using items from parent scope, move it higher: {}",
							ele.name.0
						)
						.chars(),
						Formatting::default(),
					))
					.range(ele.name.1.range())
					.build();
			}
		}
		self.first_uninitialized_local = first_in_frame;
		eprintln!("First uninit = {:?}", self.first_uninitialized_local);
	}
}

struct PendingDeconstructions {
	first_in_frame: LocalId,
	bomb: DropBomb,
}
impl PendingDeconstructions {
	fn abandon(mut self) {
		self.bomb.defuse();
	}
}
struct PendingInitialization {
	first_in_frame: LocalId,
	first_after_frame: LocalId,
	bomb: DropBomb,
}
impl PendingInitialization {
	fn ensure_pending(&self, local: LocalId) {
		assert!(
			local.defined_before(self.first_after_frame)
				&& !local.defined_before(self.first_in_frame),
			"sanity: expected to be pending"
		);
	}
	fn indexes(&self) -> impl Iterator<Item = LocalId> {
		(self.first_in_frame.0..self.first_after_frame.0).map(LocalId)
	}
}
struct PendingUsage {
	first_in_frame: LocalId,
	first_after_frame: LocalId,
	bomb: DropBomb,
}

#[allow(clippy::too_many_lines)]
fn analyze(expr: &LocExpr, stack: &mut AnalysisStack) -> AnalysisResult {
	let mut res = AnalysisResult::default();
	let span = expr.span();
	match expr.expr() {
		// Locals
		Expr::ArrComp(elem, comp) => {
			todo!("FORSPEC WORKS AS LOCAL");
		}
		Expr::LocalExpr(l, v) => return analyze_local(&l, stack, |stack| analyze(v, stack)),

		// Objects
		Expr::Obj(obj) => return analyze_object(obj, stack),

		// Dependencies
		Expr::Var(v) => {
			stack.use_local(v, span, &mut res);
		}
		Expr::Literal(l) => match l {
			LiteralType::This | LiteralType::Super => {
				res.depend_on_object(stack.last_object());
			}
			LiteralType::Dollar => {
				res.depend_on_object(stack.first_object());
			}
			LiteralType::Null | LiteralType::True | LiteralType::False => {}
		},

		// Boring
		Expr::Str(_) => {}
		Expr::Num(_) => {}
		Expr::Arr(a) => {
			for elem in a {
				let elem_res = analyze(elem, stack);
				res.taint_by(&elem_res);
			}
		}
		Expr::UnaryOp(_, value) => {
			res.taint_by(&analyze(value, stack));
		}
		Expr::BinaryOp(left, _, right) => {
			res.taint_by(&analyze(left, stack));
			res.taint_by(&analyze(right, stack));
		}
		Expr::AssertExpr(AssertStmt(cond, message), rest) => {
			res.taint_by(&analyze(cond, stack));
			if let Some(message) = message {
				res.taint_by(&analyze(message, stack));
			}
			res.taint_by(&analyze(rest, stack));
		}
		Expr::Import(v) | Expr::ImportStr(v) | Expr::ImportBin(v) => {
			assert!(
				matches!(v.expr(), Expr::Str(_)),
				"import with non-string expression is not allowed"
			);
		}
		Expr::ErrorStmt(e) => {
			res.taint_by(&analyze(e, stack));
		}
		Expr::Apply(applicable, args, _) => {
			res.taint_by(&analyze(applicable, stack));
			for arg in &args.unnamed {
				res.taint_by(&analyze(arg, stack));
			}
			let mut passed = FxHashSet::default();
			for (name, arg) in &args.named {
				assert!(passed.insert(name), "argument was passed twice: {name}");
				res.taint_by(&analyze(arg, stack));
			}
		}
		Expr::Function(_, _) => todo!(),
		Expr::IfElse {
			cond,
			cond_then,
			cond_else,
		} => {
			res.taint_by(&analyze(&cond.0, stack));
			res.taint_by(&analyze(cond_then, stack));
			if let Some(cond_else) = cond_else {
				res.taint_by(&analyze(cond_else, stack));
			}
		}
		Expr::Slice(expr, SliceDesc { start, end, step }) => {
			res.taint_by(&analyze(expr, stack));
			if let Some(start) = &start {
				res.taint_by(&analyze(start, stack));
			}
			if let Some(end) = &end {
				res.taint_by(&analyze(end, stack));
			}
			if let Some(step) = &step {
				res.taint_by(&analyze(step, stack));
			}
		}
		Expr::Index { indexable, parts } => {
			res.taint_by(&analyze(indexable, stack));
			for ele in parts {
				res.taint_by(&analyze(&ele.value, stack));
			}
		}
	}
	res
}
fn analyze_object(obj: &ObjBody, stack: &mut AnalysisStack) -> AnalysisResult {
	todo!()
}

#[must_use]
fn process_destruct(
	bind: &Destruct,
	stack: &mut AnalysisStack,
	dupe: &PendingDeconstructions,
) -> Option<()> {
	match bind {
		Destruct::Full(f) => stack.define_local(dupe, f.clone()),
	}
}
trait Local {
	fn destruct(&self) -> &Destruct;
	fn initialize(
		&self,
		stack: &mut AnalysisStack,
		dupe: &PendingInitialization,
		ids: &mut impl Iterator<Item = LocalId>,
		taint: &mut AnalysisResult,
	) -> Option<()>;
}
fn initialize_destruct_from_result(
	destruct: &Destruct,
	result: AnalysisResult,

	stack: &mut AnalysisStack,
	dupe: &PendingInitialization,
	ids: &mut impl Iterator<Item = LocalId>,
	taint: &mut AnalysisResult,
) {
	match destruct {
		Destruct::Full(_) => {
			stack.initialize_local(dupe, ids.next().expect("not finished yet"), result, taint);
		}
	}
}
impl Local for BindSpec {
	fn destruct(&self) -> &Destruct {
		match &self {
			Self::Field { into, value: _ } => into,
			Self::Function {
				name,
				params: _,
				value: _,
			} => name,
		}
	}

	fn initialize(
		&self,
		stack: &mut AnalysisStack,
		dupe: &PendingInitialization,
		ids: &mut impl Iterator<Item = LocalId>,
		taint: &mut AnalysisResult,
	) -> Option<()> {
		match &self {
			Self::Field { into, value } => {
				let res = analyze(value, stack);
				initialize_destruct_from_result(into, res, stack, dupe, ids, taint);
			}
			Self::Function {
				name,
				params,
				value,
			} => {
				let res = analyze_local(&params.0, stack, |stack| analyze(value, stack));
				initialize_destruct_from_result(name, res, stack, dupe, ids, taint);
			}
		};
		Some(())
	}
}
impl Local for Param {
	fn destruct(&self) -> &Destruct {
		&self.0
	}

	fn initialize(
		&self,
		stack: &mut AnalysisStack,
		dupe: &PendingInitialization,
		ids: &mut impl Iterator<Item = LocalId>,
		taint: &mut AnalysisResult,
	) -> Option<()> {
		let res = self
			.1
			.as_ref()
			.map(|e| analyze(e, stack))
			.unwrap_or_default();
		initialize_destruct_from_result(&self.0, res, stack, dupe, ids, taint);
		Some(())
	}
}

#[allow(clippy::struct_field_names)]
struct Closures {
	/// All the referenced locals, maybe repeated multiple times
	/// It is recorded as continous vec of sets, I.e we have
	/// a = 1, 2, 3
	/// b = 3, 4, 5, 6
	/// And in `referenced` we have `[ 1, 2, 3, 3, 4, 5, 6 ]`. To actually get, which closure refers to which element, see `closures`...
	referenced: Vec<LocalId>,

	/// Amount of elements per closure, for the above case it is a = 3, b = 4, so here
	/// lies `[ 3, 4 ]`
	/// ~~closures: Vec<usize>,~~
	/// Finally, we have destructs.
	/// Because single destruct references single closure, but destructs to multiple locals, we have even more complicated structure.
	/// Luckly, every destruct is not interleaved with each other, so here we can have full list...
	/// Imagine having (LocalId(20), LocalId(21)), we need to save it to the Map, but we know that the numbers are sequential, so here we store number of consequent elements
	/// for each destruct starting from `first_destruct_local`
	/// ~~destructs: Vec<usize>,~~
	///
	/// => two of those fields were merged, as there is currently no per-destruct tracking of closures.
	closures_destructs: Vec<(usize, usize)>,

	/// This is not a related doccomment, just a continuation of docs for previous fields.
	/// Having
	/// ```jsonnet
	/// local
	///     [a, b, c] = [d, e, f],
	///     [d, e, f] = [a, b, c, h],
	///     h = 1,
	/// ;
	/// ```
	///
	/// We have total of 7 locals
	/// First local here is `a` => `first_destruct_local` = `a`
	/// For first closure `[a, b, c] = [d, e, f]` we have 3 referenced locals = [d, e, f] => `referenced += [d, e, f]`, `closures += 3`; 3 destructs = [a, b, c] => `destructs += 3`
	/// [d, e, f] = [a, b, c, h], => `referenced += [a, b, c, h]`, `closures += 4`, `destructs += 3` (Note that this destruct will fail at runtime,
	///                                                                                               this thing should not care about that, it only captures what the value are referencing)
	/// h = 1 => referenced += [], closures += 0, destructs += 1
	/// And the result is
	///
	/// ```
	/// Closures {
	///     referenced: vec![d, e, f, a, b, c, h]
	///     closures: vec![3, 4, 0],
	///     destructs: vec![3, 3, 1],
	///     first_destruct_label: a,
	/// }
	/// ```
	///
	/// Reconstruction of that:
	///
	/// We know that we start with a
	/// We get the first number from destructs: `destructs.shift() == 3` => `destructs = [3, 1]`
	/// 3 elements counting from a => [a, b, c]
	/// Then we take first number from closures: `closures.shift() == 3` => `closures = [4, 0]`
	/// Then we take 3 items from referenced: `referenced.shift()x3 == d, e, f` => `referenced = [a, b, c, h]`
	///
	/// Thus we have [a, b, c] = [d, e, f]
	///
	/// ~~TODO: Merge closures and destructs? I don't think I interested in closure per destruct, but it is possible o implement.~~ - merged
	first_destruct_local: LocalId,
}
impl Closures {
	fn new(first_local: LocalId) -> Self {
		Self {
			first_destruct_local: first_local,
			closures_destructs: vec![],
			referenced: vec![],
		}
	}
	fn process(&self, mut handle: impl FnMut(Closure<'_>)) {
		let mut referenced = self.referenced.as_slice();
		let mut current_local = self.first_destruct_local;
		for (closures, destructs) in self.closures_destructs.iter().copied() {
			let (this_referenced, next_referenced) = referenced.split_at(closures);
			for _ in 0..destructs {
				handle(Closure {
					local: current_local,
					references_locals: this_referenced,
				});
				current_local.0 += 1;
			}
			referenced = next_referenced;
		}
	}
}
struct Closure<'i> {
	local: LocalId,
	references_locals: &'i [LocalId],
}

fn analyze_local<T: Local>(
	specs: &[T],
	stack: &mut AnalysisStack,
	handle_inside: impl FnOnce(&mut AnalysisStack) -> AnalysisResult,
) -> AnalysisResult {
	let pending_decon = stack.start_local_deconstructions();

	let mut had_errors = false;
	for local in specs {
		if process_destruct(local.destruct(), stack, &pending_decon).is_none() {
			had_errors = true;
		}
	}
	// Can't continue after failed destructuring, as some local ids were not allocated.
	if had_errors {
		pending_decon.abandon();
		return AnalysisResult::default();
	}

	let pending_init = stack.finish_local_deconstructions(pending_decon);

	let mut res = AnalysisResult::default();

	let mut ids = pending_init.indexes();

	let mut closures = Closures::new(
		pending_init
			.indexes()
			.next()
			.expect("empty local blocks are forbidden"),
	);

	for spec in specs {
		let mut destructs = 0;
		spec.initialize(
			stack,
			&pending_init,
			&mut (&mut ids).inspect(|_| {
				destructs += 1;
			}),
			&mut res,
		);

		let referenced_before = closures.referenced.len();
		for may_referenced_id in pending_init.indexes() {
			let may_referenced = &mut stack.local_defs[may_referenced_id.0];
			if may_referenced.referened {
				closures.referenced.push(may_referenced_id);
			}
			may_referenced.referened = false;
		}
		let referenced_after = closures.referenced.len();

		closures
			.closures_destructs
			.push((referenced_after - referenced_before, destructs));
	}

	assert!(
		ids.next().is_none() || stack.errored,
		"locals uninitialized!"
	);

	let pending_usage = stack.finish_local_initializations(pending_init);

	stack.depth += 1;

	let inner_res = handle_inside(stack);
	res.taint_by(&inner_res);

	stack.depth -= 1;

	stack.finish_local_usages(&closures, pending_usage);

	res
}

impl Default for AnalysisResult {
	fn default() -> Self {
		Self {
			object_dependent_depth: u32::MAX,
			local_dependent_depth: u32::MAX,
		}
	}
}

pub fn analyze_root(state: State, expr: &LocExpr, ctx: impl ContextInitializer) {
	let mut builder = ContextBuilder::new(state);
	ctx.populate(expr.span().0, &mut builder);
	let mut stack = AnalysisStack::new(expr.span().0.code());
	for binding in builder.binding_list_for_analysis() {
		stack.define_external_local(binding);
	}
	let _ = analyze(expr, &mut stack);
	let source = hi_doc::source_to_ansi(&stack.result.build());
	println!("{source}");
}
