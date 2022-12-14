use std::ops::{RangeBounds, RangeInclusive};

// use segment::{Segment, SegmentBuffer};
type TextPart = Segment<char, Formatting>;
type Text = SegmentBuffer<char, Formatting>;

mod segment;
use segment::{Meta, Segment, SegmentBuffer};
use smallvec::smallvec;

#[derive(Default, Clone, PartialEq, Debug)]
struct Formatting {
	color: Option<u32>,
	bg_color: Option<u32>,
	bold: bool,
	underline: bool,
}
impl Meta for Formatting {
	fn try_merge(&mut self, other: &Self) -> bool {
		self == other
	}

	type Apply = Formatting;

	fn apply(&mut self, change: &Self::Apply) {
		if let Some(color) = change.color {
			self.color = Some(color);
		}
		if let Some(bg_color) = change.bg_color {
			self.bg_color = Some(bg_color);
		}
		if change.bold {
			self.bold = true;
		}
		if change.underline {
			self.underline = true;
		}
	}
}
impl Formatting {
	fn line_number() -> Self {
		Self {
			color: Some(0x92837400),
			bg_color: Some(0x28282800),
			..Default::default()
		}
	}
	fn error() -> Self {
		Self {
			bg_color: Some(0xff000000),
			..Default::default()
		}
	}
	fn color(color: u32) -> Self {
		Self {
			color: Some(color),
			..Default::default()
		}
	}
}

#[derive(Clone)]
struct RawLine {
	data: Text,
}
impl RawLine {
	fn print(&self) {
		for frag in self.data.iter() {
			if let Some(color) = frag.meta().color {
				let [r, g, b, _a] = u32::to_be_bytes(color);
				print!("\x1b[38;2;{r};{g};{b}m");
			}
			if let Some(bg_color) = frag.meta().bg_color {
				let [r, g, b, _a] = u32::to_be_bytes(bg_color);
				print!("\x1b[48;2;{r};{g};{b}m");
			}
			print!("{}", frag.iter().copied().collect::<String>());
			if frag.meta().color.is_some() || frag.meta().bg_color.is_some() {
				print!("\x1b[0m");
			}
		}
	}
}

#[derive(Debug)]
struct InlineAnnotation {
	range: RangeInclusive<usize>,
	range_fmt: Formatting,
	range_char: char,
	text: Text,
}



struct TextLine {
	prefix: Text,
	line_num: usize,
	line: Text,
	inline_annotations: Vec<InlineAnnotation>,
	annotation_buffers: Vec<Text>,
}
impl TextLine {
	fn add_prefix(&mut self, this: Text, annotations: Text) {
		self.prefix.extend(this);
		for ele in self.annotation_buffers.iter_mut() {
			ele.splice(0..0, Some(annotations.clone()));
		}
	}
	fn len(&self) -> usize {
		self.line.len()
	}
	fn is_empty(&self) -> bool {
		self.line.is_empty()
	}
	// fn trim_end(&mut self) {
	// 	self.line.truncate(self.line.trim_end().len());
	// }
}

fn cons_slices<T>(mut slice: &mut [T], test: impl Fn(&T) -> bool) -> Vec<&mut [T]> {
	let mut out = Vec::new();

	while !slice.is_empty() {
		dbg!(slice.len());
		let mut skip = 0;
		while !slice.get(skip).map(&test).unwrap_or(true) {
			skip += 1;
		}
		let mut take = 0;
		while slice.get(skip + take).map(&test).unwrap_or(false) {
			take += 1;
		}
		let (_skipped, rest) = slice.split_at_mut(skip);
		let (taken, rest) = rest.split_at_mut(take);
		if !taken.is_empty() {
			out.push(taken);
		}
		slice = rest;
	}

	out
}

enum Line {
	Text(TextLine),
	TextAnnotation(RawLine),
	Raw(RawLine),
	Delimiter,
	Nop,
	Gap(RawLine),
}
impl Line {
	fn is_text(&self) -> bool {
		matches!(self, Self::Text(_))
	}
	fn is_text_annotation(&self) -> bool {
		matches!(self, Self::TextAnnotation(_))
	}
	fn is_gap(&self) -> bool {
		matches!(self, Self::Gap(_))
	}
	fn as_text_mut(&mut self) -> Option<&mut TextLine> {
		match self {
			Line::Text(t) => Some(t),
			_ => None,
		}
	}
	fn as_gap_mut(&mut self) -> Option<&mut RawLine> {
		match self {
			Line::Gap(t) => Some(t),
			_ => None,
		}
	}
	fn as_text(&self) -> Option<&TextLine> {
		match self {
			Line::Text(t) => Some(t),
			_ => None,
		}
	}
	fn as_raw(&self) -> Option<&RawLine> {
		match self {
			Line::Raw(r) => Some(r),
			_ => None,
		}
	}
	fn is_nop(&self) -> bool {
		matches!(self, Self::Nop)
	}
}

struct GlobalAnnotation {
	range: RangeInclusive<usize>,
	text: Text,
}
struct Source {
	lines: Vec<Line>,
	global: Vec<GlobalAnnotation>,
}

fn cleanup_nops(source: &mut Source) {
	let mut i = 0;
	while i < source.lines.len() {
		if source.lines[i].is_nop() {
			source.lines.remove(i);
		} else {
			i += 1;
		}
	}
}
fn cleanup(source: &mut Source) {
	for slice in cons_slices(&mut source.lines, Line::is_text) {
		for line in slice
			.iter_mut()
			.take_while(|l| l.as_text().unwrap().is_empty())
		{
			*line = Line::Nop;
		}
		for line in slice
			.iter_mut()
			.rev()
			.take_while(|l| l.as_text().unwrap().is_empty())
		{
			*line = Line::Nop;
		}
	}
	cleanup_nops(source);
	for slice in cons_slices(&mut source.lines, Line::is_gap) {
		if slice.len() == 1 {
			continue;
		}
		for ele in slice.iter_mut().skip(1) {
			*ele = Line::Nop;
		}
	}
	cleanup_nops(source);
}

fn process(source: &mut Source) {
	cleanup(source);
	// Format inline annotations
	{
		for line in source
			.lines
			.iter_mut()
			.flat_map(Line::as_text_mut)
			.filter(|t| !t.inline_annotations.is_empty())
		{
			if line.inline_annotations.len() == 1 {
				let annotation = &line.inline_annotations[0];
				line.line
					.apply_meta(annotation.range.clone(), &annotation.range_fmt);
				line.line.extend(SegmentBuffer::new(smallvec![Segment::new(
					annotation.range_fmt.clone(),
					smallvec![' ', '▶', ' ']
				),]));
				line.line.extend(annotation.text.clone());
			} else {
				line.inline_annotations.sort_by(|a, b| {
					a.range
						.start()
						.cmp(b.range.start())
						.then(b.range.clone().count().cmp(&a.range.clone().count()))
				});
				dbg!(&line.inline_annotations);
				let max_pos = *line
					.inline_annotations
					.iter()
					.map(|a| a.range.end())
					.max()
					.unwrap();
				let mut ranges = vec![];
				let mut processed = 0;
				while !line.inline_annotations[processed..].is_empty() {
					let annotation = &line.inline_annotations[processed];
					if let Some(prev) = processed.checked_sub(1) {
						if line.inline_annotations[prev]
							.range
							.clone()
							.any(|p| annotation.range.contains(&p))
						{
							let buf = Text::new(smallvec![Segment::new(
								Formatting::default(),
								smallvec![' '; max_pos + 1]
							)]);
							ranges.push(buf);
						}
					}
					{
						let nested = ranges.len().saturating_sub(1);
						for range in ranges.iter_mut().take(nested) {
							range.apply_meta(
								annotation.range.start()..=annotation.range.start(),
								&Formatting {
									bg_color: annotation.range_fmt.color,
									..Default::default()
								},
							);
						}
					}
					if let Some(range) = ranges.last_mut() {
						let data = Text::new(smallvec![Segment::new(
							annotation.range_fmt.clone(),
							smallvec![annotation.range_char; annotation.range.clone().count()],
						)]);
						range.splice(annotation.range.clone(), Some(data));
					} else {
						line.line
							.apply_meta(annotation.range.clone(), &annotation.range_fmt);
					}

					processed += 1;
				}
				ranges.reverse();

				for (i, annotation) in line.inline_annotations.iter().rev().enumerate() {
					let mut total_padding = *annotation.range.start();
					for line in line.inline_annotations.iter().rev().skip(i) {
						total_padding = total_padding.max(*line.range.start());
					}
					total_padding += 4;
					let mut segment = Text::new(smallvec![Segment::new(
						Formatting::default(),
						smallvec![' '; total_padding]
					)]);
					if line
						.inline_annotations
						.iter()
						.rev()
						.skip(i)
						.skip(1)
						.any(|a| a.range.start() == annotation.range.start())
					{
						segment.splice(
							annotation.range.start() + 1..=annotation.range.start() + 1,
							Some(SegmentBuffer::new(smallvec![Segment::new(
								annotation.range_fmt.clone(),
								smallvec!['├']
							),])),
						);
					} else {
						segment.splice(
							annotation.range.start() + 1..=annotation.range.start() + 2,
							Some(SegmentBuffer::new(smallvec![Segment::new(
								annotation.range_fmt.clone(),
								smallvec!['╰', '─', '─']
							),])),
						);
					}
					for line in line
						.inline_annotations
						.iter()
						.filter(|a| a.range.start() < annotation.range.start())
					{
						segment.splice(
							line.range.start() + 1..=line.range.start() + 1,
							Some(SegmentBuffer::new(smallvec![Segment::new(
								line.range_fmt.clone(),
								smallvec!['│']
							),])),
						);
					}
					segment.extend(annotation.text.clone());
					ranges.push(segment);
					// ranges.push(SegmentBuffer::new(smallvec![Segment::new(
					// 	annotation.
					// )]))
				}

				for range in ranges {
					line.annotation_buffers.push(range);
				}
			}

			// line.annotation_buffers.push(data_buf);

			// let line = smallvec![];

			// for ele in line.inline_annotations.iter() {}
		}
	}
	// Make gaps in files
	for slice in cons_slices(&mut source.lines, Line::is_text) {
		'line: for i in 0..slice.len() {
			for j in i.saturating_sub(2)..(i + 3) {
				let Some(ctx) = slice.get(j) else {
					continue;
				};
				let Line::Text(t) = ctx else {
					continue;
				};
				if t.inline_annotations.is_empty() {
					continue;
				}
				continue 'line;
			}
			slice[i] = Line::Gap(RawLine {
				data: Text::new(smallvec![]),
			});
		}
	}
	cleanup(source);

	// Bake line prefixes
	{
		for source_group in cons_slices(&mut source.lines, |l| {
			l.is_text() || l.is_text_annotation() || l.is_gap()
		}) {
			let max_prefix_len = source_group
				.iter()
				.flat_map(|l| l.as_text())
				.map(|l| {
					if l.line_num == 0 {
						1
					} else {
						l.line_num.ilog10() + 1
					}
				})
				.max()
				.unwrap()
				.max(1) as usize;
			for line in source_group.iter_mut() {
				if let Some(line) = line.as_text_mut() {
					let num = line.line_num.to_string();
					let this_prefix = Text::new(smallvec![
						Segment::new(
							Formatting::line_number(),
							smallvec![' '; max_prefix_len - num.len()],
						),
						Segment::new(Formatting::line_number(), num.chars().collect(),),
						Segment::new(Formatting::line_number(), smallvec![' ']),
						Segment::new(Formatting::default(), smallvec![' ']),
					]);
					let annotation_prefix = Text::new(smallvec![
						Segment::new(
							Formatting::line_number(),
							smallvec![' '; max_prefix_len - 1]
						),
						Segment::new(Formatting::line_number(), smallvec!['·']),
						Segment::new(Formatting::default(), smallvec![' ']),
					]);
					line.add_prefix(this_prefix, annotation_prefix)
				} else if let Some(raw) = line.as_gap_mut() {
					let annotation_prefix = Text::new(smallvec![
						Segment::new(
							Formatting::line_number(),
							smallvec![' '; max_prefix_len - 1]
						),
						Segment::new(Formatting::line_number(), smallvec!['⋮']),
						Segment::new(Formatting::default(), smallvec![' ']),
					]);
					raw.data.splice(0..0, Some(annotation_prefix));
				} else {
					unreachable!()
				}
			}
		}
	}
	// Expand annotation buffers
	{
		let mut insertions = vec![];
		for (i, line) in source
			.lines
			.iter_mut()
			.enumerate()
			.flat_map(|(i, l)| l.as_text_mut().map(|t| (i, t)))
		{
			for buf in line.annotation_buffers.drain(..) {
				insertions.push((i + 1, buf))
			}
		}
		insertions.reverse();
		for (i, l) in insertions {
			source
				.lines
				.insert(i, Line::TextAnnotation(RawLine { data: l }));
		}
	}
	// To raw
	{
		for line in &mut source.lines {
			match line {
				Line::Text(t) => {
					let mut buf = SegmentBuffer::new(smallvec![]);
					buf.extend(t.prefix.clone());
					buf.extend(t.line.clone());
					*line = Line::Raw(RawLine { data: buf });
				}
				Line::TextAnnotation(t) => *line = Line::Raw(t.clone()),
				Line::Delimiter => *line = Line::Nop,
				Line::Raw(_) | Line::Nop => {}
				Line::Gap(t) => *line = Line::Raw(t.clone()),
			}
		}
	}
	cleanup(source);
}

fn parse(txt: &str) -> Source {
	let mut lines = txt
		.split('\n')
		.map(|s| s.to_string())
		.enumerate()
		.map(|(num, line)| TextLine {
			line_num: num + 1,
			line: SegmentBuffer::new(smallvec![Segment::new(
				Formatting::default(),
				line.chars().collect()
			)]),
			prefix: SegmentBuffer::new(smallvec![]),
			inline_annotations: Vec::new(),
			annotation_buffers: Vec::new(),
		})
		.map(Line::Text)
		.collect();
	Source {
		lines,
		global: vec![],
	}
}

fn print(s: &Source) {
	for line in s.lines.iter() {
		line.as_raw()
			.expect("only raw expected after transforms")
			.print();
		println!();
	}
}

#[test]
fn test_fmt() {
	let mut s = parse(include_str!("../../jrsonnet-stdlib/src/std.jsonnet"));
	s.lines[1]
		.as_text_mut()
		.unwrap()
		.inline_annotations
		.extend(vec![
			InlineAnnotation {
				range: 2..=6,
				range_char: '~',
				range_fmt: Formatting::color(0x00ff0000),
				text: SegmentBuffer::new(smallvec![Segment::new(
					Formatting::default(),
					"Local def".chars().collect()
				)]),
			},
			InlineAnnotation {
				range: 8..=10,
				range_char: '-',
				range_fmt: Formatting::color(0x0000ff00),
				text: SegmentBuffer::new(smallvec![Segment::new(
					Formatting::default(),
					"Local name".chars().collect()
				)]),
			},
			InlineAnnotation {
				range: 12..=12,
				range_char: '=',
				range_fmt: Formatting::color(0xff000000),
				text: SegmentBuffer::new(smallvec![Segment::new(
					Formatting::default(),
					"Equals".chars().collect()
				)]),
			},
			// InlineAnnotation {
			// 	range: 3..=13,
			// 	range_char: '#',
			// 	range_fmt: Formatting::color(0xffff0000),
			// 	text: SegmentBuffer::new(smallvec![Segment::new(
			// 		Formatting::default(),
			// 		"Full local".chars().collect()
			// 	)]),
			// },
		]);

	s.lines[99]
		.as_text_mut()
		.unwrap()
		.inline_annotations
		.extend(vec![InlineAnnotation {
			range: 4..=8,
			range_char: '~',
			range_fmt: Formatting::color(0x00ff0000),
			text: SegmentBuffer::new(smallvec![Segment::new(
				Formatting::default(),
				"IDK".chars().collect()
			)]),
		}]);

	s.global.push(GlobalAnnotation {
		range: 2832..=3135,
		text: SegmentBuffer::new(smallvec![Segment::new(
			Formatting::default(),
			"TEST".chars().collect()
		)]),
	});

	process(&mut s);

	print(&s);
}
