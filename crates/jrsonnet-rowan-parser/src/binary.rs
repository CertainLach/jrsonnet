#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOperator {
	Mul,
	Div,
	Mod,
	Plus,
	Minus,
	ShiftLeft,
	ShiftRight,
	LessThan,
	GreaterThan,
	LessThanOrEqual,
	GreaterThanOrEqual,
	Equal,
	NotEqual,
	BitAnd,
	BitXor,
	BitOr,
	And,
	Or,
	In,
	ObjectApply,
	Invalid,
}

impl BinaryOperator {
	pub fn binding_power(&self) -> (u8, u8) {
		match self {
			Self::ObjectApply => (22, 23),
			Self::Mul | Self::Div | Self::Mod => (20, 21),
			Self::Plus | Self::Minus => (18, 19),
			Self::ShiftLeft | Self::ShiftRight => (16, 17),
			Self::LessThan
			| Self::GreaterThan
			| Self::LessThanOrEqual
			| Self::GreaterThanOrEqual
			| Self::In => (14, 15),
			Self::Equal | Self::NotEqual => (12, 13),
			Self::BitAnd => (10, 11),
			Self::BitXor => (8, 9),
			Self::BitOr => (6, 7),
			Self::And => (4, 5),
			Self::Or => (2, 3),
			Self::Invalid => (0, 1),
		}
	}
}
