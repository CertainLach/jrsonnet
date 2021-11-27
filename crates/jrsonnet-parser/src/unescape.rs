pub fn unescape(s: &str) -> Option<String> {
	let mut chars = s.chars();
	let mut out = String::with_capacity(s.len());

	while let Some(c) = chars.next() {
		if c != '\\' {
			out.push(c);
			continue;
		}
		match chars.next()? {
			c @ ('\\' | '"' | '\'') => out.push(c),
			'b' => out.push('\u{0008}'),
			'f' => out.push('\u{000c}'),
			'n' => out.push('\n'),
			'r' => out.push('\r'),
			't' => out.push('\t'),
			'u' => {
				let c = IntoIterator::into_iter([
					chars.next()?,
					chars.next()?,
					chars.next()?,
					chars.next()?,
				])
				.map(|c| c.to_digit(16))
				.try_fold(0u32, |acc, v| Some((acc << 8) | (v?)))?;
				out.push(char::from_u32(c)?)
			}
			'x' => {
				let c = IntoIterator::into_iter([chars.next()?, chars.next()?])
					.map(|c| c.to_digit(16))
					.try_fold(0u32, |acc, v| Some((acc << 8) | (v?)))?;
				out.push(char::from_u32(c)?)
			}
			_ => return None,
		}
	}
	Some(out)
}
