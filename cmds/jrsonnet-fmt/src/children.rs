// TODO: Return errors as trivia

use std::{fmt::Debug, mem};

use jrsonnet_rowan_parser::{
	nodes::{CustomError, Trivia, TriviaKind},
	AstNode, AstToken, SyntaxElement, SyntaxNode, TS,
};

pub type ChildTrivia = Vec<Result<Trivia, String>>;

/// Node should have no non-trivia tokens before element
pub fn trivia_before(node: SyntaxNode, end: Option<&SyntaxElement>) -> ChildTrivia {
	let mut out = Vec::new();
	for item in node.children_with_tokens() {
		if Some(&item) == end {
			break;
		}

		if let Some(trivia) = item.as_token().cloned().and_then(Trivia::cast) {
			out.push(Ok(trivia));
		} else if CustomError::can_cast(item.kind()) {
			out.push(Err(item.to_string()));
		} else if end.is_none() {
			break;
		} else {
			assert!(
				TS![, ;].contains(item.kind()),
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
			out.push(Ok(trivia));
		} else if CustomError::can_cast(item.kind()) {
			out.push(Err(item.to_string()))
		} else {
			assert!(
				TS![, ;].contains(item.kind()),
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
) -> EndingComments {
	let mut iter = node.children_with_tokens().peekable();
	while iter.peek() != start {
		iter.next();
	}
	iter.next();

	let loose = start.is_none() || end.is_none();

	let mut out = Vec::new();
	for item in iter.take_while(|i| Some(i) != end) {
		if let Some(trivia) = item.as_token().cloned().and_then(Trivia::cast) {
			out.push(Ok(trivia));
		} else if CustomError::can_cast(item.kind()) {
			out.push(Err(item.to_string()))
		} else if loose {
			break;
		} else {
			assert!(
				TS![, ;].contains(item.kind()),
				"silently eaten token: {:?}",
				item.kind()
			)
		}
	}
	EndingComments {
		should_start_with_newline: should_start_with_newline(None, &out),
		trivia: out,
	}
}

pub fn children_between<T: AstNode + Debug>(
	node: SyntaxNode,
	start: Option<&SyntaxElement>,
	end: Option<&SyntaxElement>,
) -> (Vec<Child<T>>, EndingComments) {
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

pub fn should_start_with_newline(prev_inline: Option<&ChildTrivia>, tt: &ChildTrivia) -> bool {
	count_newlines_before(tt)
		+ prev_inline
			.map(count_newlines_after)
			.unwrap_or_default()

		// First for previous item end, second for current item
		>= 2
}

fn count_newlines_before(tt: &ChildTrivia) -> usize {
	let mut nl_count = 0;
	for t in tt {
		match t {
			Ok(t) => match t.kind() {
				TriviaKind::Whitespace => {
					nl_count += t.text().bytes().filter(|b| *b == b'\n').count();
				}
				_ => break,
			},
			Err(_) => {
				nl_count += 1;
			}
		}
	}
	nl_count
}
fn count_newlines_after(tt: &ChildTrivia) -> usize {
	let mut nl_count = 0;
	for t in tt.iter().rev() {
		match t {
			Ok(t) => match t.kind() {
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
			},
			Err(_) => nl_count += 1,
		}
	}
	nl_count
}

pub fn children<T: AstNode + Debug>(
	items: impl Iterator<Item = SyntaxElement>,
	loose: bool,
) -> (Vec<Child<T>>, EndingComments) {
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
				// First item should not start with newline
				should_start_with_newline: had_some
					&& should_start_with_newline(
						current_child.as_ref().map(|c| &c.inline_trivia),
						&before_trivia,
					),
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
				next.push(Ok(trivia.clone()));
				started_next = true;
			} else {
				let cur = current_child.as_mut().expect("checked not none");
				cur.inline_trivia.push(Ok(trivia));
				if is_single_line_comment {
					started_next = true;
				}
			}
			had_some = true;
		} else if CustomError::can_cast(item.kind()) {
			next.push(Err(item.to_string()))
		} else if loose {
			if had_some {
				break;
			}
			started_next = true;
		} else {
			assert!(
				TS![, ;].contains(item.kind()),
				"silently eaten token: {:?}",
				item.kind()
			)
		}
	}

	let ending_comments = EndingComments {
		should_start_with_newline: should_start_with_newline(
			current_child.as_ref().map(|c| &c.inline_trivia),
			&next,
		),
		trivia: next,
	};

	if let Some(current_child) = current_child {
		out.push(current_child);
	}

	(out, ending_comments)
}

#[derive(Debug)]
pub struct Child<T> {
	/// If this child has two newlines above in source code, so it needs to have it in the output
	pub should_start_with_newline: bool,
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

pub struct EndingComments {
	/// If this child has two newlines above in source code, so it needs to have it in the output
	pub should_start_with_newline: bool,
	pub trivia: ChildTrivia,
}
impl EndingComments {
	pub fn is_empty(&self) -> bool {
		!self.should_start_with_newline && self.trivia.is_empty()
	}
}
