use std::str::Chars;

fn decode_unicode(chars: &mut Chars) -> Option<u16> {
	IntoIterator::into_iter([chars.next()?, chars.next()?, chars.next()?, chars.next()?])
		.map(|c| c.to_digit(16).map(|f| f as u16))
		.try_fold(0u16, |acc, v| Some((acc << 4) | (v?)))
}

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
			'u' => match decode_unicode(&mut chars)? {
				// May only be second byte
				0xDC00..=0xDFFF => return None,
				// Surrogate pair
				n1 @ 0xD800..=0xDBFF => {
					if chars.next() != Some('\\') {
						return None;
					}
					if chars.next() != Some('u') {
						return None;
					}
					let n2 = decode_unicode(&mut chars)?;
					if !matches!(n2, 0xDC00..=0xDFFF) {
						return None;
					}
					let n = (((n1 - 0xD800) as u32) << 10 | (n2 - 0xDC00) as u32) + 0x1_0000;
					out.push(char::from_u32(n)?);
				}
				n => out.push(char::from_u32(n as u32)?),
			},
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
