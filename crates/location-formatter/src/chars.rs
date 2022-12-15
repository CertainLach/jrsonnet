pub mod line {
	pub const CONTINUE: char = '─';
	pub const CONTINUE_CROSS: char = '━';

	pub const RANGE_EMPTY: char = '╶';
	pub const RANGE_EMPTY_CROSS: char = '╼';
	pub const RANGE_EMPTY_CROSS_CROSS: char = '━';

	pub const RANGE_START: char = '╭';
	pub const RANGE_START_CROSS: char = '┮';
	pub const RANGE_START_CROSS_CROSS: char = '┯';

	pub const RANGE_END: char = '╰';
	pub const RANGE_END_CROSS: char = '┶';
	pub const RANGE_END_CROSS_CROSS: char = '┷';

	pub const RANGE_CONNECTION: char = '├';
	pub const RANGE_CONNECTION_CROSS: char = '┾';
	pub const RANGE_CONNECTION_CROSS_CROSS: char = '┿';

	pub const RANGE_CONTINUE: char = '│';
	pub const RANGE_CONTINUE_CROSS: char = '┼';
	pub const RANGE_CONTINUE_CROSS_CROSS: char = '┿';

	pub fn cross(char: char) -> Option<(bool, char)> {
		match char {
			CONTINUE => Some((true, CONTINUE_CROSS)),
			#[allow(unreachable_patterns)]
			CONTINUE_CROSS | RANGE_EMPTY_CROSS_CROSS => None,

			RANGE_EMPTY => Some((true, RANGE_EMPTY_CROSS)),
			RANGE_EMPTY_CROSS => Some((true, RANGE_EMPTY_CROSS_CROSS)),

			RANGE_START => Some((true, RANGE_START_CROSS)),
			RANGE_START_CROSS => Some((true, RANGE_START_CROSS_CROSS)),
			RANGE_START_CROSS_CROSS => None,

			RANGE_END => Some((true, RANGE_END_CROSS)),
			RANGE_END_CROSS => Some((true, RANGE_END_CROSS_CROSS)),
			RANGE_END_CROSS_CROSS => None,

			RANGE_CONNECTION => Some((true, RANGE_CONNECTION_CROSS)),
			RANGE_CONNECTION_CROSS => Some((true, RANGE_CONNECTION_CROSS_CROSS)),
			#[allow(unreachable_patterns)]
			RANGE_CONNECTION_CROSS_CROSS | RANGE_CONTINUE_CROSS_CROSS => None,

			RANGE_CONTINUE => Some((false, RANGE_CONTINUE_CROSS)),
			RANGE_CONTINUE_CROSS => Some((true, RANGE_CONTINUE_CROSS_CROSS)),

			' ' => Some((false, CONTINUE)),

			c => unreachable!("{c:?}"),
		}
	}
}

pub mod arrow {
	pub const CONTINUE: char = '│';
	pub const CONTINUE_CROSS: char = '┃';

	pub const ARROW_RL: char = '┴';
	pub const ARROW_RL_CROSS: char = '╀';
	pub const ARROW_RL_CROSS_CROSS: char = '╂';

	pub const ARROW_L: char = '╯';
	pub const ARROW_L_CROSS: char = '┦';
	pub const ARROW_L_CROSS_CROSS: char = '┨';

	pub const ARROW_R: char = '╰';
	pub const ARROW_R_CROSS: char = '┞';
	pub const ARROW_R_CROSS_CROSS: char = '┠';

	pub const ARROW_CONTINUE: char = '─';
	pub const ARROW_CONTINUE_CROSS: char = '┼';
	pub const ARROW_CONTINUE_CROSS_CROSS: char = '╂';

	pub fn cross(char: char) -> Option<(bool, char)> {
		match char {
			CONTINUE => Some((true, CONTINUE_CROSS)),
			CONTINUE_CROSS => None,

			ARROW_CONTINUE => Some((false, ARROW_CONTINUE_CROSS)),
			ARROW_CONTINUE_CROSS => Some((true, ARROW_CONTINUE_CROSS_CROSS)),
			#[allow(unreachable_patterns)]
			ARROW_CONTINUE_CROSS_CROSS | ARROW_RL_CROSS_CROSS => None,

			ARROW_R => Some((false, ARROW_R_CROSS)),
			ARROW_R_CROSS => Some((true, ARROW_R_CROSS_CROSS)),
			ARROW_R_CROSS_CROSS => None,

			ARROW_L => Some((false, ARROW_L_CROSS)),
			ARROW_L_CROSS => Some((true, ARROW_L_CROSS_CROSS)),
			ARROW_L_CROSS_CROSS => None,

			ARROW_RL => Some((false, ARROW_RL_CROSS)),
			ARROW_RL_CROSS => Some((true, ARROW_RL_CROSS_CROSS)),

			' ' => Some((false, CONTINUE)),

			c => unreachable!("{c:?}"),
		}
	}
}

pub mod single {
	pub const CONTINUE: char = '│';
	pub const CONTINUE_CROSS: char = '┃';

	pub const RANGE_START: char = '├';
	pub const RANGE_START_CROSS: char = '┠';

	pub const RANGE_CONTINUE: char = '─';
	pub const RANGE_CONTINUE_CROSS: char = '┼';
	pub const RANGE_CONTINUE_CROSS_CROSS: char = '╂';

	pub const RANGE_END: char = '╯';
	pub const RANGE_END_CROSS: char = '┦';
	pub const RANGE_END_CROSS_CROSS: char = '┨';

	pub fn cross(char: char) -> Option<(bool, char)> {
		match char {
			CONTINUE => Some((true, CONTINUE_CROSS)),
			CONTINUE_CROSS => None,

			RANGE_START => Some((true, RANGE_START_CROSS)),
			RANGE_START_CROSS => None,

			RANGE_CONTINUE => Some((false, RANGE_CONTINUE_CROSS)),
			RANGE_CONTINUE_CROSS => Some((true, RANGE_CONTINUE_CROSS_CROSS)),
			RANGE_CONTINUE_CROSS_CROSS => None,

			RANGE_END => Some((true, RANGE_END_CROSS)),
			RANGE_END_CROSS => Some((true, RANGE_END_CROSS_CROSS)),
			RANGE_END_CROSS_CROSS => None,

			' ' => Some((false, CONTINUE)),

			c => unreachable!("{c:?}"),
		}
	}
}
