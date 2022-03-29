#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOperator {
	Minus,
	Not,
	BitNegate,
}
impl UnaryOperator {
	pub fn binding_power(&self) -> ((), u8) {
		match self {
			UnaryOperator::Minus => ((), 20),
			UnaryOperator::Not => ((), 20),
			UnaryOperator::BitNegate => ((), 20),
		}
	}
}
