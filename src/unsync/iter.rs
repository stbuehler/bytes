use std::iter::FusedIterator;
use std::marker::PhantomData;

#[derive(Clone)]
pub struct SliceIter<'a> {
	ptr: *const u8,
	end: *const u8,
	_marker: PhantomData<&'a u8>
}

impl<'a> SliceIter<'a> {
	pub fn new(data: &'a [u8]) -> Self {
		let ptr = data.as_ptr();
		SliceIter {
			ptr: ptr,
			end: unsafe { ptr.offset(data.len() as isize) },
			_marker: PhantomData,
		}
	}
}

impl<'a> Iterator for SliceIter<'a> {
	type Item = u8;

	#[inline]
	fn next(&mut self) -> Option<u8> {
		unsafe {
			if self.ptr == self.end {
				return None;
			}
			let v: u8 = *self.ptr;
			self.ptr = self.ptr.offset(1);
			Some(v)
		}
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let rem = self.len();
		(rem, Some(rem))
	}

	fn count(self) -> usize {
		self.len()
	}
}

impl<'a> DoubleEndedIterator for SliceIter<'a> {
	#[inline]
	fn next_back(&mut self) -> Option<u8> {
		unsafe {
			if self.ptr == self.end {
				return None;
			}
			self.end = self.end.offset(-1);
			let v: u8 = *self.end;
			Some(v)
		}
	}
}

impl<'a> ExactSizeIterator for SliceIter<'a> {
	fn len(&self) -> usize {
		self.end as usize - self.ptr as usize
	}
}

impl<'a> FusedIterator for SliceIter<'a> {
}

pub struct Iter<T>
where
	T: AsRef<[u8]>,
{
	pos: usize,
	len: usize,
	data: T,
}

impl<T> Iter<T>
where
	T: AsRef<[u8]>,
{
	pub(crate) fn new(data: T) -> Self
	{
		let len = data.as_ref().len();
		Iter {
			pos: 0,
			len,
			data,
		}
	}
}

impl<T> Iterator for Iter<T>
where
	T: AsRef<[u8]>,
{
	type Item = u8;

	#[inline]
	fn next(&mut self) -> Option<u8> {
		if self.pos >= self.len {
			return None;
		}
		let s = self.data.as_ref();
		let v = s[self.pos];
		self.pos += 1;
		Some(v)
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let rem = self.len();
		(rem, Some(rem))
	}

	fn count(self) -> usize {
		self.len()
	}
}

impl<T> DoubleEndedIterator for Iter<T>
where
	T: AsRef<[u8]>,
{
	#[inline]
	fn next_back(&mut self) -> Option<u8> {
		if self.pos >= self.len {
			return None;
		}
		let s = self.data.as_ref();
		self.len -= 1;
		let v = s[self.len];
		Some(v)
	}
}

impl<T> ExactSizeIterator for Iter<T>
where
	T: AsRef<[u8]>,
{
	fn len(&self) -> usize {
		self.data.as_ref().len() - self.pos
	}
}

impl<T> FusedIterator for Iter<T>
where
	T: AsRef<[u8]>,
{
}
