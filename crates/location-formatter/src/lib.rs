use std::{collections::HashMap, ops::RangeInclusive};

// use segment::{Segment, SegmentBuffer};
type TextPart = Segment<char, Formatting>;
type Text = SegmentBuffer<char, Formatting>;

mod segment;
use range_map::{Range, RangeSet};
use segment::{Meta, Segment, SegmentBuffer};
use single_line::{AnnotationId, LineAnnotation, Opts};

mod chars;
mod single_line;

#[derive(Default, Clone, PartialEq, Debug)]
pub struct Formatting {
	color: Option<u32>,
	bg_color: Option<u32>,
	bold: bool,
	underline: bool,
	decoration: bool,
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
	fn decoration(mut self) -> Self {
		self.decoration = true;
		self
	}
}

pub(crate) fn print_buf(buf: &Text) {
	for frag in buf.segments() {
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

#[derive(Clone)]
struct RawLine {
	data: Text,
}
impl RawLine {
	fn print(&self) {
		print_buf(&self.data);
		// for frag in self.data.iter() {}
	}
}

struct AnnotationLine {
	prefix: Text,
	line: Text,
	annotation: Option<AnnotationId>,
}

struct GapLine {
	prefix: Text,
	line: Text,
}

struct TextLine {
	prefix: Text,
	line_num: usize,
	line: Text,
	annotations: Vec<LineAnnotation>,
	annotation_buffers: Vec<(Option<AnnotationId>, Text)>,
}
impl TextLine {
	fn add_prefix(&mut self, this: Text, annotations: Text) {
		self.prefix.extend(this);
		for (_, ele) in self.annotation_buffers.iter_mut() {
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
	Annotation(AnnotationLine),
	Raw(RawLine),
	Nop,
	Gap(GapLine),
}
impl Line {
	fn text_mut(&mut self) -> Option<&mut Text> {
		Some(match self {
			Line::Text(t) => &mut t.line,
			Line::Gap(t) => &mut t.line,
			Line::Annotation(t) => &mut t.line,
			_ => return None,
		})
	}
	fn is_text(&self) -> bool {
		matches!(self, Self::Text(_))
	}
	fn is_annotation(&self) -> bool {
		matches!(self, Self::Annotation(_))
	}
	fn as_annotation(&self) -> Option<&AnnotationLine> {
		match self {
			Self::Annotation(a) => Some(a),
			_ => None,
		}
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
	fn as_gap_mut(&mut self) -> Option<&mut GapLine> {
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

fn process(source: &mut Source, opts: &Opts) {
	cleanup(source);
	// Format inline annotations
	{
		for line in source
			.lines
			.iter_mut()
			.flat_map(Line::as_text_mut)
			.filter(|t| !t.annotations.is_empty())
		{
			let (replace, extra) =
				single_line::generate_segment(line.annotations.clone(), line.line.clone(), opts);
			line.line = replace;
			line.annotation_buffers = extra;
			line.annotations.truncate(0);
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
				if t.annotation_buffers.is_empty() {
					continue;
				}
				continue 'line;
			}
			slice[i] = Line::Gap(GapLine {
				prefix: Text::new([]),
				line: Text::new([]),
			});
		}
	}
	cleanup(source);

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
		for (i, (annotation, line)) in insertions {
			source.lines.insert(
				i,
				Line::Annotation(AnnotationLine {
					line,
					annotation,
					prefix: SegmentBuffer::new([]),
				}),
			);
		}
	}
	// Connect annotation lines
	{
		for lines in &mut cons_slices(&mut source.lines, |l| {
			l.is_annotation() || l.is_text() || l.is_gap()
		}) {
			struct Connection {
				range: Range<usize>,
				connected: Vec<usize>,
			}

			let mut connected_annotations = HashMap::new();
			for (i, line) in lines.iter().enumerate() {
				if let Some(annotation) = line.as_annotation() {
					if let Some(annotation) = annotation.annotation {
						let conn = connected_annotations
							.entry(annotation)
							.or_insert(Connection {
								range: Range::new(i, i),
								connected: Vec::new(),
							});
						conn.range.start = conn.range.start.min(i);
						conn.range.end = conn.range.end.max(i);
						conn.connected.push(i);
					}
				}
			}
			let mut grouped = connected_annotations
				.iter()
				.map(|(k, v)| (*k, vec![v.range].into_iter().collect::<RangeSet<usize>>()))
				.collect::<Vec<_>>();
			grouped.sort_by_key(|a| a.1.num_elements());
			let grouped = single_line::group_nonconflicting(grouped);

			for group in grouped {
				for annotation in group {
					let annotation_fmt = Formatting::default().decoration();
					let conn = connected_annotations.get(&annotation).expect("exists");
					let range = conn.range;
					let mut max_index = usize::MAX;
					for line in range.start..=range.end {
						match &lines[line] {
							Line::Text(t) if t.line.data().all(|c| c.is_whitespace()) => {}
							Line::Text(t) => {
								print_buf(&t.line);
								println!();
								let whitespaces =
									t.line.data().take_while(|i| i.is_whitespace()).count();
								dbg!(whitespaces);
								max_index = max_index.min(whitespaces)
							}
							Line::Annotation(t) if t.line.data().all(|c| c.is_whitespace()) => {}
							Line::Annotation(t) => {
								print_buf(&t.line);
								println!();
								let whitespaces =
									t.line.data().take_while(|i| i.is_whitespace()).count();
								dbg!(whitespaces);
								max_index = max_index.min(whitespaces)
							}
							Line::Gap(_) => {}
							_ => unreachable!(),
						}
					}
					while max_index < 2 {
						let seg = Some(SegmentBuffer::new([Segment::new(
							vec![' '; 2 - max_index],
							annotation_fmt.clone(),
						)]));
						for line in lines.iter_mut() {
							match line {
								Line::Text(t) => t.line.splice(0..0, seg.clone()),
								Line::Annotation(t) => t.line.splice(0..0, seg.clone()),
								Line::Gap(t) => t.line.splice(0..0, seg.clone()),
								_ => unreachable!(),
							}
						}
						max_index = 2;
					}
					if max_index >= 2 {
						let offset = max_index - 2;

						for line in range.start..=range.end {
							use chars::line::*;
							let char = if range.start == range.end {
								RANGE_EMPTY
							} else if line == range.start {
								RANGE_START
							} else if line == range.end {
								RANGE_END
							} else if conn.connected.contains(&line) {
								RANGE_CONNECTION
							} else {
								RANGE_CONTINUE
							};
							let text = lines[line].text_mut().expect("only with text reachable");
							if text.len() <= offset {
								text.resize(offset + 1, ' ', annotation_fmt.clone());
							}
							text.splice(
								offset..=offset,
								Some(SegmentBuffer::new([Segment::new(
									[char],
									annotation_fmt.clone(),
								)])),
							);

							if conn.connected.contains(&line) {
								for i in offset + 1..text.len() {
									let (char, fmt) = text.get(i).expect("in bounds");
									if !text.get(i).expect("in bounds").0.is_whitespace()
										&& !fmt.decoration
									{
										break;
									}
									if let Some((keep_style, replacement)) = cross(char) {
										text.splice(
											i..=i,
											Some(SegmentBuffer::new([Segment::new(
												[replacement],
												if keep_style {
													fmt
												} else {
													annotation_fmt.clone()
												},
											)])),
										)
									}
								}
							}
						}
					}
				}
			}

			// dbg!(grouped);
		}

		// todo!()
	}
	// Apply line numbers
	{
		for lines in &mut cons_slices(&mut source.lines, |l| {
			l.is_annotation() || l.is_text() || l.is_gap()
		}) {
			let max_num = lines
				.iter()
				.filter_map(|l| match l {
					Line::Text(t) => Some(t.line_num),
					_ => None,
				})
				.max()
				.unwrap_or(0);
			let max_len = max_num.to_string().len();
			let prefix_segment = Segment::new(vec![' '; max_len - 1], Formatting::line_number());
			for line in lines.iter_mut() {
				match line {
					Line::Text(t) => t.prefix.extend(SegmentBuffer::new([Segment::new(
						format!("{:>width$} ", t.line_num, width = max_len).chars(),
						Formatting::line_number(),
					)])),
					Line::Annotation(a) => a.prefix.extend(SegmentBuffer::new([
						prefix_segment.clone(),
						Segment::new(['·', ' '], Formatting::line_number()),
					])),
					Line::Gap(a) => a.prefix.extend(SegmentBuffer::new([
						prefix_segment.clone(),
						Segment::new(['⋮', ' '], Formatting::line_number()),
					])),
					_ => unreachable!(),
				}
			}
		}
	}
	// To raw
	{
		for line in &mut source.lines {
			match line {
				Line::Text(t) => {
					let mut buf = SegmentBuffer::new([]);
					buf.extend(t.prefix.clone());
					buf.extend(t.line.clone());
					*line = Line::Raw(RawLine { data: buf });
				}
				Line::Annotation(t) => {
					let mut buf = SegmentBuffer::new([]);
					buf.extend(t.prefix.clone());
					buf.extend(t.line.clone());
					*line = Line::Raw(RawLine { data: buf })
				}
				Line::Gap(t) => {
					let mut buf = SegmentBuffer::new([]);
					buf.extend(t.prefix.clone());
					buf.extend(t.line.clone());
					*line = Line::Raw(RawLine { data: buf })
				}
				Line::Raw(_) | Line::Nop => {}
			}
		}
	}
	cleanup(source);
}

fn parse(txt: &str) -> Source {
	let lines = txt
		.split('\n')
		.map(|s| s.to_string())
		.enumerate()
		.map(|(num, line)| TextLine {
			line_num: num + 1,
			line: SegmentBuffer::new([Segment::new(line.chars(), Formatting::default())]),
			prefix: SegmentBuffer::new([]),
			annotations: Vec::new(),
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
	use range_map::Range;
	use single_line::AnnotationIdAllocator;
	let mut aid = AnnotationIdAllocator::new();
	let mut s = parse(include_str!("../../jrsonnet-stdlib/src/std.jsonnet"));

	let local_def = aid.next();
	s.lines[1].as_text_mut().unwrap().annotations.extend(vec![
		LineAnnotation {
			id: local_def,
			priority: 0,
			ranges: vec![Range::new(2, 6)].into_iter().collect(),
			formatting: Formatting::color(0x00ff0000),
			left: true,
			right: vec![SegmentBuffer::new([Segment::new(
				"Local def".chars(),
				Formatting::default(),
			)])],
		},
		LineAnnotation {
			id: aid.next(),
			priority: 0,
			ranges: vec![Range::new(8, 10)].into_iter().collect(),
			formatting: Formatting::color(0x0000ff00),
			left: false,
			right: vec![SegmentBuffer::new([Segment::new(
				"Local name".chars(),
				Formatting::default(),
			)])],
		},
		LineAnnotation {
			id: aid.next(),
			priority: 0,
			ranges: vec![Range::new(12, 12)].into_iter().collect(),
			formatting: Formatting::color(0xff000000),
			left: false,
			right: vec![SegmentBuffer::new([Segment::new(
				"Equals".chars(),
				Formatting::default(),
			)])],
		},
	]);

	s.lines[99]
		.as_text_mut()
		.unwrap()
		.annotations
		.extend(vec![LineAnnotation {
			id: local_def,
			priority: 0,
			ranges: vec![Range::new(4, 8)].into_iter().collect(),
			formatting: Formatting::color(0x00ff0000),
			left: true,
			right: vec![SegmentBuffer::new([Segment::new(
				"Another local def".chars(),
				Formatting::default(),
			)])],
		}]);

	{
		let connected = aid.next();
		s.lines[188]
			.as_text_mut()
			.unwrap()
			.annotations
			.extend(vec![LineAnnotation {
				id: connected,
				priority: 0,
				ranges: vec![Range::new(10, 14)].into_iter().collect(),
				formatting: Formatting::color(0x00ffff00),
				left: true,
				right: vec![],
			}]);

		s.lines[191]
			.as_text_mut()
			.unwrap()
			.annotations
			.extend(vec![LineAnnotation {
				id: connected,
				priority: 0,
				ranges: vec![Range::new(10, 14)].into_iter().collect(),
				formatting: Formatting::color(0x00ffff00),
				left: true,
				right: vec![],
			}]);

		s.lines[194]
			.as_text_mut()
			.unwrap()
			.annotations
			.extend(vec![LineAnnotation {
				id: connected,
				priority: 0,
				ranges: vec![Range::new(10, 12)].into_iter().collect(),
				formatting: Formatting::color(0x00ffff00),
				left: true,
				right: vec![SegmentBuffer::new([Segment::new(
					"Example connected definition".chars(),
					Formatting::default(),
				)])],
			}])
	}
	{
		let conflicting_connection = aid.next();
		s.lines[97]
			.as_text_mut()
			.unwrap()
			.annotations
			.extend(vec![LineAnnotation {
				id: conflicting_connection,
				priority: 0,
				ranges: vec![Range::new(6, 7)].into_iter().collect(),
				formatting: Formatting::color(0xffff0000),
				left: true,
				right: vec![],
			}]);

		s.lines[193]
			.as_text_mut()
			.unwrap()
			.annotations
			.extend(vec![LineAnnotation {
				id: conflicting_connection,
				priority: 0,
				ranges: vec![Range::new(12, 14)].into_iter().collect(),
				formatting: Formatting::color(0xffff0000),
				left: true,
				right: vec![SegmentBuffer::new([Segment::new(
					"Example connected definition".chars(),
					Formatting::default(),
				)])],
			}])
	}

	s.global.push(GlobalAnnotation {
		range: 2832..=3135,
		text: SegmentBuffer::new([Segment::new("TEST".chars(), Formatting::default())]),
	});

	process(
		&mut s,
		&Opts {
			ratnest_sort: false,
			ratnest_merge: false,
			first_layer_reformats_orig: true,
		},
	);

	print(&s);
}
