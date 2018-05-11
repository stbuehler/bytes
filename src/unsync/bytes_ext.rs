use std::fmt;
use std::iter::FromIterator;
use std::ops::{
	Deref,
	DerefMut,
};

use unsync::storage::Storage;
use unsync::UnBytes;
use unsync::UnBytesMut;

pub struct UnBytesExt(pub(super) Storage);

impl UnBytesExt {
	#[inline]
	pub fn with_capacity(len: usize) -> Self {
		UnBytesExt(Storage::with_capacity(len))
	}

	#[inline]
	pub fn new() -> Self {
		UnBytesExt(Storage::new())
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
		UnBytesExt(self.0.split_off(at))
	}

	pub fn take(&mut self) -> Self {
		UnBytesExt(self.0.take())
	}

	pub fn split_to(&mut self, at: usize) -> Self {
		UnBytesExt(self.0.split_to(at))
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
		self.0.try_unsplit(other.0).map_err(UnBytesExt)
	}

	pub fn unsplit(&mut self, other: Self) {
		if let Err(other) = self.try_unsplit(other) {
			self.extend_from_slice(&other)
		}
	}

	pub fn try_into_vec(self) -> Result<(Vec<u8>, usize), Self> {
		self.0.try_into_vec().map_err(UnBytesExt)
	}
}

impl fmt::Write for UnBytesExt {
	#[inline]
	fn write_str(&mut self, s: &str) -> fmt::Result {
		self.0.extend(s.as_bytes());
		Ok(())
	}

	// optimized version
	#[inline]
	fn write_fmt(&mut self, args: fmt::Arguments) -> fmt::Result {
		fmt::write(self, args)
	}
}

impl From<UnBytes> for UnBytesExt {
	fn from(mut v: UnBytes) -> Self {
		if v.0.upgrade() {
			UnBytesExt(v.0)
		} else {
			UnBytesExt(Storage::from_data(&v))
		}
	}
}

impl From<UnBytesMut> for UnBytesExt {
	fn from(v: UnBytesMut) -> Self {
		UnBytesExt(v.0)
	}
}

impl_common_mut!(UnBytesExt);
