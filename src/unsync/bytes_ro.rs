use std::iter::FromIterator;
use std::mem;
use std::ops::{
	Deref,
};

use BufMut;
use unsync::storage::Storage;
use unsync::UnBytesMut;
use unsync::UnBytesExt;

pub struct UnBytes(pub(super) Storage);

impl UnBytes {
	#[inline]
	pub fn with_capacity(len: usize) -> Self {
		UnBytes(Storage::with_capacity(len))
	}

	#[inline]
	pub fn new() -> Self {
		UnBytes(Storage::new())
	}

	pub fn from_static(data: &'static [u8]) -> Self {
		UnBytes(Storage::from_static(data))
	}

	pub fn len(&self) -> usize {
		self.0.len()
	}

	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}

	pub fn slice(&self, begin: usize, end: usize) -> Self {
		UnBytes(self.0.slice(begin, end))
	}

	pub fn slice_from(&self, begin: usize) -> Self {
		UnBytes(self.0.slice_from(begin))
	}

	pub fn slice_to(&self, end: usize) -> Self {
		UnBytes(self.0.slice_to(end))
	}

	pub fn split_off(&mut self, at: usize) -> Self {
		UnBytes(self.0.split_off(at))
	}

	pub fn split_to(&mut self, at: usize) -> Self {
		UnBytes(self.0.split_to(at))
	}

	pub fn truncate(&mut self, len: usize) {
		self.0.truncate(len)
	}

	pub fn advance(&mut self, skip: usize) {
		self.0.advance(skip)
	}

	pub fn clear(&mut self) {
		self.0.truncate(0);
	}

	pub fn extend_from_slice(&mut self, extend: &[u8]) {
		if extend.is_empty() {
			return;
		}

		let new_cap = self.len().checked_add(extend.len()).expect("capacity overflow");

		let result = match mem::replace(self, UnBytes::new()).try_mut() {
			Ok(mut bytes_mut) => {
				bytes_mut.extend_from_slice(extend);
				bytes_mut
			},
			Err(bytes) => {
				let mut bytes_mut = UnBytesMut::with_capacity(new_cap);
				bytes_mut.put_slice(&bytes);
				bytes_mut.put_slice(extend);
				bytes_mut
			}
		};

		mem::replace(self, result.freeze());
	}

	pub fn try_mut(mut self) -> Result<UnBytesMut, Self> {
		if self.0.upgrade() {
			Ok(UnBytesMut(self.0))
		} else {
			Err(self)
		}
	}

	pub fn try_ext(mut self) -> Result<UnBytesExt, Self> {
		if self.0.upgrade() {
			Ok(UnBytesExt(self.0))
		} else {
			Err(self)
		}
	}

	pub fn try_unsplit(&mut self, other: Self) -> Result<(), Self> {
		self.0.try_unsplit(other.0).map_err(UnBytes)
	}

	pub fn unsplit(&mut self, other: Self) {
		if let Err(other) = self.try_unsplit(other) {
			self.extend_from_slice(&other)
		}
	}

	pub fn try_into_vec(self) -> Result<(Vec<u8>, usize), Self> {
		self.0.try_into_vec().map_err(UnBytes)
	}
}

impl Clone for UnBytes {
	#[inline]
	fn clone(&self) -> Self {
		UnBytes(self.0.shallow_clone())
	}
}

impl From<UnBytesMut> for UnBytes {
	fn from(v: UnBytesMut) -> Self {
		v.freeze()
	}
}

impl From<UnBytesExt> for UnBytes {
	fn from(v: UnBytesExt) -> Self {
		v.freeze()
	}
}

impl Extend<u8> for UnBytes {
	fn extend<T>(&mut self, iter: T) where T: IntoIterator<Item = u8> {
		let iter = iter.into_iter();

		let (lower, _) = iter.size_hint();

		let mut bytes_mut = match mem::replace(self, UnBytes::new()).try_mut() {
			Ok(mut bytes_mut) => {
				bytes_mut.reserve(lower);
				bytes_mut
			},
			Err(bytes) => {
				let mut bytes_mut = UnBytesMut::with_capacity(bytes.len() + lower);
				bytes_mut.put_slice(&bytes);
				bytes_mut
			}
		};

		for item in iter {
			bytes_mut.0.append(item);
		}

		mem::replace(self, bytes_mut.freeze());
	}
}

impl<'a> Extend<&'a u8> for UnBytes {
	fn extend<T>(&mut self, iter: T) where T: IntoIterator<Item = &'a u8> {
		self.extend(iter.into_iter().map(|b| *b))
	}
}

impl_common!(UnBytes);
