#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CodeLocation {
	pub offset: usize,

	pub line: usize,
	pub column: usize,

	pub line_start_offset: usize,
	pub line_end_offset: usize,
}

pub fn location_to_offset(mut file: &str, mut line: usize, column: usize) -> Option<usize> {
	let mut offset = 0;
	while line > 1 {
		let pos = file.find('\n')?;
		offset += pos + 1;
		file = &file[pos + 1..];
		line -= 1;
	}
	offset += column - 1;
	Some(offset)
}

pub fn offset_to_location<const S: usize>(file: &str, offsets: &[u32; S]) -> [CodeLocation; S] {
	if offsets.is_empty() {
		return [CodeLocation::default(); S];
	}
	let mut line = 1;
	let mut column = 1;
	let max_offset = *offsets.iter().max().expect("offsets is not empty");

	let mut offset_map = offsets
		.iter()
		.enumerate()
		.map(|(pos, offset)| (*offset, pos))
		.collect::<Vec<_>>();
	offset_map.sort_by_key(|v| v.0);
	offset_map.reverse();

	let mut out = [CodeLocation::default(); S];
	let mut with_no_known_line_ending = vec![];
	let mut this_line_offset = 0;
	for (pos, ch) in file
		.chars()
		.enumerate()
		.chain(std::iter::once((file.len(), ' ')))
	{
		column += 1;
		match offset_map.last() {
			Some(x) if x.0 == pos as u32 => {
				let out_idx = x.1;
				with_no_known_line_ending.push(out_idx);
				out[out_idx].offset = pos;
				out[out_idx].line = line;
				out[out_idx].column = column;
				out[out_idx].line_start_offset = this_line_offset;
				offset_map.pop();
			}
			_ => {}
		}
		if ch == '\n' {
			line += 1;
			column = 1;

			for idx in with_no_known_line_ending.drain(..) {
				out[idx].line_end_offset = pos;
			}
			this_line_offset = pos + 1;

			if pos == max_offset as usize + 1 {
				break;
			}
		}
	}
	let file_end = file.chars().count();
	for idx in with_no_known_line_ending {
		out[idx].line_end_offset = file_end;
	}

	out
}

#[cfg(test)]
pub mod tests {
	use super::{offset_to_location, CodeLocation};

	#[test]
	fn test() {
		assert_eq!(
			offset_to_location(
				"hello world\n_______________________________________________________",
				&[0, 14]
			),
			[
				CodeLocation {
					offset: 0,
					line: 1,
					column: 2,
					line_start_offset: 0,
					line_end_offset: 11,
				},
				CodeLocation {
					offset: 14,
					line: 2,
					column: 4,
					line_start_offset: 12,
					line_end_offset: 67
				}
			]
		)
	}
}
