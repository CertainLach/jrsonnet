use dprint_core::formatting::PrintItems;
use jrsonnet_rowan_parser::{nodes::TriviaKind, AstToken};

use crate::{children::ChildTrivia, p, pi};

pub enum CommentLocation {
	/// Above local, field, other things
	AboveItem,
	/// After item
	ItemInline,
	/// After all items in object
	EndOfItems,
}

#[must_use]
pub fn format_comments(comments: &ChildTrivia, loc: CommentLocation) -> PrintItems {
	let mut pi = p!(new:);

	for c in comments {
		let Ok(c) = c else {
			let mut text = c.as_ref().unwrap_err() as &str;
			while !text.is_empty() {
				let pos = text.find(|c| c == '\n' || c == '\t').unwrap_or(text.len());
				let sliced = &text[..pos];
				p!(pi: string(sliced.to_string()));
				text = &text[pos..];
				if! text.is_empty(){
					match text.as_bytes()[0] {
						b'\n' => p!(pi: nl),
						b'\t' => p!(pi: tab),
						_ => unreachable!()
					}
					text = &text[1..];
				}
			}
			continue;
		};
		match c.kind() {
			TriviaKind::Whitespace => {}
			TriviaKind::MultiLineComment => {
				let mut text = c
					.text()
					.strip_prefix("/*")
					.expect("ml comment starts with /*")
					.strip_suffix("*/")
					.expect("ml comment ends with */");
				// doc-style comment, /**
				let doc = if text.starts_with('*') {
					text = &text[1..];
					true
				} else {
					false
				};
				// Is comment starts with text immediatly, i.e /*text
				let mut immediate_start = true;
				let mut lines = text
					.split('\n')
					.map(|l| l.trim_end().to_string())
					.skip_while(|l| {
						if l.is_empty() {
							immediate_start = false;
							true
						} else {
							false
						}
					})
					.collect::<Vec<_>>();
				while lines.last().map(|l| l.is_empty()).unwrap_or(false) {
					lines.pop();
				}
				if lines.len() == 1 && !doc {
					p!(pi: str("/* ") string(lines[0].trim().to_string()) str(" */") nl)
				} else if !lines.is_empty() {
					fn common_ws_prefix<'a>(a: &'a str, b: &str) -> &'a str {
						let offset = a
							.bytes()
							.zip(b.bytes())
							.take_while(|(a, b)| a == b && (a.is_ascii_whitespace() || *a == b'*'))
							.count();
						&a[..offset]
					}
					// First line is not empty, extract ws prefix of it
					let mut common_ws_padding = (if immediate_start && lines.len() > 1 {
						common_ws_prefix(&lines[1], &lines[1])
					} else {
						common_ws_prefix(&lines[0], &lines[0])
					})
					.to_string();
					for line in lines
						.iter()
						.skip(if immediate_start { 2 } else { 1 })
						.filter(|l| !l.is_empty())
					{
						common_ws_padding = common_ws_prefix(&common_ws_padding, line).to_string();
					}
					for line in lines
						.iter_mut()
						.skip(if immediate_start { 1 } else { 0 })
						.filter(|l| !l.is_empty())
					{
						*line = line
							.strip_prefix(&common_ws_padding)
							.expect("all non-empty lines start with this padding")
							.to_string();
					}

					p!(pi: str("/*"));
					if doc {
						p!(pi: str("*"));
					}
					p!(pi: nl);
					for mut line in lines {
						if doc {
							p!(pi: str(" *"));
						}
						if line.is_empty() {
							p!(pi: nl);
						} else {
							if doc {
								p!(pi: str(" "));
							}
							while let Some(new_line) = line.strip_prefix('\t') {
								if doc {
									p!(pi: str("    "));
								} else {
									p!(pi: tab);
								}
								line = new_line.to_string();
							}
							p!(pi: string(line.to_string()) nl)
						}
					}
					if doc {
						p!(pi: str(" "));
					}
					p!(pi: str("*/") nl)
				}
			}
			// TODO: Keep common padding for multiple continous lines of single-line comments
			// I.e
			// ```
			// #  Line1
			// #    Line2
			// ```
			// Should be reformatted as
			// ```
			// # Line1
			// #   Line2
			// ```
			// But currently comment formatter is not aware of continous comment lines, and reformats it as
			// ```
			// # Line1
			// # Line2
			// ```
			TriviaKind::SingleLineHashComment => {
				if matches!(loc, CommentLocation::ItemInline) {
					p!(pi: str(" "))
				}
				p!(pi: str("# ") string(c.text().strip_prefix('#').expect("hash comment starts with #").trim().to_string()));
				if !matches!(loc, CommentLocation::ItemInline) {
					p!(pi: nl)
				}
			}
			TriviaKind::SingleLineSlashComment => {
				if matches!(loc, CommentLocation::ItemInline) {
					p!(pi: str(" "))
				}
				p!(pi: str("// ") string(c.text().strip_prefix("//").expect("comment starts with //").trim().to_string()));
				if !matches!(loc, CommentLocation::ItemInline) {
					p!(pi: nl)
				}
			}
			// Garbage in - garbage out
			TriviaKind::ErrorCommentTooShort => p!(pi: str("/*/")),
			TriviaKind::ErrorCommentUnterminated => p!(pi: string(c.text().to_string())),
		}
	}

	pi
}
