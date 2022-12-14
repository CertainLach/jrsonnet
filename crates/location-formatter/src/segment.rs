use std::{
	convert::Infallible,
	ops::{Bound, Deref, DerefMut, RangeBounds},
};

use smallvec::SmallVec;

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
pub struct Segment<D, M>(M, SmallVec<[D; 16]>);
impl<D, M> Deref for Segment<D, M> {
	type Target = SmallVec<[D; 16]>;

	fn deref(&self) -> &Self::Target {
		&self.1
	}
}
impl<D, M> DerefMut for Segment<D, M> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.1
	}
}
impl<D, M> Segment<D, M> {
	pub fn new(meta: M, data: SmallVec<[D; 16]>) -> Self {
		Self(meta, data)
	}
	#[inline]
	pub fn meta(&self) -> &M {
		&self.0
	}
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct SegmentBuffer<D, M> {
	// Can be replaced with Vec<u8> and segments to (UserId, Range<usize>), instead of keeping every buffer inside of segment,
	// But it only would be faster for compaction, inserts would be slower
	segments: SmallVec<[Segment<D, M>; 1]>,
	len: usize,
}
impl<D: Clone, M: Meta> SegmentBuffer<D, M> {
	pub fn new(segments: SmallVec<[Segment<D, M>; 1]>) -> Self {
		let len = segments.iter().map(|s| s.len()).sum();
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
			while rest.len() != merged && first.0.try_merge(&rest[merged].0) {
				first.1.extend(rest[merged].1.drain(..));
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
		if end > self.len() {
			panic!("slice out of range: {}", end)
		}
		for segment in self.segments.iter() {
			if start <= segment.len() {
				let end = segment.len().min(end);
				segments.push(Segment(segment.meta().clone(), segment[start..end].into()));
				len += end - start;
			}
			start = start.saturating_sub(segment.len());
			end = end.saturating_sub(segment.len());
			if end == 0 {
				break;
			}
		}
		Self { segments, len }
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
			panic!("splice out of range: {}", end)
		}
		let mut insert_at = None;
		let mut segment_idx = 0;
		while segment_idx < self.segments.len() {
			let segment_length = self.segments[segment_idx].len();
			if start < segment_length {
				println!("In segment {}: {}", segment_idx, start);
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
						let new_segment = Segment(
							old_segment.meta().clone(),
							old_segment[removed.end..].into(),
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
						let new_segment = Segment(
							old_segment.meta().clone(),
							old_segment[removed.end..].into(),
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

	pub fn iter(&self) -> impl Iterator<Item = &Segment<D, M>> {
		self.segments.iter()
	}
	pub fn apply_meta(&mut self, range: impl RangeBounds<usize> + Clone, apply: &M::Apply) {
		let mut slice = self.slice(range.clone());
		for segment in slice.segments.iter_mut() {
			segment.0.apply(&apply);
		}
		self.splice(range, Some(slice));
	}
	pub fn extend(&mut self, other: SegmentBuffer<D, M>) {
		self.len += other.len;
		self.segments.extend(other.segments);
	}
}

#[cfg(test)]
mod tests {
	mod compact {
		use smallvec::smallvec;

		// use crate::segment::{Segment, SegmentBuffer};
		// type Segment = crate::segment::Segment<usize>;
		use crate::segment::Segment;
		type SegmentBuffer = crate::segment::SegmentBuffer<u8, usize>;

		#[test]
		fn simple() {
			let mut buf = SegmentBuffer::new(smallvec![
				Segment::new(1, smallvec![1, 2]),
				Segment::new(1, smallvec![3, 4]),
			]);
			buf.compact();
			assert_eq!(
				buf,
				SegmentBuffer::new(smallvec![Segment::new(1, smallvec![1, 2, 3, 4])])
			);
		}

		#[test]
		fn single() {
			let mut buf = SegmentBuffer::new(smallvec![
				Segment::new(1, smallvec![1, 2]),
				Segment::new(1, smallvec![3, 4]),
				Segment::new(2, smallvec![5]),
			]);
			buf.compact();
			assert_eq!(
				buf,
				SegmentBuffer::new(smallvec![
					Segment::new(1, smallvec![1, 2, 3, 4]),
					Segment::new(2, smallvec![5])
				])
			);
		}
	}

	mod slice {
		use smallvec::smallvec;

		use crate::segment::{Segment, SegmentBuffer};

		#[test]
		fn first() {
			let input = SegmentBuffer::new(smallvec![Segment(1, smallvec![1, 2, 3, 4])]);
			assert_eq!(input.slice(0..=3), input);

			let input = SegmentBuffer::new(smallvec![Segment(1, smallvec![1, 2, 3, 4])]);
			assert_eq!(input.slice(0..4), input);
		}

		#[test]
		fn part() {
			let input = SegmentBuffer::new(smallvec![Segment(1, smallvec![1, 2, 3, 4])]);
			assert_eq!(
				input.slice(0..=2),
				SegmentBuffer::new(smallvec![Segment(1, smallvec![1, 2, 3])])
			);

			let input = SegmentBuffer::new(smallvec![Segment(1, smallvec![1, 2, 3, 4])]);
			assert_eq!(
				input.slice(1..=3),
				SegmentBuffer::new(smallvec![Segment(1, smallvec![2, 3, 4])])
			);
		}

		#[test]
		fn two() {
			let input = SegmentBuffer::new(smallvec![
				Segment(1, smallvec![1, 2, 3, 4]),
				Segment(1, smallvec![5, 6, 7, 8])
			]);
			assert_eq!(
				input.slice(2..=5),
				SegmentBuffer::new(smallvec![
					Segment(1, smallvec![3, 4]),
					Segment(1, smallvec![5, 6])
				])
			);

			let input = SegmentBuffer::new(smallvec![
				Segment(1, smallvec![1, 2, 3, 4]),
				Segment(1, smallvec![5, 6, 7, 8])
			]);
			assert_eq!(input.slice(0..=7), input);
		}
	}

	mod splice {
		use smallvec::smallvec;

		use crate::segment::{Segment, SegmentBuffer};

		#[test]
		fn insert_start() {
			let mut buf = SegmentBuffer::new(smallvec![Segment(1, smallvec![1, 2, 3, 4])]);
			buf.splice(
				0..0,
				Some(SegmentBuffer::new(smallvec![Segment(2, smallvec![5])])),
			);
			assert_eq!(
				buf,
				SegmentBuffer::new(smallvec![
					Segment(2, smallvec![5]),
					Segment(1, smallvec![1, 2, 3, 4])
				])
			)
		}

		#[test]
		fn insert_end() {
			let mut buf = SegmentBuffer::new(smallvec![Segment(1, smallvec![1, 2, 3, 4])]);
			buf.splice(
				buf.len..buf.len,
				Some(SegmentBuffer::new(smallvec![Segment(2, smallvec![5])])),
			);
			assert_eq!(
				buf,
				SegmentBuffer::new(smallvec![
					Segment(1, smallvec![1, 2, 3, 4]),
					Segment(2, smallvec![5]),
				])
			)
		}

		#[test]
		fn insert_middle() {
			let mut buf = SegmentBuffer::new(smallvec![Segment(1, smallvec![1, 2, 3, 4])]);
			buf.splice(
				2..2,
				Some(SegmentBuffer::new(smallvec![Segment(2, smallvec![5])])),
			);
			assert_eq!(
				buf,
				SegmentBuffer::new(smallvec![
					Segment(1, smallvec![1, 2]),
					Segment(2, smallvec![5]),
					Segment(1, smallvec![3, 4]),
				])
			)
		}

		#[test]
		fn replace_middle() {
			let mut buf = SegmentBuffer::new(smallvec![Segment(1, smallvec![1, 2, 3, 4])]);
			buf.splice(
				2..=2,
				Some(SegmentBuffer::new(smallvec![Segment(2, smallvec![5])])),
			);
			assert_eq!(
				buf,
				SegmentBuffer::new(smallvec![
					Segment(1, smallvec![1, 2]),
					Segment(2, smallvec![5]),
					Segment(1, smallvec![4]),
				])
			)
		}

		#[test]
		fn replace_middle_overlap() {
			let mut buf = SegmentBuffer::new(smallvec![
				Segment(1, smallvec![1, 2]),
				Segment(1, smallvec![3, 4])
			]);
			buf.splice(
				1..3,
				Some(SegmentBuffer::new(smallvec![Segment(2, smallvec![5])])),
			);
			assert_eq!(
				buf,
				SegmentBuffer::new(smallvec![
					Segment(1, smallvec![1]),
					Segment(2, smallvec![5]),
					Segment(1, smallvec![4]),
				])
			)
		}
	}
}
