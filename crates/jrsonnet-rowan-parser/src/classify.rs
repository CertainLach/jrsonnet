use crate::SyntaxKind;

impl SyntaxKind {
	pub fn is_trivia(self) -> bool {
		matches!(
			self,
			Self::WHITESPACE
				| Self::MULTI_LINE_COMMENT
				| Self::ERROR_COMMENT_TOO_SHORT
				| Self::ERROR_COMMENT_UNTERMINATED
				| Self::SINGLE_LINE_HASH_COMMENT
				| Self::SINGLE_LINE_SLASH_COMMENT
		)
	}
	pub fn is_string(self) -> bool {
		matches!(
			self,
			Self::STRING_SINGLE
				| Self::ERROR_STRING_SINGLE_UNTERMINATED
				| Self::STRING_DOUBLE
				| Self::ERROR_STRING_DOUBLE_UNTERMINATED
				| Self::STRING_SINGLE_VERBATIM
				| Self::ERROR_STRING_SINGLE_VERBATIM_UNTERMINATED
				| Self::STRING_DOUBLE_VERBATIM
				| Self::ERROR_STRING_DOUBLE_VERBATIM_UNTERMINATED
				| Self::STRING_BLOCK
				| Self::ERROR_STRING_BLOCK_UNEXPECTED_END
				| Self::ERROR_STRING_BLOCK_MISSING_NEW_LINE
				| Self::ERROR_STRING_BLOCK_MISSING_TERMINATION
				| Self::ERROR_STRING_BLOCK_MISSING_INDENT
		)
	}
	pub fn is_number(self) -> bool {
		matches!(
			self,
			Self::FLOAT
				| Self::ERROR_FLOAT_JUNK_AFTER_POINT
				| Self::ERROR_FLOAT_JUNK_AFTER_EXPONENT
				| Self::ERROR_FLOAT_JUNK_AFTER_EXPONENT_SIGN
		)
	}
	pub fn is_literal(self) -> bool {
		matches!(
			self,
			Self::NULL_KW
				| Self::TRUE_KW | Self::FALSE_KW
				| Self::SELF_KW | Self::DOLLAR
				| Self::SUPER_KW
		)
	}
}
