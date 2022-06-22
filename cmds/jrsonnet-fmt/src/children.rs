// TODO: Return errors as trivia

use std::{fmt::Debug, marker::PhantomData, mem};

use jrsonnet_rowan_parser::{
	nodes::{Trivia, TriviaKind},
	AstNode, AstToken, SyntaxElement,
	SyntaxKind::*,
	SyntaxNode, TS,
};

pub type ChildTrivia = Vec<Trivia>;

/// Node should have no non-trivia tokens before element
pub fn trivia_before(node: SyntaxNode, end: Option<&SyntaxElement>) -> ChildTrivia {
	let mut out = Vec::new();
	for item in node.children_with_tokens() {
		if Some(&item) == end {
			break;
		}

		if let Some(trivia) = item.as_token().cloned().and_then(Trivia::cast) {
			out.push(trivia);
		} else if end.is_none() {
			break;
		} else {
			assert!(
				TS![, ;].contains(item.kind()) || item.kind() == ERROR,
				"silently eaten token: {:?}",
				item.kind()
			)
		}
	}
	out
}
/// Node should have no non-trivia tokens after element
pub fn trivia_after(node: SyntaxNode, start: Option<&SyntaxElement>) -> ChildTrivia {
	if start.is_none() {
		return Vec::new();
	}
	let mut iter = node.children_with_tokens().peekable();
	while iter.peek() != start {
		iter.next();
	}
	iter.next();
	let mut out = Vec::new();
	for item in iter {
		if let Some(trivia) = item.as_token().cloned().and_then(Trivia::cast) {
			out.push(trivia);
		} else {
			assert!(
				TS![, ;].contains(item.kind()) || item.kind() == ERROR,
				"silently eaten token: {:?}",
				item.kind()
			)
		}
	}
	out
}

pub fn trivia_between(
	node: SyntaxNode,
	start: Option<&SyntaxElement>,
	end: Option<&SyntaxElement>,
) -> ChildTrivia {
	let mut iter = node.children_with_tokens().peekable();
	while iter.peek() != start {
		iter.next();
	}
	iter.next();

	let loose = start.is_none() || end.is_none();

	let mut out = Vec::new();
	for item in iter.take_while(|i| Some(i) != end) {
		if let Some(trivia) = item.as_token().cloned().and_then(Trivia::cast) {
			out.push(trivia);
		} else if loose {
			break;
		} else {
			assert!(
				TS![, ;].contains(item.kind()) || item.kind() == ERROR,
				"silently eaten token: {:?}",
				item.kind()
			)
		}
	}
	out
}

pub fn children_between<T: AstNode + Debug>(
	node: SyntaxNode,
	start: Option<&SyntaxElement>,
	end: Option<&SyntaxElement>,
) -> (Vec<Child<T>>, ChildTrivia) {
	let mut iter = node.children_with_tokens().peekable();
	while iter.peek() != start {
		iter.next();
	}
	iter.next();
	children(
		iter.take_while(|i| Some(i) != end),
		start.is_none() || end.is_none(),
	)
}

pub fn should_start_with_newline(tt: &ChildTrivia) -> bool {
	// First for previous item end
	count_newlines_before(tt) >= 2
}

fn count_newlines_before(tt: &ChildTrivia) -> usize {
	let mut nl_count = 0;
	for t in tt {
		match t.kind() {
			TriviaKind::Whitespace => {
				nl_count += t.text().bytes().filter(|b| *b == b'\n').count();
			}
			_ => break,
		}
	}
	nl_count
}
fn count_newlines_after(tt: &ChildTrivia) -> usize {
	let mut nl_count = 0;
	for t in tt.iter().rev() {
		match t.kind() {
			TriviaKind::Whitespace => {
				nl_count += t.text().bytes().filter(|b| *b == b'\n').count();
			}
			TriviaKind::SingleLineHashComment => {
				nl_count += 1;
				break;
			}
			TriviaKind::SingleLineSlashComment => {
				nl_count += 1;
				break;
			}
			_ => {}
		}
	}
	nl_count
}

pub fn children<'a, T: AstNode + Debug>(
	items: impl Iterator<Item = SyntaxElement>,
	loose: bool,
) -> (Vec<Child<T>>, ChildTrivia) {
	let mut out = Vec::new();
	let mut current_child = None::<Child<T>>;
	let mut next = ChildTrivia::new();
	// Previous element ended, do not add more inline comments
	let mut started_next = false;
	let mut had_some = false;

	for item in items {
		if let Some(value) = item.as_node().cloned().and_then(T::cast) {
			let before_trivia = mem::take(&mut next);
			let last_child = current_child.replace(Child {
				newlines_above: if had_some {
					count_newlines_before(&before_trivia)
						+ current_child
							.as_ref()
							.map(|c| count_newlines_after(&c.inline_trivia))
							.unwrap_or_default()
				} else {
					0
				},
				before_trivia,
				value,
				inline_trivia: Vec::new(),
			});
			if let Some(last_child) = last_child {
				out.push(last_child)
			}
			had_some = true;
			started_next = false;
		} else if let Some(trivia) = item.as_token().cloned().and_then(Trivia::cast) {
			let is_single_line_comment = trivia.kind() == TriviaKind::SingleLineHashComment
				|| trivia.kind() == TriviaKind::SingleLineSlashComment;
			if started_next
				|| current_child.is_none()
				|| trivia.text().contains('\n') && !is_single_line_comment
			{
				next.push(trivia.clone());
				started_next = true;
			} else {
				let cur = current_child.as_mut().expect("checked not none");
				cur.inline_trivia.push(trivia);
				if is_single_line_comment {
					started_next = true;
				}
			}
			had_some = true;
		} else if loose {
			if had_some {
				break;
			}
			started_next = true;
		} else {
			assert!(
				TS![, ;].contains(item.kind()) || item.kind() == ERROR,
				"silently eaten token: {:?}",
				item.kind()
			)
		}
	}

	if let Some(current_child) = current_child {
		out.push(current_child);
	}

	(out, next)
}

#[derive(Debug)]
pub struct Child<T> {
	newlines_above: usize,
	/// Comment before item, i.e
	///
	/// ```ignore
	/// // Comment
	/// item
	/// ```
	pub before_trivia: ChildTrivia,
	pub value: T,
	/// Comment after line, but located at same line
	///
	/// ```ignore
	/// item1, // Inline comment
	/// // Not inline comment
	/// item2,
	/// ```
	pub inline_trivia: ChildTrivia,
}

impl<T> Child<T> {
	/// If this child has two newlines above in source code, so it needs to have it in output
	pub fn needs_newline_above(&self) -> bool {
		// First line for end of previous item
		self.newlines_above >= 2
	}
}
