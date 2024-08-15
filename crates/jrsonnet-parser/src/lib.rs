#![allow(clippy::redundant_closure_call, clippy::derive_partial_eq_without_eq)]

use std::{
	char,
	collections::HashSet,
	fmt,
	marker::PhantomData,
	ops::{Bound, RangeBounds, RangeFrom, RangeTo},
	rc::Rc,
	slice::SliceIndex,
	str::{CharIndices, Chars},
};

use jrsonnet_gcmodule::Trace;
use nom::{
	branch::alt,
	bytes::complete::{is_a, is_not, tag, tag_no_case, take_until},
	character::complete::{alpha1, char, digit1, one_of},
	combinator::{cut, iterator, map, map_res, not, opt, peek, recognize, value},
	error::{context, ErrorKind},
	multi::{
		fold_many0, fold_many1, many0, many0_count, many1, many1_count, many_till, separated_list1,
	},
	sequence::{delimited, preceded, separated_pair, terminated, tuple},
	AsBytes, Compare, FindSubstring, IResult, InputIter, InputLength, InputTake,
	InputTakeAtPosition, Needed, Offset, Parser, Slice,
};
mod expr;
pub use expr::*;
pub use jrsonnet_interner::IStr;
mod location;
mod source;
pub use location::CodeLocation;
pub use source::{
	Source, SourceDirectory, SourceFifo, SourceFile, SourcePath, SourcePathT, SourceVirtual,
};
use static_assertions::assert_eq_size;

pub struct ParserSettings {
	pub source: Source,
}

#[derive(Clone, Copy)]
#[repr(packed)]
pub struct Input<'i> {
	// Input length is already limited by 4GB (gence u32 offsets), yet &str carries slice length around (usize),
	// replacing this metadata with u32 start/end markers (maybe this should be start/len?)
	input: *const u8,
	start: u32,
	end: u32,
	_marker: PhantomData<&'i str>,
}

impl<'i> fmt::Debug for Input<'i> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.as_str().fmt(f)
	}
}
type Output<'i, O> = IResult<Input<'i>, O>;
impl<'i> Input<'i> {
	fn len(&self) -> usize {
		self.as_bytes().len()
	}
	fn is_empty(&self) -> bool {
		self.start == self.end
	}
	fn new(str: &str) -> Self {
		Self {
			input: str.as_ptr(),
			start: 0,
			// I don't thing it makes sense to propagate this error.
			// TODO: How does other jsonnet implementations handle such error?
			end: str
				.len()
				.try_into()
				.expect("parser input is limited by 4gb files"),
			_marker: PhantomData,
		}
	}
	fn _new_pos(str: &str, start: u32) -> Self {
		Self {
			input: str.as_ptr(),
			start,

			// This method is not part of public api, and only used by this file, no risc of 4gb overflow.
			end: str.len() as u32 + start,
			_marker: PhantomData,
		}
	}
	fn as_bytes(&self) -> &[u8] {
		// Safety: String was constructed/sliced the same way
		unsafe { std::slice::from_raw_parts(self.input, (self.end - self.start) as usize) }
	}
	fn to_bytes(self) -> &'i [u8] {
		// Safety: String was constructed/sliced the same way
		unsafe { std::slice::from_raw_parts(self.input, (self.end - self.start) as usize) }
	}
	fn as_str(&'i self) -> &'i str {
		// Safety: This struct is instantiated from &str, and slicing checks utf-8 correctness.
		unsafe { std::str::from_utf8_unchecked(self.as_bytes()) }
	}
	fn to_str(self) -> &'i str {
		// Safety: This struct is instantiated from &str, and slicing checks utf-8 correctness.
		unsafe { std::str::from_utf8_unchecked(self.to_bytes()) }
	}
	fn get<T>(&self, range: T) -> Self
	where
		T: RangeBounds<usize>,
		T: SliceIndex<str, Output = str>,
	{
		let start = match range.start_bound() {
			Bound::Included(v) => *v,
			Bound::Excluded(_) => unreachable!(),
			Bound::Unbounded => 0,
		};
		Self::_new_pos(
			self.as_str().get(range).expect("incorrect slice range"),
			start as u32 + self.start,
		)
	}
	unsafe fn get_unchecked<T>(&self, range: T) -> Self
	where
		T: RangeBounds<usize>,
		T: SliceIndex<str, Output = str>,
	{
		let start = match range.start_bound() {
			Bound::Included(v) => *v,
			Bound::Excluded(_) => unreachable!(),
			Bound::Unbounded => 0,
		};
		Self::_new_pos(
			self.as_str().get_unchecked(range),
			start as u32 + self.start,
		)
	}
}
impl AsBytes for Input<'_> {
	fn as_bytes(&self) -> &[u8] {
		self.as_bytes()
	}
}
impl InputLength for Input<'_> {
	fn input_len(&self) -> usize {
		self.as_bytes().len()
	}
}
impl InputTake for Input<'_> {
	fn take(&self, count: usize) -> Self {
		self.get(..count)
	}

	fn take_split(&self, count: usize) -> (Self, Self) {
		(self.get(count..), self.get(..count))
	}
}
impl Compare<&str> for Input<'_> {
	fn compare(&self, t: &str) -> nom::CompareResult {
		self.as_str().compare(t)
	}

	fn compare_no_case(&self, t: &str) -> nom::CompareResult {
		self.as_str().compare_no_case(t)
	}
}

impl FindSubstring<&str> for Input<'_> {
	fn find_substring(&self, substr: &str) -> Option<usize> {
		self.as_str().find_substring(substr)
	}
}
impl<'i> InputIter for Input<'i> {
	type Item = char;

	type Iter = CharIndices<'i>;

	type IterElem = Chars<'i>;

	fn iter_indices(&self) -> Self::Iter {
		self.to_str().char_indices()
	}

	fn iter_elements(&self) -> Self::IterElem {
		self.to_str().chars()
	}

	fn position<P>(&self, predicate: P) -> Option<usize>
	where
		P: Fn(Self::Item) -> bool,
	{
		todo!()
	}

	fn slice_index(&self, count: usize) -> Result<usize, nom::Needed> {
		todo!()
	}
}
impl Slice<RangeFrom<usize>> for Input<'_> {
	fn slice(&self, range: RangeFrom<usize>) -> Self {
		self.get(range)
	}
}
impl Slice<RangeTo<usize>> for Input<'_> {
	fn slice(&self, range: RangeTo<usize>) -> Self {
		self.get(range)
	}
}
impl Offset for Input<'_> {
	fn offset(&self, second: &Self) -> usize {
		(second.start - self.start) as usize
	}
}
impl InputTakeAtPosition for Input<'_> {
	type Item = char;

	fn split_at_position<P, E: nom::error::ParseError<Self>>(
		&self,
		predicate: P,
	) -> IResult<Self, Self, E>
	where
		P: Fn(Self::Item) -> bool,
	{
		match self.as_str().find(predicate) {
			// find() returns a byte index that is already in the slice at a char boundary
			Some(i) => unsafe { Ok((self.get_unchecked(i..), self.get_unchecked(..i))) },
			None => Err(nom::Err::Incomplete(Needed::new(1))),
		}
	}

	fn split_at_position1<P, E: nom::error::ParseError<Self>>(
		&self,
		predicate: P,
		e: ErrorKind,
	) -> IResult<Self, Self, E>
	where
		P: Fn(Self::Item) -> bool,
	{
		match self.as_str().find(predicate) {
			Some(0) => Err(nom::Err::Error(E::from_error_kind(*self, e))),
			Some(i) => unsafe { Ok((self.get_unchecked(i..), self.get_unchecked(..i))) },
			None => Err(nom::Err::Incomplete(Needed::new(1))),
		}
	}

	fn split_at_position_complete<P, E: nom::error::ParseError<Self>>(
		&self,
		predicate: P,
	) -> IResult<Self, Self, E>
	where
		P: Fn(Self::Item) -> bool,
	{
		match self.as_str().find(predicate) {
			// find() returns a byte index that is already in the slice at a char boundary
			Some(i) => unsafe { Ok((self.get_unchecked(i..), self.get_unchecked(..i))) },
			// the end of slice is a char boundary
			None => unsafe {
				Ok((
					self.get_unchecked(self.len()..),
					self.get_unchecked(..self.len()),
				))
			},
		}
	}

	fn split_at_position1_complete<P, E: nom::error::ParseError<Self>>(
		&self,
		predicate: P,
		e: ErrorKind,
	) -> IResult<Self, Self, E>
	where
		P: Fn(Self::Item) -> bool,
	{
		match self.as_str().find(predicate) {
			Some(0) => Err(nom::Err::Error(E::from_error_kind(*self, e))),
			// find() returns a byte index that is already in the slice at a char boundary
			Some(i) => unsafe { Ok((self.get_unchecked(i..), self.get_unchecked(..i))) },
			None => {
				if self.is_empty() {
					Err(nom::Err::Error(E::from_error_kind(*self, e)))
				} else {
					// the end of slice is a char boundary
					unsafe {
						Ok((
							self.get_unchecked(self.len()..),
							self.get_unchecked(..self.len()),
						))
					}
				}
			}
		}
	}
}

#[cfg(target_pointer_width = "64")]
assert_eq_size!(Input<'_>, (usize, usize));

fn ignore<I, O1, E, F>(parser: F) -> impl FnMut(I) -> IResult<I, (), E>
where
	F: Parser<I, O1, E>,
{
	map(parser, |_| ())
}
fn comment(input: Input<'_>) -> Output<()> {
	// peg-based parser supported escaping finishing */, but I have no idea why I tough it is possible
	let multiline = delimited(tag("/*"), take_until("*/"), tag("*/"));
	let singleline_hash = preceded(tag("#"), is_not("\n"));
	let singleline_slash = preceded(tag("//"), is_not("\n"));
	ignore(alt((multiline, singleline_hash, singleline_slash)))(input)
}
fn ws_single(input: Input<'_>) -> Output<()> {
	let ws = ignore(is_a(" \n\r\t"));
	alt((ws, comment))(input)
}
fn ws_mandatory(input: Input<'_>) -> Output<()> {
	ignore(many1_count(ws_single))(input)
}
fn ws(input: Input<'_>) -> Output<()> {
	ignore(many0_count(ws_single))(input)
}
fn in_ws<'i, O>(
	rule: impl FnMut(Input<'i>) -> Output<'i, O>,
) -> impl FnMut(Input<'i>) -> Output<'i, O> {
	delimited(ws, rule, ws)
}
fn n_ws<'i, O: fmt::Debug>(
	rule: impl FnMut(Input<'i>) -> Output<'i, O>,
) -> impl FnMut(Input<'i>) -> Output<'i, O> {
	terminated(rule, ws)
}
fn comma(input: Input<'_>) -> Output<()> {
	ignore(in_ws(char(',')))(input)
}
fn equal_sign(input: Input<'_>) -> Output<()> {
	ignore(in_ws(char('=')))(input)
}
fn plus_minus(input: Input<'_>) -> Output<()> {
	ignore(opt(one_of("+-")))(input)
}
fn number(input: Input<'_>) -> Output<f64> {
	let fract = opt(preceded(char('.'), decimal));
	let scient = opt(preceded(tag_no_case("e"), tuple((plus_minus, decimal))));
	map_res(
		recognize(tuple((plus_minus, decimal, fract, scient))),
		|s| s.as_str().replace('_', "").parse(),
	)(input)
}
/// Like `digit1`, but allows `_` in the middle of the number
fn decimal(input: Input) -> Output<()> {
	ignore(separated_list1(
		char('_'),
		// separated_list1 collects values into a vec. To avoid allocations here, replacing value with ZST,
		// so vec works just as a counter
		ignore(digit1),
	))(input)
}
fn id(input: Input<'_>) -> Output<IStr> {
	let start = many1_count(alt((ignore(alpha1), ignore(char('_')))));
	let rest = many0_count(alt((ignore(alpha1), ignore(digit1), ignore(char('_')))));
	map_res(recognize(tuple((start, rest))), |v: Input<'_>| {
		let ident = IStr::from(v.as_str());
		if RESERVED.with(|r| r.contains(&ident)) {
			return Err(ErrorKind::Tag);
		}
		Ok(ident)
	})(input)
}
thread_local! {
	static RESERVED: HashSet<IStr> = [
		"assert",
		"else",
		"error",
		"false",
		"for",
		"function",
		"if",
		"import",
		"importstr",
		"importbin",
		"in",
		"local",
		"null",
		"tailstrict",
		"then",
		"self",
		"super",
		"true",
	].into_iter().map(IStr::from).collect();
}
fn keyword<'i, 'p: 'i>(kw: &'i str) -> impl FnMut(Input<'p>) -> Output<'p, ()> + 'i {
	ignore(terminated(
		tag(kw),
		terminated(
			not(alt((ignore(digit1), ignore(alpha1), ignore(char('_'))))),
			ws,
		),
	))
}

fn destruct(input: Input<'_>) -> Output<Destruct> {
	let full = map(spanned_any(id), Destruct::Full);
	#[cfg(feature = "exp-destruct")]
	let rest = map(preceded(tag("..."), opt(id)), |v| {
		Destruct::Rest(v.map_or(DestructRest::Drop, DestructRest::Keep))
	});
	#[cfg(feature = "exp-destruct")]
	let array = map(
		delimited(
			char('['),
			tuple((
				// Start
				separated_trailing0(comma, alt((destruct, rest))),
			)),
			char(']'),
		),
		|v| todo!(),
	);
	// TODO
	// let object = map(delimited(char('{'), , char('}')), f)
	#[cfg(feature = "exp-destruct")]
	let skip = map(char('?'), |_| Destruct::Skip);

	alt((
		full,
		#[cfg(feature = "exp-destruct")]
		skip,
		#[cfg(feature = "exp-destruct")]
		rest,
		#[cfg(feature = "exp-destruct")]
		array,
		#[cfg(feature = "exp-destruct")]
		object,
	))(input)
}

fn expr(input: Input<'_>) -> Output<LocExpr> {
	map(expr_binding_power(0), |v| v.0)(input)
}
fn param(input: Input<'_>) -> Output<Param> {
	map(
		tuple((destruct, opt(preceded(equal_sign, expr)))),
		|(key, value)| Param(key, value),
	)(input)
}
fn params(input: Input<'_>) -> Output<ParamsDesc> {
	let inner = |input| {
		map(separated_trailing0(comma, param), |params| {
			ParamsDesc(Rc::new(params))
		})(input)
	};

	delimited(char('('), cut(inner), cut(char(')')))(input)
}
fn arg(input: Input) -> Output<(Option<IStr>, LocExpr)> {
	alt((
		map(expr, |v| (None, v)),
		map(separated_pair(id, equal_sign, expr), |(k, v)| (Some(k), v)),
	))(input)
}
fn args(input: Input<'_>) -> Output<ArgsDesc> {
	let inner = |input| {
		map_res(separated_trailing0(comma, arg), |args| {
			let unnamed_count = args.iter().take_while(|(n, _)| n.is_none()).count();
			let mut unnamed = Vec::with_capacity(unnamed_count);
			let mut named = Vec::with_capacity(args.len() - unnamed_count);
			let mut named_started = false;
			for (name, value) in args {
				if let Some(name) = name {
					named_started = true;
					named.push((name, value));
				} else {
					if named_started {
						return Err("unexpected unnamed argument after named");
					}
					unnamed.push(value);
				}
			}
			Ok(expr::ArgsDesc::new(unnamed, named))
		})(input)
	};
	delimited(char('('), context("arguments", cut(inner)), cut(char(')')))(input)
}
fn separated_trailing0<'i, O, O2>(
	with: impl FnMut(Input<'i>) -> Output<'i, O2> + Copy,
	del_value: impl FnMut(Input<'i>) -> Output<'i, O>,
) -> impl FnMut(Input<'i>) -> Output<'i, Vec<O>> {
	map(
		opt(terminated(
			separated_list1(with, del_value),
			tuple((ws, opt(with))),
		)),
		|v| v.unwrap_or_default(),
	)
}
fn bind(input: Input<'_>) -> Output<BindSpec> {
	map(
		tuple((
			destruct,
			in_ws(opt(params)),
			preceded(tuple((char('='), ws)), expr),
		)),
		|(into, params, value)| match params {
			None => BindSpec::Field { into, value },
			Some(params) => BindSpec::Function {
				name: into,
				params,
				value,
			},
		},
	)(input)
}
fn assertion(input: Input<'_>) -> Output<AssertStmt> {
	let (input, _) = keyword("assert")(input)?;
	cut(map(
		tuple((expr, opt(preceded(char(':'), expr)))),
		|(a, b)| AssertStmt(a, b),
	))(input)
}
fn string_block<'i>(input: Input<'i>) -> Output<'i, IStr> {
	let inner = |input: Input<'i>| -> Output<'i, IStr> {
		let (input, _header) = tuple((
			// At least one newline is from the header:
			// |||\t\t\t\n
			// ^^^
			//    ^^^^^^ - optional ws
			//          ^^ first NL, but there might be many ignored.
			many_till(ignore(is_a(" \r\t")), char('\n')),
		))(input)?;
		let (input, newlines) = many0_count(char('\n'))(input)?;
		let (input, prefix) = is_a("\t ")(input)?;

		let mut whole_line = recognize(tuple((is_not("\n"), char('\n'))));

		let (input, first_line) = whole_line(input)?;

		let (input, rest_lines) = many0(alt((
			value("\n", char('\n')),
			map(preceded(tag(prefix.to_str()), whole_line), |v| v.to_str()),
		)))(input)?;

		let (input, _final) = tuple((opt(is_a("\t ")), tag("|||")))(input)?;

		let mut out = String::with_capacity(
			newlines + first_line.len() + rest_lines.iter().copied().map(str::len).sum::<usize>(),
		);
		for _ in 0..newlines {
			out.push('\n');
		}
		out.push_str(first_line.as_str());
		out.extend(rest_lines);

		Ok((input, out.into()))
	};

	let (input, _prefix) = tag("|||")(input)?;

	cut(inner)(input)
}

fn hex_char(input: Input<'_>) -> Output<u8> {
	map(one_of("0123456789abcdefABCDEF"), |c| match c {
		'0'..='9' => c as u8 - b'0',
		'a'..='f' => c as u8 - b'a' + 10,
		'A'..='F' => c as u8 - b'A' + 10,
		_ => unreachable!(),
	})(input)
}
fn hex_byte(input: Input<'_>) -> Output<u8> {
	map(tuple((hex_char, hex_char)), |(a, b)| (a << 4) | b)(input)
}
fn unicode_char(input: Input<'_>) -> Output<char> {
	let prefix = tag("\\u");

	let cont = |input| {
		// Tag is not Copy
		let prefix = tag("\\u");

		let mut hex_unicode_surrogate = map(tuple((hex_byte, hex_byte)), |(a, b)| {
			((a as u16) << 8) | b as u16
		});

		let (input, first) = hex_unicode_surrogate(input)?;
		let first = match first {
			0xdc00..=0xdfff => {
				// FIXME: Only valid as second part of surrogate pair
				return Err(nom::Err::Error(nom::error::make_error(
					input,
					ErrorKind::IsA,
				)));
			}
			n @ 0xd800..=0xdbff => (n - 0xd800) as u32,
			n => return Ok((input, char::from_u32(n as u32).expect("correct"))),
		};

		let (input, _marker) = prefix(input)?;

		let (input, second) = hex_unicode_surrogate(input)?;
		let second = match second {
			0xdc00..=0xdfff => (second - 0xdc00) as u32,
			_ => {
				// FIXME: Invalid surrogate pair
				return Err(nom::Err::Error(nom::error::make_error(
					input,
					ErrorKind::IsA,
				)));
			}
		};

		Ok((
			input,
			char::from_u32(((first << 10) | second) + 0x10000).expect("correct"),
		))
	};

	let (input, _marker) = prefix(input)?;
	cut(cont)(input)
}
fn string_quoted(input: Input<'_>) -> Output<IStr> {
	#[derive(Clone, Copy)]
	enum StringPart<'i> {
		Raw(&'i str),
		Special(char),
	}

	let unicode_part = map(unicode_char, StringPart::Special);
	let byte_part = map(preceded(tag("\\x"), cut(hex_byte)), |v| {
		StringPart::Special(v as char)
	});
	let escape_char_part = map(
		preceded(
			char('\\'),
			cut(alt((
				value('\\', char('\\')),
				value('\u{0008}', char('b')),
				value('\u{000c}', char('f')),
				value('\n', char('n')),
				value('\r', char('r')),
				value('\t', char('t')),
				value('"', char('"')),
				value('\'', char('\'')),
				// TODO: add \x, \u for better suggestions?
			))),
		),
		StringPart::Special,
	);

	let inner = |escapeend: &'static str| {
		map(
			fold_many0(
				alt((
					map(is_not(escapeend), |v: Input<'_>| {
						StringPart::Raw(v.to_str())
					}),
					unicode_part,
					byte_part,
					escape_char_part,
				)),
				String::new,
				|mut acc, v| {
					match v {
						StringPart::Raw(s) => acc.push_str(s),
						StringPart::Special(c) => acc.push(c),
					}
					acc
				},
			),
			IStr::from,
		)
	};

	let cont = |double_quote: bool| {
		terminated(
			inner(if double_quote { "\"\\" } else { "'\\" }),
			char(if double_quote { '"' } else { '\'' }),
		)
	};

	let (input, double_quote) = alt((value(true, char('"')), value(false, char('\''))))(input)?;

	cut(cont(double_quote))(input)
}
fn string_raw(input: Input<'_>) -> Output<IStr> {
	#[derive(Clone, Copy)]
	enum StringPart<'i> {
		Raw(&'i str),
		Quote,
	}

	let inner = |quote: &'static str, quotequote: &'static str| {
		map(
			fold_many0(
				alt((
					map(is_not(quote), |v: Input<'_>| StringPart::Raw(v.to_str())),
					value(StringPart::Quote, tag(quotequote)),
				)),
				String::new,
				|mut acc, v| {
					match v {
						StringPart::Raw(s) => acc.push_str(s),
						StringPart::Quote => acc.push_str(quote),
					}
					acc
				},
			),
			IStr::from,
		)
	};
	let cont = |double_quote: bool| {
		terminated(
			if double_quote {
				inner("\"", "\"\"")
			} else {
				inner("'", "''")
			},
			char(if double_quote { '"' } else { '\'' }),
		)
	};

	let (input, double_quote) = preceded(
		char('@'),
		cut(alt((value(true, char('"')), value(false, char('\''))))),
	)(input)?;

	cut(cont(double_quote))(input)
}

fn string(input: Input<'_>) -> Output<IStr> {
	alt((string_quoted, string_raw, string_block))(input)
}

fn field_name(input: Input<'_>) -> Output<FieldName> {
	let dynamic = map(delimited(char('['), expr, char(']')), FieldName::Dyn);
	let fixed = map(alt((string, id)), FieldName::Fixed);

	alt((fixed, dynamic))(input)
}

fn visibility(input: Input<'_>) -> Output<Visibility> {
	alt((
		value(Visibility::Unhide, tag(":::")),
		value(Visibility::Hidden, tag("::")),
		value(Visibility::Normal, tag(":")),
	))(input)
}
fn obj_field(input: Input<'_>) -> Output<FieldMember> {
	#[derive(Debug)]
	enum FieldKind {
		Field { plus: bool },
		Method { params: ParamsDesc },
	}
	impl FieldKind {
		fn plus(&self) -> bool {
			match self {
				FieldKind::Field { plus } => *plus,
				FieldKind::Method { .. } => false,
			}
		}
		fn params(self) -> Option<ParamsDesc> {
			match self {
				FieldKind::Field { .. } => None,
				FieldKind::Method { params } => Some(params),
			}
		}
	}
	let field = map(opt(tag("+")), |v| FieldKind::Field { plus: v.is_some() });
	let method = map(params, |params| FieldKind::Method { params });

	let kind = alt((field, method));

	map(
		tuple((
			n_ws(field_name),
			cut(n_ws(kind)),
			cut(n_ws(visibility)),
			cut(expr),
		)),
		|(name, kind, visibility, value)| FieldMember {
			name,
			plus: kind.plus(),
			params: kind.params(),
			visibility,
			value,
		},
	)(input)
}
fn obj_local(input: Input) -> Output<BindSpec> {
	let (input, _) = keyword("local")(input)?;

	cut(in_ws(bind))(input)
}
fn member(input: Input) -> Output<Member> {
	alt((
		map(obj_field, Member::Field),
		map(obj_local, Member::BindStmt),
		map(assertion, Member::AssertStmt),
	))(input)
}
fn obj_body(input: Input) -> Output<ObjBody> {
	let inner = |input| {
		let (input, members) = separated_trailing0(comma, member)(input)?;

		let (input, compspecs) = opt(compspecs)(input)?;

		Ok((
			input,
			if let Some(compspecs) = compspecs {
				#[derive(Clone, Copy)]
				enum State {
					Pre,
					Post,
				}
				let mut state = State::Pre;
				let mut pre_locals = vec![];
				let mut post_locals = vec![];
				let mut field = None::<FieldMember>;
				for member in members {
					match (member, state) {
						(Member::BindStmt(v), State::Pre) => pre_locals.push(v),
						(Member::BindStmt(v), State::Post) => post_locals.push(v),
						(Member::Field(v), State::Pre) => {
							field = Some(v);
							state = State::Post;
						}
						(Member::Field(_), State::Post) => {
							// FIXME: only one field per objcomp
							return Err(nom::Err::Failure(nom::error::make_error(
								input,
								ErrorKind::Many0,
							)));
						}
						(Member::AssertStmt(_), _) => {
							// FIXME: asserts aren't supported in objcomp
							return Err(nom::Err::Failure(nom::error::make_error(
								input,
								ErrorKind::Many0,
							)));
						}
					}
				}

				ObjBody::ObjComp(ObjComp {
					pre_locals,
					field: field.ok_or_else(|| {
						// FIXME: field is required
						nom::Err::Failure(nom::error::make_error(input, ErrorKind::IsA))
					})?,
					post_locals,
					compspecs,
				})
			} else {
				ObjBody::MemberList(members)
			},
		))
	};
	delimited(
		char('{'),
		context("objinside", cut(in_ws(inner))),
		context("object end", cut(char('}'))),
	)(input)
}

fn compspecs(input: Input) -> Output<Vec<CompSpec>> {
	let ifspec = map(preceded(keyword("if"), cut(expr)), |v| {
		CompSpec::IfSpec(IfSpecData(v))
	});
	let forspec = map(
		preceded(
			keyword("for"),
			cut(in_ws(separated_pair(destruct, in_ws(keyword("in")), expr))),
		),
		|(dest, inv)| CompSpec::ForSpec(ForSpecData(dest, inv)),
	);

	let spec = alt((forspec, ifspec));

	// TODO: Ensure first spec is forspec?
	fold_many1(spec, Vec::new, |mut acc: Vec<_>, v| {
		acc.push(v);
		acc
	})(input)
}

fn local_expr(input: Input) -> Output<Expr> {
	let (input, _) = keyword("local")(input)?;

	map(
		cut(in_ws(separated_pair(
			separated_trailing0(comma, bind),
			n_ws(char(';')),
			dbg("local expr", expr),
		))),
		|(binds, expr)| Expr::LocalExpr(binds, expr),
	)(input)
}

fn arr_expr(input: Input) -> Output<Expr> {
	let inner = |input| {
		let (input, elems) = separated_trailing0(comma, expr)(input)?;
		let (input, specs) = opt(compspecs)(input)?;

		Ok((
			input,
			if let Some(comp) = specs {
				if elems.len() != 1 {
					// FIXME: array forspec only supports one element
					return Err(nom::Err::Failure(nom::error::make_error(
						input,
						ErrorKind::Many0,
					)));
				}
				let elem = elems.into_iter().next().expect("len == 1");
				Expr::ArrComp(elem, comp)
			} else {
				Expr::Arr(elems)
			},
		))
	};
	delimited(char('['), cut(inner), cut(char(']')))(input)
}
fn if_then_else_expr(input: Input) -> Output<Expr> {
	let (input, _) = keyword("if")(input)?;

	map(
		cut(tuple((
			expr,
			preceded(keyword("then"), expr),
			opt(preceded(keyword("else"), expr)),
		))),
		|(cond, cond_then, cond_else)| Expr::IfElse {
			cond: IfSpecData(cond),
			cond_then,
			cond_else,
		},
	)(input)
}

fn literal_expr(input: Input) -> Output<Expr> {
	let literal = alt((
		value(LiteralType::Null, keyword("null")),
		value(LiteralType::True, keyword("true")),
		value(LiteralType::False, keyword("false")),
		value(LiteralType::This, keyword("self")),
		value(LiteralType::Dollar, keyword("$")),
		value(LiteralType::Super, keyword("super")),
	));

	map(literal, Expr::Literal)(input)
}

fn import_expr(input: Input) -> Output<Expr> {
	// TODO: Parser should have this field in Import expr instead of 3 diferent expr kinds.
	#[derive(Clone, Copy)]
	enum ImportKind {
		Normal,
		String,
		Binary,
	}
	let (input, kind) = alt((
		value(ImportKind::Normal, keyword("import")),
		value(ImportKind::String, keyword("importstr")),
		value(ImportKind::Binary, keyword("importbin")),
	))(input)?;

	let (input, expr) = cut(expr)(input)?;

	// TODO: Should expr type be checked here? (Only Str allowed as import operand, yet parser outputs Expr)

	Ok((
		input,
		match kind {
			ImportKind::Normal => Expr::Import(expr),
			ImportKind::String => Expr::ImportStr(expr),
			ImportKind::Binary => Expr::ImportBin(expr),
		},
	))
}
fn function_expr(input: Input) -> Output<Expr> {
	let (input, _) = keyword("function")(input)?;

	map(cut(tuple((params, expr))), |(params, value)| {
		Expr::Function(params, value)
	})(input)
}
fn assert_expr(input: Input) -> Output<Expr> {
	map(
		separated_pair(assertion, cut(char(';')), cut(expr)),
		|(ass, v)| Expr::AssertExpr(ass, v),
	)(input)
}

#[cfg(feature = "exp-null-coaelse")]
fn index_part(input: Input) -> Output<IndexPart> {
	let (input, null_coaelse) = map(opt(value(true, char('?'))), |v| v.unwrap_or_default())(input)?;

	if null_coaelse {
		let inner = |input| {
			let (input, _) = char('.')(input)?;

			let (input, value) = alt((
				spanned(map(id, Expr::Str)),
				map(delimited(char('['), expr, char(']')), |e| e),
			))(input)?;

			IndexPart {
				value,
				null_coaelse: true,
			}
		};

		cut(inner)(input)
	} else {
		let (input, _) = char('.')(input)?;
		map(cut(spanned(map(id, Expr::Str))), |value| IndexPart {
			value,
			null_coaelse: false,
		})(input)
	}
}
#[cfg(not(feature = "exp-null-coaelse"))]
fn index_part(input: Input) -> Output<IndexPart> {
	let (input, _) = char('.')(input)?;
	map(cut(spanned(map(id, Expr::Str))), |value| IndexPart {
		value,
	})(input)
}

#[derive(Debug, Trace)]
enum Suffix {
	Args(ArgsDesc, bool),
	SliceOrIndex(SliceOrIndex),
	Index(Vec<IndexPart>),
}

fn unary_op(input: Input) -> Output<UnaryOpType> {
	let op = |ty: UnaryOpType| value(ty, tag(ty.name()));
	alt((
		op(UnaryOpType::Not),
		op(UnaryOpType::Plus),
		op(UnaryOpType::Minus),
		op(UnaryOpType::BitNot),
	))(input)
}

fn suffix(input: Input) -> Output<Spanned<Suffix>> {
	spanned_any(alt((
		// TODO: move tailstrict to argsdesc?
		map(tuple((args, opt(keyword("tailstrict")))), |(args, ts)| {
			Suffix::Args(args, ts.is_some())
		}),
		map(slice_or_index, Suffix::SliceOrIndex),
		map(many1(index_part), Suffix::Index),
	)))(input)
}

fn dbg<'i, T: fmt::Debug>(
	ctx: &'static str,
	mut handle: impl FnMut(Input<'i>) -> Output<'i, T>,
) -> impl FnMut(Input<'i>) -> Output<'i, T> {
	move |input| {
		eprintln!("entered {ctx}: {input:?}");
		let value = handle(input);
		eprintln!("exited {ctx}: {value:?}");
		value
	}
}

fn spanned<'i>(
	mut inner: impl FnMut(Input<'i>) -> Output<'i, Expr>,
) -> impl FnMut(Input<'i>) -> Output<'i, LocExpr> {
	move |input| {
		let start = input.start;
		let (input, value) = inner(input)?;
		Ok((
			input,
			LocExpr::new(value, Span(current_source(), start, input.start)),
		))
	}
}
fn spanned_any<'i, T: Trace>(
	mut inner: impl FnMut(Input<'i>) -> Output<'i, T>,
) -> impl FnMut(Input<'i>) -> Output<'i, Spanned<T>> {
	move |input| {
		let start = input.start;
		let (input, value) = inner(input)?;
		Ok((
			input,
			Spanned(value, Span(current_source(), start, input.start)),
		))
	}
}

fn lhs_unary_op(input: Input<'_>) -> Output<LocExpr> {
	let start = input.start;
	let (input, un) = unary_op(input)?;

	let (_, right_binding_power) = un.binding_power();

	let (input, (expr, end)) = cut(expr_binding_power(right_binding_power))(input)?;

	Ok((
		input,
		LocExpr::new(Expr::UnaryOp(un, expr), Span(current_source(), start, end)),
	))
}

fn lhs_basic(input: Input<'_>) -> Output<LocExpr> {
	alt((
		delimited(char('('), cut(expr), cut(char(')'))),
		// 2. Numbers are parsed before the unary op, because I want -1 to be parsed as Num(-1), not as UnaryOp(Minus, Num(1))
		spanned(map(number, Expr::Num)),
		// 1. It needs to be separated, as inner expr_binding_power consumes whitespace unnecessarily, and expression end needs to be recovered.
		lhs_unary_op,
		spanned(alt((
			literal_expr,
			map(string, Expr::Str),
			arr_expr,
			map(obj_body, Expr::Obj),
			import_expr,
			map(id, Expr::Var),
			local_expr,
			if_then_else_expr,
			function_expr,
			assert_expr,
			map(preceded(keyword("error"), cut(expr)), Expr::ErrorStmt),
		))),
	))(input)
}
fn lhs(input: Input<'_>) -> Output<LocExpr> {
	let (input, mut out) = lhs_basic(input)?;

	let mut suffixes = iterator(input, suffix);

	for Spanned(suffix, span) in suffixes.into_iter() {
		out = LocExpr::new(
			match suffix {
				Suffix::Args(a, tailstrict) => Expr::Apply(out, a, tailstrict),
				Suffix::SliceOrIndex(slice) => match slice {
					SliceOrIndex::Index(i) => Expr::Index {
						indexable: out,
						parts: vec![IndexPart {
							value: i,
							#[cfg(feature = "exp-null-coaelse")]
							null_coaelse: false,
						}],
					},
					SliceOrIndex::Slice(s) => Expr::Slice(out, s),
				},
				Suffix::Index(parts) => Expr::Index {
					indexable: out,
					parts,
				},
			},
			span,
		)
	}

	let input = match suffixes.finish() {
		Ok((input, ())) => input,
		// Recover
		Err(nom::Err::Error(nom::error::Error { input, code: _ })) => input,
		Err(e) => return Err(e),
	};
	Ok((input, out))
}

fn operator(input: Input) -> Output<BinaryOpType> {
	let op = |ty: BinaryOpType| value(ty, tag(ty.name()));
	//Better form of operator matching should be used
	alt((
		value(BinaryOpType::ObjectApply, peek(tag("{"))),
		op(BinaryOpType::Mul),
		op(BinaryOpType::Div),
		op(BinaryOpType::Mod),
		op(BinaryOpType::Add),
		op(BinaryOpType::Sub),
		op(BinaryOpType::Lhs),
		op(BinaryOpType::Rhs),
		// Prefixed by Lt-Gt
		op(BinaryOpType::Lte),
		op(BinaryOpType::Gte),
		op(BinaryOpType::Lt),
		op(BinaryOpType::Gt),
		value(BinaryOpType::In, keyword("in")),
		op(BinaryOpType::Eq),
		op(BinaryOpType::Neq),
		// Prefixed by BinAnd-BitOr
		op(BinaryOpType::And),
		op(BinaryOpType::Or),
		op(BinaryOpType::BitAnd),
		op(BinaryOpType::BitOr),
		op(BinaryOpType::BitXor),
		#[cfg(feature = "exp-null-coaelse")]
		op(BinaryOpType::NullCoalesce),
	))(input)
}

/// As this parser consumes whitespace after LHS, we somehow need to account for bytes consumed,
/// to do that - parser returns end of expression as the second tuple value.
fn expr_binding_power(
	minimum_binding_power: u8,
) -> impl FnMut(Input<'_>) -> Output<(LocExpr, u32)> {
	move |input| {
		let start = input.start;
		let (input, mut lhs) = lhs(input)?;
		let mut end = input.start;
		let (mut input, _) = ws(input)?;

		// TODO: use fold1?
		while let (input_, Some(op)) = peek(opt(operator))(input)? {
			input = input_;
			let (left_binding_power, right_binding_power) = op.binding_power();

			if left_binding_power < minimum_binding_power {
				break;
			}

			// Maybe `operator` combinator should also handle binding power?
			let (input_, op2) = n_ws(operator)(input)?;
			input = input_;
			debug_assert_eq!(op, op2, "first time we peeked, then we popd");

			let (input_, (rhs, end_)) = cut(expr_binding_power(right_binding_power))(input)?;
			input = input_;
			end = end_;

			lhs = LocExpr::new(
				Expr::BinaryOp(lhs, op, rhs),
				Span(current_source(), start, end),
			);
		}

		Ok((input, (lhs, end)))
	}
}

#[derive(Debug, Trace)]
enum SliceOrIndex {
	Index(LocExpr),
	Slice(SliceDesc),
}

fn slice_or_index(input: Input) -> Output<SliceOrIndex> {
	let inner = |input| {
		let (input, start) = opt(expr)(input)?;

		let (input, start_del) = opt(char(':'))(input)?;

		if start_del.is_some() {
			let (input, (end, step)) =
				tuple((opt(expr), opt(preceded(char(':'), opt(expr)))))(input)?;

			let step = step.flatten();

			Ok((input, SliceOrIndex::Slice(SliceDesc { start, end, step })))
		} else {
			Ok((
				input,
				SliceOrIndex::Index(start.ok_or_else(|| {
					// FIXME: missing expression
					nom::Err::Failure(nom::error::make_error(input, ErrorKind::Tag))
				})?),
			))
		}
	};
	delimited(char('['), cut(inner), cut(char(']')))(input)
}

pub type ParseError = nom::Err<nom::error::Error<()>>;
pub fn parse(str: &str, settings: &ParserSettings) -> Result<LocExpr, ParseError> {
	with_current_source(settings.source.clone(), || {
		let (input, out) = match in_ws(expr)(Input::new(str)) {
			Ok(v) => v,
			Err(e) => {
				panic!("failed: {e:#}");
			}
		};
		assert_eq!(input.as_str(), "", "some input was not eaten");
		Ok(out)
	})
}
/// Used for importstr values
pub fn string_to_expr(str: IStr, settings: &ParserSettings) -> LocExpr {
	let len = str.len();
	LocExpr::new(Expr::Str(str), Span(settings.source.clone(), 0, len as u32))
}

#[cfg(test)]
pub mod tests {
	use jrsonnet_interner::IStr;
	use BinaryOpType::*;

	use super::{expr::*, parse};
	use crate::{source::Source, ParserSettings};

	macro_rules! parse {
		($s:expr) => {
			parse(
				$s,
				&ParserSettings {
					source: Source::new_virtual("<test>".into(), IStr::empty()),
				},
			)
			.unwrap()
		};
	}

	macro_rules! el {
		($expr:expr, $from:expr, $to:expr$(,)?) => {
			LocExpr::new(
				$expr,
				Span(
					Source::new_virtual("<test>".into(), IStr::empty()),
					$from,
					$to,
				),
			)
		};
	}
	macro_rules! sp {
		($expr:expr, $from:expr, $to:expr$(,)?) => {
			Spanned(
				$expr,
				Span(
					Source::new_virtual("<test>".into(), IStr::empty()),
					$from,
					$to,
				),
			)
		};
	}

	#[test]
	fn multiline_string() {
		assert_eq!(
			parse!("|||\n    Hello world!\n     a\n|||"),
			el!(Expr::Str("Hello world!\n a\n".into()), 0, 31),
		);
		assert_eq!(
			parse!("|||\n  Hello world!\n   a\n|||"),
			el!(Expr::Str("Hello world!\n a\n".into()), 0, 27),
		);
		assert_eq!(
			parse!("|||\n\t\tHello world!\n\t\t\ta\n|||"),
			el!(Expr::Str("Hello world!\n\ta\n".into()), 0, 27),
		);
		assert_eq!(
			parse!("|||\n   Hello world!\n    a\n |||"),
			el!(Expr::Str("Hello world!\n a\n".into()), 0, 30),
		);
	}

	#[test]
	fn slice() {
		parse!("a[1:]");
		parse!("a[1::]");
		parse!("a[:1:]");
		parse!("a[::1]");
		parse!("str[:len - 1]");
	}

	#[test]
	fn string_escaping() {
		assert_eq!(
			parse!(r#""Hello, \"world\"!""#),
			el!(Expr::Str(r#"Hello, "world"!"#.into()), 0, 19),
		);
		assert_eq!(
			parse!(r#"'Hello \'world\'!'"#),
			el!(Expr::Str("Hello 'world'!".into()), 0, 18),
		);
		assert_eq!(parse!(r#"'\\\\'"#), el!(Expr::Str("\\\\".into()), 0, 6));
	}

	#[test]
	fn string_unescaping() {
		assert_eq!(
			parse!(r#""Hello\nWorld""#),
			el!(Expr::Str("Hello\nWorld".into()), 0, 14),
		);
	}

	#[test]
	fn string_verbantim() {
		assert_eq!(
			parse!(r#"@"Hello\n""World""""#),
			el!(Expr::Str("Hello\\n\"World\"".into()), 0, 19),
		);
	}

	#[test]
	fn imports() {
		assert_eq!(
			parse!("import \"hello\""),
			el!(Expr::Import(el!(Expr::Str("hello".into()), 7, 14)), 0, 14),
		);
		assert_eq!(
			parse!("importstr \"garnish.txt\""),
			el!(
				Expr::ImportStr(el!(Expr::Str("garnish.txt".into()), 10, 23)),
				0,
				23
			)
		);
		assert_eq!(
			parse!("importbin \"garnish.bin\""),
			el!(
				Expr::ImportBin(el!(Expr::Str("garnish.bin".into()), 10, 23)),
				0,
				23
			)
		);
	}

	#[test]
	fn empty_object() {
		assert_eq!(
			parse!("{}"),
			el!(Expr::Obj(ObjBody::MemberList(vec![])), 0, 2)
		);
	}

	#[test]
	fn basic_math() {
		assert_eq!(
			parse!("2+2*2"),
			el!(
				Expr::BinaryOp(
					el!(Expr::Num(2.0), 0, 1),
					Add,
					el!(
						Expr::BinaryOp(el!(Expr::Num(2.0), 2, 3), Mul, el!(Expr::Num(2.0), 4, 5)),
						2,
						5
					)
				),
				0,
				5
			)
		);
	}

	#[test]
	fn basic_math_with_indents() {
		assert_eq!(
			parse!("2 +    2   * 2    "),
			el!(
				Expr::BinaryOp(
					el!(Expr::Num(2.0), 0, 1),
					Add,
					el!(
						Expr::BinaryOp(el!(Expr::Num(2.0), 7, 8), Mul, el!(Expr::Num(2.0), 13, 14),),
						7,
						14
					),
				),
				0,
				14
			)
		);
	}

	#[test]
	fn basic_math_parened() {
		assert_eq!(
			parse!("2+(2+2*2)"),
			el!(
				Expr::BinaryOp(
					el!(Expr::Num(2.0), 0, 1),
					Add,
					el!(
						Expr::BinaryOp(
							el!(Expr::Num(2.0), 3, 4),
							Add,
							el!(
								Expr::BinaryOp(
									el!(Expr::Num(2.0), 5, 6),
									Mul,
									el!(Expr::Num(2.0), 7, 8),
								),
								5,
								8
							),
						),
						3,
						8
					),
				),
				0,
				9
			)
		);
	}

	/// Comments should not affect parsing
	#[test]
	fn comments() {
		assert_eq!(
			parse!("2//comment\n+//comment\n3/*test*/*/*test*/4"),
			el!(
				Expr::BinaryOp(
					el!(Expr::Num(2.0), 0, 1),
					Add,
					el!(
						Expr::BinaryOp(
							el!(Expr::Num(3.0), 22, 23),
							Mul,
							el!(Expr::Num(4.0), 40, 41)
						),
						22,
						41
					)
				),
				0,
				41
			)
		);
	}

	/// Comments should be able to be escaped (This behavior is not present in upstream jsonnet, I have no ide why I had
	/// implemented that, it is pretty ugly to be used)
	// #[test]
	// fn comment_escaping() {
	// 	assert_eq!(
	// 		parse!("2/*\\*/+*/ - 22"),
	// 		el!(
	// 			Expr::BinaryOp(el!(Expr::Num(2.0), 0, 1), Sub, el!(Expr::Num(22.0), 12, 14)),
	// 			0,
	// 			14
	// 		)
	// 	);
	// }

	#[test]
	fn suffix() {
		// assert_eq!(parse!("std.test"), el!(Expr::Num(2.2)));
		// assert_eq!(parse!("std(2)"), el!(Expr::Num(2.2)));
		// assert_eq!(parse!("std.test(2)"), el!(Expr::Num(2.2)));
		// assert_eq!(parse!("a[b]"), el!(Expr::Num(2.2)))
	}

	#[test]
	fn array_comp() {
		use Expr::*;
		/*
		`ArrComp(Apply(Index(Var("std") from "test.jsonnet":1-4, Var("deepJoin") from "test.jsonnet":5-13) from "test.jsonnet":1-13, ArgsDesc { unnamed: [Var("x") from "test.jsonnet":14-15], named: [] }, false) from "test.jsonnet":1-16, [ForSpec(ForSpecData("x", Var("arr") from "test.jsonnet":26-29))]) from "test.jsonnet":0-30`,
		`ArrComp(Apply(Index(Var("std") from "test.jsonnet":1-4, Str("deepJoin") from "test.jsonnet":5-13) from "test.jsonnet":1-13, ArgsDesc { unnamed: [Var("x") from "test.jsonnet":14-15], named: [] }, false) from "test.jsonnet":1-16, [ForSpec(ForSpecData("x", Var("arr") from "test.jsonnet":26-29))]) from "test.jsonnet":0-30`
				*/
		assert_eq!(
			parse!("[std.deepJoin(x) for x in arr]"),
			el!(
				ArrComp(
					el!(
						Apply(
							el!(
								Index {
									indexable: el!(Var("std".into()), 1, 4),
									parts: vec![IndexPart {
										value: el!(Str("deepJoin".into()), 5, 13),
										#[cfg(feature = "exp-null-coaelse")]
										null_coaelse: false,
									}],
								},
								4,
								13
							),
							ArgsDesc::new(vec![el!(Var("x".into()), 14, 15)], vec![]),
							false,
						),
						13,
						16
					),
					vec![CompSpec::ForSpec(ForSpecData(
						Destruct::Full(sp!("x".into(), 21, 22)),
						el!(Var("arr".into()), 26, 29)
					))]
				),
				0,
				30
			),
		)
	}

	#[test]
	fn reserved() {
		use Expr::*;
		assert_eq!(parse!("null"), el!(Literal(LiteralType::Null), 0, 4));
		assert_eq!(parse!("nulla"), el!(Var("nulla".into()), 0, 5));
	}

	#[test]
	fn multiple_args_buf() {
		parse!("a(b, null_fields)");
	}

	#[test]
	fn infix_precedence() {
		use Expr::*;
		assert_eq!(
			parse!("!a && !b"),
			el!(
				BinaryOp(
					el!(UnaryOp(UnaryOpType::Not, el!(Var("a".into()), 1, 2)), 0, 2),
					And,
					el!(UnaryOp(UnaryOpType::Not, el!(Var("b".into()), 7, 8)), 6, 8)
				),
				0,
				8
			)
		);
	}

	#[test]
	fn infix_precedence_division() {
		use Expr::*;
		assert_eq!(
			parse!("!a / !b"),
			el!(
				BinaryOp(
					el!(UnaryOp(UnaryOpType::Not, el!(Var("a".into()), 1, 2)), 0, 2),
					Div,
					el!(UnaryOp(UnaryOpType::Not, el!(Var("b".into()), 6, 7)), 5, 7)
				),
				0,
				7
			)
		);
	}

	#[test]
	fn double_negation() {
		use Expr::*;
		assert_eq!(
			parse!("!!a"),
			el!(
				UnaryOp(
					UnaryOpType::Not,
					el!(UnaryOp(UnaryOpType::Not, el!(Var("a".into()), 2, 3)), 1, 3)
				),
				0,
				3
			)
		)
	}

	#[test]
	fn array_test_error() {
		parse!("[a for a in b if c for e in f]");
		//                    ^^^^ failed code
	}

	#[test]
	fn missing_newline_between_comment_and_eof() {
		parse!(
			"{a:1}

			//+213"
		);
	}

	#[test]
	fn default_param_before_nondefault() {
		parse!("local x(foo = 'foo', bar) = null; null");
	}

	#[test]
	fn add_location_info_to_all_sub_expressions() {
		use Expr::*;
		assert_eq!(
			parse!("{} { local x = 1, x: x } + {}"),
			el!(
				BinaryOp(
					el!(
						BinaryOp(
							el!(Obj(ObjBody::MemberList(vec![])), 0, 2),
							BinaryOpType::ObjectApply,
							el!(
								Obj(ObjBody::MemberList(vec![
									Member::BindStmt(BindSpec::Field {
										into: Destruct::Full(sp!("x".into(), 11, 12)),
										value: el!(Num(1.0), 15, 16)
									}),
									Member::Field(FieldMember {
										name: FieldName::Fixed("x".into()),
										plus: false,
										params: None,
										visibility: Visibility::Normal,
										value: el!(Var("x".into()), 21, 22),
									})
								])),
								3,
								24
							),
						),
						0,
						24
					),
					BinaryOpType::Add,
					el!(Obj(ObjBody::MemberList(vec![])), 27, 29),
				),
				0,
				29
			),
		);
	}
	#[test]
	fn num() {
		use Expr::*;
		assert_eq!(parse!("-1"), el!(Num(-1.0,), 0, 2));
		assert_eq!(parse!("-1_0"), el!(Num(-10.0,), 0, 4));
	}
}
