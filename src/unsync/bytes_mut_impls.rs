// trait implementations related to `bytes` crate

use BufMut;

use unsync::UnBytesMut;

impl BufMut for UnBytesMut {
	#[inline]
	fn remaining_mut(&self) -> usize {
		self.0.reserved_len()
	}

	#[inline]
	unsafe fn advance_mut(&mut self, cnt: usize) {
		self.0.inc_len(cnt)
	}

	#[inline]
	unsafe fn bytes_mut(&mut self) -> &mut [u8] {
		self.0.reserved()
	}

	#[inline]
	fn put_slice(&mut self, src: &[u8]) {
		self.0.put_slice(src)
	}

	#[inline]
	fn put_u8(&mut self, n: u8) {
		self.0.put_u8(n);
	}

	#[inline]
	fn put_i8(&mut self, n: i8) {
		self.0.put_u8(n as u8);
	}
}
