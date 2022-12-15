/// Mutable rich text implementation
use std::{
	convert::Infallible,
	fmt::Debug,
	ops::{Bound, Deref, DerefMut, RangeBounds},
};

use smallvec::{smallvec, SmallVec};

pub trait Meta: Clone {
	type Apply;
	fn try_merge(&mut self, other: &Self) -> bool;
	fn apply(&mut self, change: &Self::Apply);
}
impl Meta for usize {
	fn try_merge(&mut self, other: &Self) -> bool {
		if *self != *other {
			return false;
		}
		true
	}

	type Apply = Infallible;

	fn apply(&mut self, change: &Self::Apply) {
		unreachable!()
	}
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Segment<D, M> {
	meta: M,
	data: SmallVec<[D; 16]>,
}
impl<D, M> Segment<D, M> {
	pub fn new(data: impl IntoIterator<Item = D>, meta: M) -> Self {
		Self {
			meta,
			data: data.into_iter().collect(),
		}
	}
	#[inline]
	pub fn meta(&self) -> &M {
		&self.meta
	}
}
impl<D, M> Deref for Segment<D, M> {
	type Target = SmallVec<[D; 16]>;

	fn deref(&self) -> &Self::Target {
		&self.data
	}
}
impl<D, M> DerefMut for Segment<D, M> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.data
	}
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct SegmentBuffer<D, M> {
	// Can be replaced with Vec<u8> and segments to (UserId, Range<usize>), instead of keeping every buffer inside of segment,
	// But it only would be faster for compaction, inserts would be slower
	segments: SmallVec<[Segment<D, M>; 1]>,
	len: usize,
}
impl<D: Clone + Debug, M: Meta + Debug> SegmentBuffer<D, M> {
	pub fn empty() -> Self {
		Self {
			segments: smallvec![],
			len: 0,
		}
	}
	pub fn new(segments: impl IntoIterator<Item = Segment<D, M>>) -> Self {
		let mut len = 0;
		let segments = segments.into_iter().inspect(|s| len += s.len()).collect();
		Self { segments, len }
	}
	pub fn compact(&mut self) {
		if self.segments.len() <= 1 {
			return;
		}
		let mut last_segment = 0;
		let mut removed = Vec::new();
		loop {
			if self.segments.len() < last_segment + 1 {
				break;
			}
			let (first, rest) = self.segments.split_at_mut(last_segment + 1);
			if rest.is_empty() {
				break;
			}
			let first = &mut first[first.len() - 1];
			let mut merged = 0;
			while rest.len() != merged && first.meta.try_merge(&rest[merged].meta) {
				first.data.extend(rest[merged].data.drain(..));
				merged += 1;
			}
			removed.push((last_segment + 1)..(last_segment + 1 + merged));
			last_segment = last_segment + 1 + merged;
		}
		for range in removed.into_iter().rev() {
			self.segments.drain(range);
		}
	}
	pub fn slice(&self, range: impl RangeBounds<usize>) -> Self {
		let mut segments = SmallVec::new();
		let mut len = 0;
		let mut start = match range.start_bound() {
			Bound::Included(i) => *i,
			Bound::Excluded(_) => unreachable!(),
			Bound::Unbounded => 0,
		};
		let mut end = match range.end_bound() {
			Bound::Included(i) => *i + 1,
			Bound::Excluded(i) => *i,
			Bound::Unbounded => self.len(),
		};
		dbg!(start, end);
		if end > self.len() {
			panic!("slice out of range: {end}")
		}
		for segment in self.segments.iter() {
			dbg!(segment.len());
			if start < segment.len() {
				let end = segment.len().min(end);
				segments.push(Segment::new(
					segment[start..end].iter().cloned(),
					segment.meta().clone(),
				));
				len += end - start;
			}
			start = start.saturating_sub(segment.len());
			end = end.saturating_sub(segment.len());
			dbg!(start, end);
			if end == 0 {
				break;
			}
		}
		Self { segments, len }
	}

	pub fn get(&self, offset: usize) -> Option<(D, M)> {
		if offset > self.len() {
			return None;
		}
		dbg!(offset, self.len());
		eprintln!("Slice in get: {offset}");
		let segment = &self.slice(offset..=offset).segments[0];
		dbg!(segment);
		Some((segment.data[0].clone(), segment.meta.clone()))
	}

	pub fn splice(&mut self, range: impl RangeBounds<usize>, insert: Option<SegmentBuffer<D, M>>) {
		let mut start = match range.start_bound() {
			Bound::Included(i) => *i,
			Bound::Excluded(_) => unreachable!(),
			Bound::Unbounded => 0,
		};
		let mut end = match range.end_bound() {
			Bound::Included(i) => *i + 1,
			Bound::Excluded(i) => *i,
			Bound::Unbounded => self.len(),
		};
		if end > self.len() {
			panic!("splice out of range: {end}")
		}
		let mut insert_at = None;
		let mut segment_idx = 0;
		while segment_idx < self.segments.len() {
			let segment_length = self.segments[segment_idx].len();
			if start < segment_length {
				println!("In segment {segment_idx}: {start}");
				let removed = start..end.min(segment_length);
				if start == 0 {
					println!("Start");
					// Beginning of segment
					// abcdefg
					// ^
					if removed.end < segment_length {
						// Start of segment
						// abcdefg
						// ^-^
						if insert_at.is_none() {
							insert_at = Some(segment_idx);
						}
						let old_segment = &self.segments[segment_idx];
						let new_segment = Segment::new(
							old_segment[removed.end..].iter().cloned(),
							old_segment.meta().clone(),
						);
						self.len -= removed.end;
						self.segments[segment_idx] = new_segment;
					} else {
						// Full segment
						// abcdefg
						// ^-----^
						self.len -= self.segments[segment_idx].len();
						self.segments.remove(segment_idx);
						if insert_at.is_none() {
							insert_at = Some(segment_idx);
						}
						segment_idx = segment_idx.saturating_sub(1);
					}
				} else {
					println!("Middle");
					// Inside of segment
					// abcdefg
					//   ^
					if insert_at.is_none() {
						insert_at = Some(segment_idx + 1);
					}
					if removed.end < segment_length {
						// Part of segment
						// abcdefg
						//   ^-^
						let old_segment = &mut self.segments[segment_idx];
						let new_segment = Segment::new(
							old_segment[removed.end..].iter().cloned(),
							old_segment.meta().clone(),
						);
						old_segment.truncate(removed.start);
						self.len -= removed.end - removed.start;
						self.segments.insert(segment_idx + 1, new_segment);
						segment_idx += 1;
					} else {
						// End of segment
						// abcdefg
						//   ^---^
						self.segments[segment_idx].truncate(removed.start);
						self.len -= removed.end - removed.start;
					}
				}
				dbg!(end);
			}
			if start < segment_length && end == start {
				if insert_at.is_none() {
					insert_at = Some(segment_idx);
				}
				break;
			}
			end = end.saturating_sub(segment_length);
			start = start.saturating_sub(segment_length);
			segment_idx += 1;
		}
		if let Some(insert) = insert {
			self.len += insert.len();
			let insert_at = insert_at.unwrap_or(self.segments.len());
			self.segments.insert_many(insert_at, insert.segments);
		}
		self.compact()
	}

	pub fn len(&self) -> usize {
		self.len
	}
	pub fn is_empty(&self) -> bool {
		self.segments.is_empty()
	}

	pub fn segments(&self) -> impl Iterator<Item = &Segment<D, M>> {
		self.segments.iter()
	}
	pub fn data(&self) -> impl Iterator<Item = &D> {
		self.segments().flat_map(|s| s.data.iter())
	}
	pub fn apply_meta(&mut self, range: impl RangeBounds<usize> + Clone, apply: &M::Apply) {
		let mut slice = self.slice(range.clone());
		for segment in slice.segments.iter_mut() {
			segment.meta.apply(apply);
		}
		self.splice(range, Some(slice));
	}
	pub fn push(&mut self, segment: Segment<D, M>) {
		self.len += segment.len();
		self.segments.push(segment);
	}
	pub fn extend(&mut self, other: SegmentBuffer<D, M>) {
		self.len += other.len;
		self.segments.extend(other.segments);
	}
	pub fn resize(&mut self, size: usize, fill: D, meta: M) {
		if self.len() > size {
			self.splice(0..self.len(), Some(self.slice(0..size)));
		} else {
			let extra = size - self.len();
			let segment = Segment::new(vec![fill; extra], meta);
			self.push(segment);
		}
	}
}

#[cfg(test)]
mod tests {
	mod compact {
		use crate::segment::Segment;
		type SegmentBuffer = crate::segment::SegmentBuffer<u8, usize>;

		#[test]
		fn simple() {
			let mut buf = SegmentBuffer::new([Segment::new([1, 2], 1), Segment::new([3, 4], 1)]);
			buf.compact();
			assert_eq!(buf, SegmentBuffer::new([Segment::new([1, 2, 3, 4], 1)]));
		}

		#[test]
		fn single() {
			let mut buf = SegmentBuffer::new([
				Segment::new([1, 2], 1),
				Segment::new([3, 4], 1),
				Segment::new([5], 2),
			]);
			buf.compact();
			assert_eq!(
				buf,
				SegmentBuffer::new([Segment::new([1, 2, 3, 4], 1), Segment::new([5], 2)])
			);
		}
	}

	mod slice {
		use crate::segment::{Segment, SegmentBuffer};

		#[test]
		fn first() {
			let input = SegmentBuffer::new([Segment::new([1, 2, 3, 4], 1)]);
			assert_eq!(input.slice(0..=3), input);

			let input = SegmentBuffer::new([Segment::new([1, 2, 3, 4], 1)]);
			assert_eq!(input.slice(0..4), input);
		}

		#[test]
		fn part() {
			let input = SegmentBuffer::new([Segment::new([1, 2, 3, 4], 1)]);
			assert_eq!(
				input.slice(0..=2),
				SegmentBuffer::new([Segment::new([1, 2, 3], 1)])
			);

			let input = SegmentBuffer::new([Segment::new([1, 2, 3, 4], 1)]);
			assert_eq!(
				input.slice(1..=3),
				SegmentBuffer::new([Segment::new([2, 3, 4], 1)])
			);
		}

		#[test]
		fn two() {
			let input =
				SegmentBuffer::new([Segment::new([1, 2, 3, 4], 1), Segment::new([5, 6, 7, 8], 1)]);
			assert_eq!(
				input.slice(2..=5),
				SegmentBuffer::new([Segment::new([3, 4], 1), Segment::new([5, 6], 1)])
			);

			let input =
				SegmentBuffer::new([Segment::new([1, 2, 3, 4], 1), Segment::new([5, 6, 7, 8], 1)]);
			assert_eq!(input.slice(0..=7), input);
		}
	}

	mod splice {
		use crate::segment::{Segment, SegmentBuffer};

		#[test]
		fn insert_start() {
			let mut buf = SegmentBuffer::new([Segment::new([1, 2, 3, 4], 1)]);
			buf.splice(0..0, Some(SegmentBuffer::new([Segment::new([5], 2)])));
			assert_eq!(
				buf,
				SegmentBuffer::new([Segment::new([5], 2), Segment::new([1, 2, 3, 4], 1)])
			)
		}

		#[test]
		fn insert_end() {
			let mut buf = SegmentBuffer::new([Segment::new([1, 2, 3, 4], 1)]);
			buf.splice(
				buf.len..buf.len,
				Some(SegmentBuffer::new([Segment::new([5], 2)])),
			);
			assert_eq!(
				buf,
				SegmentBuffer::new([Segment::new([1, 2, 3, 4], 1), Segment::new([5], 2),])
			)
		}

		#[test]
		fn insert_middle() {
			let mut buf = SegmentBuffer::new([Segment::new([1, 2, 3, 4], 1)]);
			buf.splice(2..2, Some(SegmentBuffer::new([Segment::new([5], 5)])));
			assert_eq!(
				buf,
				SegmentBuffer::new([
					Segment::new([1, 2], 1),
					Segment::new([5], 2),
					Segment::new([3, 4], 1),
				])
			)
		}

		#[test]
		fn replace_middle() {
			let mut buf = SegmentBuffer::new([Segment::new([1, 2, 3, 4], 1)]);
			buf.splice(2..=2, Some(SegmentBuffer::new([Segment::new([5], 2)])));
			assert_eq!(
				buf,
				SegmentBuffer::new([
					Segment::new([1, 2], 1),
					Segment::new([5], 2),
					Segment::new([4], 1),
				])
			)
		}

		#[test]
		fn replace_middle_overlap() {
			let mut buf = SegmentBuffer::new([Segment::new([1, 2], 1), Segment::new([3, 4], 1)]);
			buf.splice(1..3, Some(SegmentBuffer::new([Segment::new([5], 2)])));
			assert_eq!(
				buf,
				SegmentBuffer::new([
					Segment::new([1], 1),
					Segment::new([5], 2),
					Segment::new([4], 1),
				])
			)
		}
	}
}
