use std::marker::PhantomData;

use crate::{SyntaxKind, SyntaxNode, SyntaxNodeChildren, SyntaxToken};

/// The main trait to go from untyped `SyntaxNode`  to a typed ast. The
/// conversion itself has zero runtime cost: ast and syntax nodes have exactly
/// the same representation: a pointer to the tree root and a pointer to the
/// node itself.
pub trait AstNode {
	fn can_cast(kind: SyntaxKind) -> bool
	where
		Self: Sized;

	fn cast(syntax: SyntaxNode) -> Option<Self>
	where
		Self: Sized;

	fn syntax(&self) -> &SyntaxNode;
	fn clone_for_update(&self) -> Self
	where
		Self: Sized,
	{
		Self::cast(self.syntax().clone_for_update()).unwrap()
	}
	fn clone_subtree(&self) -> Self
	where
		Self: Sized,
	{
		Self::cast(self.syntax().clone_subtree()).unwrap()
	}
}

/// Like `AstNode`, but wraps tokens rather than interior nodes.
pub trait AstToken {
	fn can_cast(token: SyntaxKind) -> bool
	where
		Self: Sized;

	fn cast(syntax: SyntaxToken) -> Option<Self>
	where
		Self: Sized;

	fn syntax(&self) -> &SyntaxToken;

	fn text(&self) -> &str {
		self.syntax().text()
	}
}

#[derive(Debug, Clone)]
pub struct AstChildren<N> {
	inner: SyntaxNodeChildren,
	ph: PhantomData<N>,
}

impl<N> AstChildren<N> {
	fn new(parent: &SyntaxNode) -> Self {
		AstChildren {
			inner: parent.children(),
			ph: PhantomData,
		}
	}
}

impl<N: AstNode> Iterator for AstChildren<N> {
	type Item = N;
	fn next(&mut self) -> Option<N> {
		self.inner.find_map(N::cast)
	}
}

pub mod support {
	use super::{AstChildren, AstNode, AstToken, SyntaxKind, SyntaxNode, SyntaxToken};

	pub fn child<N: AstNode>(parent: &SyntaxNode) -> Option<N> {
		parent.children().find_map(N::cast)
	}
	pub fn token_child<N: AstToken>(parent: &SyntaxNode) -> Option<N> {
		parent.children_with_tokens().find_map(|n| match n {
			rowan::NodeOrToken::Node(_) => None,
			rowan::NodeOrToken::Token(t) => N::cast(t),
		})
	}

	pub fn children<N: AstNode>(parent: &SyntaxNode) -> AstChildren<N> {
		AstChildren::new(parent)
	}

	pub fn token(parent: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
		parent
			.children_with_tokens()
			.filter_map(|it| it.into_token())
			.find(|it| it.kind() == kind)
	}
}
