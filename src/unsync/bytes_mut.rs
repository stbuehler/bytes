use std::fmt;
use std::iter::FromIterator;
use std::ops::{
	Deref,
	DerefMut,
};

use unsync::storage::Storage;
use unsync::UnBytes;
use unsync::UnBytesExt;

pub struct UnBytesMut(pub(super) Storage);

impl UnBytesMut {
	#[inline]
	pub fn with_capacity(len: usize) -> Self {
		UnBytesMut(Storage::with_capacity(len))
	}

	#[inline]
	pub fn new() -> Self {
		UnBytesMut(Storage::new())
	}

	pub fn len(&self) -> usize {
		self.0.len()
	}

	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}

	pub fn capacity(&self) -> usize {
		self.0.capacity()
	}

	pub fn freeze(self) -> UnBytes {
		UnBytes(self.0)
	}

	pub fn split_off(&mut self, at: usize) -> Self {
		UnBytesMut(self.0.split_off(at))
	}

	pub fn take(&mut self) -> Self {
		UnBytesMut(self.0.take())
	}

	pub fn split_to(&mut self, at: usize) -> Self {
		UnBytesMut(self.0.split_to(at))
	}

	pub fn truncate(&mut self, len: usize) {
		self.0.truncate(len);
	}

	pub fn advance(&mut self, skip: usize) {
		self.0.advance(skip);
	}

	pub fn clear(&mut self) {
		self.0.truncate(0);
	}

	pub unsafe fn set_len(&mut self, len: usize) {
		self.0.set_len(len);
	}

	/// Mutable slice of the (uninitialized) reserved data
	pub unsafe fn reserved(&mut self) -> &mut [u8] {
		self.0.reserved()
	}

	pub fn reserve(&mut self, additional: usize) {
		self.0.reserve(additional);
	}

	pub fn extend_from_slice(&mut self, extend: &[u8]) {
		self.0.extend(extend);
	}

	pub fn try_unsplit(&mut self, other: Self) -> Result<(), Self> {
		self.0.try_unsplit(other.0).map_err(UnBytesMut)
	}

	pub fn unsplit(&mut self, other: Self) {
		if let Err(other) = self.try_unsplit(other) {
			self.extend_from_slice(&other)
		}
	}

	pub fn try_into_vec(self) -> Result<(Vec<u8>, usize), Self> {
		self.0.try_into_vec().map_err(UnBytesMut)
	}
}

impl fmt::Write for UnBytesMut {
	#[inline]
	fn write_str(&mut self, s: &str) -> fmt::Result {
		if self.0.reserved_len() >= s.len() {
			self.0.put_slice(s.as_bytes());
			Ok(())
		} else {
			Err(fmt::Error)
		}
	}

	// optimized version
	#[inline]
	fn write_fmt(&mut self, args: fmt::Arguments) -> fmt::Result {
		fmt::write(self, args)
	}
}

impl From<UnBytesExt> for UnBytesMut {
	fn from(v: UnBytesExt) -> Self {
		UnBytesMut(v.0)
	}
}

impl From<UnBytes> for UnBytesMut {
	fn from(mut v: UnBytes) -> Self {
		if v.0.upgrade() {
			UnBytesMut(v.0)
		} else {
			UnBytesMut(Storage::from_data(&v))
		}
	}
}

impl_common_mut!(UnBytesMut);
