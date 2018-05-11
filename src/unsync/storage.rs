use std::cell::Cell;
use std::iter::FromIterator;
use std::mem;
use std::slice;
use std::ptr;
use std::marker::PhantomData;

struct Shared {
	ptr: *mut u8,
	cap: usize,
	ref_count: usize,
}

#[derive(Clone, Copy)]
struct KindShared {
	rc: ptr::NonNull<Shared>,
}

impl KindShared {
	fn release(mut self) {
		unsafe {
			{
				let shared = self.rc.as_mut();
				shared.ref_count -= 1;
				if shared.ref_count > 0 {
					// still alive
					return;
				}
				// drop vector
				if !shared.ptr.is_null() {
					drop(Vec::from_raw_parts(shared.ptr, 0, shared.cap));
				}
			}
			// drop shared box
			drop(Box::from_raw(self.rc.as_ptr()));
		}
	}

	fn acquire(mut self) {
		unsafe {
			let shared = self.rc.as_mut();
			shared.ref_count += 1;
		}
	}

	fn try_into_vec(mut self, storage: &Storage) -> Option<(Vec<u8>, usize)> {
		let result = {
			let shared = unsafe { self.rc.as_mut() };
			if 1 != shared.ref_count {
				return None;
			}

			let ptr = shared.ptr;
			let cap = shared.cap;
			let offset = storage.ptr as usize - ptr as usize;

			storage.kind.set_empty();

			(unsafe { Vec::from_raw_parts(ptr, offset + storage.len, cap) }, offset)
		};
		// drop only the box; we're reusing the vector
		drop(unsafe { Box::from_raw(self.rc.as_ptr()) });
		Some(result)
	}

	// ref_count 1 - when we need to store a Vec with large capacity
	fn new1(mut v: Vec<u8>) -> Self {
		let ptr = v.as_mut_ptr();
		let cap = v.capacity();
		mem::forget(v);
		let shared = Box::new(Shared {
			ptr,
			cap,
			ref_count: 1,
		});
		KindShared {
			rc: unsafe { ptr::NonNull::new_unchecked(Box::into_raw(shared)) },
		}
	}

	// ref_count 2 - we don't create shared data without reason
	fn new2(mut v: Vec<u8>) -> Self {
		let ptr = v.as_mut_ptr();
		let cap = v.capacity();
		mem::forget(v);
		let shared = Box::new(Shared {
			ptr,
			cap,
			ref_count: 2,
		});
		KindShared {
			rc: unsafe { ptr::NonNull::new_unchecked(Box::into_raw(shared)) },
		}
	}
}

#[derive(Clone, Copy)]
struct KindInline {
	len: u8,
	data: usize,
}

const MAX_KIND_VEC_CAPACITY_BITS: usize = 8*mem::size_of::<usize>() - 2;

#[derive(Clone, Copy)]
struct KindVec {
	// offset of `ptr` in vector
	offset: usize,
}

impl KindVec {
	fn rebuild_vec(self, storage: &Storage) -> Vec<u8> {
		let ptr = (storage.ptr as usize - self.offset) as *mut u8;
		let length = storage.len + self.offset;
		let capacity = storage.cap + self.offset;
		storage.kind.set_empty();
		unsafe { Vec::from_raw_parts(ptr, length, capacity) }
	}

	fn store(self, mut v: Vec<u8>) -> Storage {
		let mut kind = encode(self);
		let ptr = (v.as_mut_ptr() as usize + self.offset) as *mut u8;
		let len = v.len() - self.offset;
		let cap = v.capacity() - self.offset;

		if 0 != v.capacity() >> MAX_KIND_VEC_CAPACITY_BITS {
			// cannot store all possible offsets, convert to `Shared`
			kind = encode(KindShared::new1(v))
		} else {
			mem::forget(v);
		}
		Storage {
			kind: kind,
			ptr,
			len,
			cap,
			_drop_marker: PhantomData,
		}
	}
}

#[derive(Clone, Copy)]
enum Kind {
	Static,
	Vec(KindVec),
	Inline(KindInline),
	Shared(KindShared),
}

#[inline(always)]
fn encode<K: Into<Kind>>(k: K) -> KindTag {
	KindTag(Cell::new(k.into().encode()))
}

impl Kind {
	#[inline(always)]
	fn encode(self) -> usize {
		match self {
			Kind::Static => 0,
			Kind::Vec(KindVec{offset}) => (offset << 2) | 0x1,
			Kind::Inline(KindInline{len, data}) => (data & !0xff) | (len << 2) as usize | 0x2,
			Kind::Shared(KindShared{rc}) => rc.as_ptr() as usize,
		}
	}
}

impl From<KindShared> for Kind {
	#[inline(always)]
	fn from(v: KindShared) -> Kind {
		Kind::Shared(v)
	}
}

impl From<KindInline> for Kind {
	#[inline(always)]
	fn from(v: KindInline) -> Kind {
		Kind::Inline(v)
	}
}

impl From<KindVec> for Kind {
	#[inline(always)]
	fn from(v: KindVec) -> Kind {
		Kind::Vec(v)
	}
}

impl From<()> for Kind {
	#[inline(always)]
	fn from(_v: ()) -> Kind {
		Kind::Static
	}
}

// bit string suffix:
// - ..01: plain vector; other bits encode offset in vector
// - ..10: inline data, other bits encode length of data (normal Storage fields
//   invalid)
// - all 0: static data
// - otherwise pointer to `Shared`
//
// The tag has interior mutability; but usage below must not invalidate any data
// that might have been returned by getters.
#[repr(C)]
struct KindTag(Cell<usize>);

impl KindTag {
	#[inline(always)]
	fn decode(&self) -> Kind {
		let t = self.0.get();
		if 0 == t {
			Kind::Static
		} else if 0 != t & 0x1 {
			Kind::Vec(KindVec {
				offset: t >> 2,
			})
		} else if 0 != t & 0x2 {
			Kind::Inline(KindInline {
				len: (t as u8) >> 2,
				data: t & !0xff,
			})
		} else {
			Kind::Shared(KindShared {
				rc: unsafe { ptr::NonNull::new_unchecked(t as *mut Shared) },
			})
		}
	}

	#[inline(always)]
	fn is_inline(&self) -> bool {
		0 != self.0.get() & 0x2
	}

	#[inline(always)]
	fn decode_inline_len(&self) -> Option<usize> {
		if self.is_inline() {
			Some((self.0.get() & 0xff) >> 2)
		} else {
			None
		}
	}

	#[inline(always)]
	fn is_inline_empty(&self) -> Option<bool> {
		if self.is_inline() {
			Some(self.0.get() & 0xff == 0x02)
		} else {
			None
		}
	}

	#[inline(always)]
	fn is_static(&self) -> bool {
		self.0.get() == 0
	}

	#[inline(always)]
	fn is_vec(&self) -> bool {
		self.0.get() & 0x1 == 0x1
	}

	#[inline(always)]
	fn advance_vec(&self, skip: usize) {
		debug_assert!(self.is_vec());
		self.0.set(self.0.get() + skip << 2);
	}

	#[inline(always)]
	fn set(&self, kind: Kind) {
		self.0.set(kind.encode())
	}

	// set empty inline kind
	#[inline(always)]
	fn set_empty(&self) {
		self.0.set((self.0.get() & !0xff) | 0x02)
	}

	#[inline(always)]
	fn set_inline_len(&self, len: usize) {
		debug_assert!(self.is_inline());
		debug_assert!(len <= INLINE_CAPACITY);
		self.0.set((self.0.get() & !0xfc) | (len << 2));
	}
}

// Storage offers a shared mutable interface to [u8]; a wrapping type needs to
// ensure mutables slices don't overlap with (possible shared) immutable slices.
//
// Storage backed by static (immutable) slices must never be used for a mutable
// wrapper (requesting mutable access would otherwise require on-demand
// allocation).
//
// All methods are supposed to not have hidden complexity costs, i.e. allocation
// only happens in methods which obviously can't avoid it.
//
// The interface is mostly "safe" out of convenience - the wrapping types need
// to make sure the calls are actually safe.

#[cfg(target_endian = "little")]
#[repr(C)]
pub(super) struct Storage {
	kind: KindTag,
	ptr: *mut u8,
	len: usize,
	cap: usize,
	_drop_marker: PhantomData<(Vec<u8>, Shared)>,
}

#[cfg(target_endian = "big")]
#[repr(C)]
pub(super) struct Storage {
	ptr: *mut u8,
	len: usize,
	cap: usize,
	_drop_marker: PhantomData<(Vec<u8>, Shared)>,
	kind: KindTag,
}

/// should be 4*size_of::<usize>() - 1, i.e. 15 on 32-bit and 31 on 64-bit
pub(super) const INLINE_CAPACITY: usize = mem::size_of::<Storage>() - 1;

impl Storage {
	/// create empty storage
	#[inline]
	pub fn new() -> Self {
		let storage: Storage = unsafe { mem::uninitialized() };
		storage.kind.set_empty();
		storage
	}

	// drop current data, release all refs
	pub fn set_empty(&mut self) {
		match self.kind.decode() {
			Kind::Static | Kind::Inline(_) => (),
			Kind::Vec(v) => {
				drop(v.rebuild_vec(self));
			},
			Kind::Shared(s) => {
				s.release();
				self.kind.set_empty();
			},
		}
	}

	/// create storage with static backed data (not mutable, doesn't "own" the
	/// data)
	#[inline]
	pub fn from_static(data: &'static [u8]) -> Self {
		Storage {
			kind: encode(()),
			ptr: data.as_ptr() as *mut u8,
			len: data.len(),
			cap: data.len(),
			_drop_marker: PhantomData,
		}
	}

	/// create storage from Vec
	#[inline]
	pub fn from_vec(data: Vec<u8>, offset: usize) -> Self {
		assert!(offset <= data.len());
		if data.len() - offset <= INLINE_CAPACITY {
			Storage::from_data_inline(&data[offset..])
		} else {
			KindVec{offset}.store(data)
		}
	}

	/// allocate owned (mutable) storage with vector backend (never uses inline
	/// representation)
	fn alloc_vec(capacity: usize) -> Self {
		let data = Vec::with_capacity(capacity);
		KindVec{offset: 0}.store(data)
	}

	/// allocate owned (mutable) storage with vector or inline backend
	fn with_capacity_and_data(capacity: usize, data: &[u8]) -> Self {
		debug_assert!(capacity >= data.len());
		if capacity <= INLINE_CAPACITY {
			Storage::from_data_inline(data)
		} else {
			let mut vec = Vec::with_capacity(capacity);
			vec.extend_from_slice(data);
			KindVec{offset: 0}.store(vec)
		}
	}

	/// allocate owned (mutable) storage
	#[inline]
	pub fn with_capacity(capacity: usize) -> Self {
		if capacity <= INLINE_CAPACITY {
			Storage::new()
		} else {
			Storage::alloc_vec(capacity)
		}
	}

	/// use inline allocation to create (owned, mutable) storage from data
	#[inline]
	fn from_data_inline(data: &[u8]) -> Self {
		debug_assert!(data.len() <= INLINE_CAPACITY);
		let storage: Storage = Storage::new();
		storage.kind.set_inline_len(data.len());
		unsafe {
			ptr::copy(data.as_ptr(), storage.inline_ptr(), data.len());
		}
		storage
	}

	/// allocate (owned, mutable) storage for data
	pub fn from_data(data: &[u8]) -> Self {
		if data.len() <= INLINE_CAPACITY {
			Storage::from_data_inline(data)
		} else {
			let mut storage = Storage::alloc_vec(data.len());
			unsafe {
				ptr::copy(data.as_ptr(), storage.ptr, data.len());
			}
			storage.len = data.len();
			storage
		}
	}

	/// length of data
	#[inline]
	pub fn len(&self) -> usize {
		self.kind.decode_inline_len().unwrap_or(self.len)
	}

	/// whether storage is empty
	#[inline]
	pub fn is_empty(&self) -> bool {
		self.kind.is_inline_empty().unwrap_or(0 == self.len)
	}

	/// storage capacity
	#[inline]
	pub fn capacity(&self) -> usize {
		if self.kind.is_inline() {
			INLINE_CAPACITY
		} else {
			self.cap
		}
	}

	/// for inline storage pointer to first storage byte
	#[inline]
	fn inline_ptr(&self) -> *mut u8 {
		let base = self as *const Self as *mut u8;

		#[cfg(target_endian = "little")]
		let result = unsafe { base.offset(1) };

		#[cfg(target_endian = "big")]
		let result = base;

		result
	}

	#[inline]
	fn raw_data(&self) -> (*mut u8, usize) {
		if let Some(len) = self.kind.decode_inline_len() {
			(self.inline_ptr(), len)
		} else {
			(self.ptr, self.len)
		}
	}

	/// returns a slice of length `self.len()`
	#[inline]
	pub fn data(&self) -> &[u8] {
		let (ptr, len) = self.raw_data();
		unsafe { slice::from_raw_parts(ptr, len) }
	}

	/// returns a mutable slice of length `self.len()`
	#[inline]
	pub fn data_mut(&mut self) -> &mut [u8] {
		debug_assert!(!self.kind.is_static(), "can't get mutable reference to static data");
		let (ptr, len) = self.raw_data();
		unsafe { slice::from_raw_parts_mut(ptr, len) }
	}

	/// truncate data length; noop if `len >= self.len()`
	pub fn truncate(&mut self, len: usize) {
		if let Some(cur_len) = self.kind.decode_inline_len() {
			if len < cur_len {
				self.kind.set_inline_len(len);
			}
		} else if len < self.len {
			self.len = len;
		}
	}

	/// truncate length and (shared) capacity
	///
	/// when using shared storage this reduces the capacity so another reference
	/// can use mutable access for the trailing part.
	///
	/// for inline storage the capacity can't be reduce, and there is no need
	/// to: that storage is owned uniquely.
	///
	/// Noop if `len >= self.capacity()`, and doesn't change length if `len >=
	/// self.len()`.
	fn truncate_capacity(&mut self, len: usize) {
		match self.kind.decode() {
			Kind::Static | Kind::Vec(_) => {
				// completely immutable or owned buffer: don't touch capacity
				if len < self.len {
					self.len = len;
				}
			},
			Kind::Inline(i) => {
				// (owned) inline buffer: don't (and can't) touch capaciy
				if len < i.len as usize {
					self.kind.set_inline_len(len);
				}
			},
			Kind::Shared(_) => {
				if len < self.len {
					self.len = len;
					self.cap = len;
				} else if len < self.cap {
					self.cap = len;
				}
			},
		}
	}

	/// increments length (after writing some data to `self.reserved()`)
	///
	/// can't increase beyond capacity, doesn't allocate.
	#[inline]
	pub fn inc_len(&mut self, len: usize) {
		if let Some(cur_len) = self.kind.decode_inline_len() {
			assert!(len <= INLINE_CAPACITY - cur_len);
			let new_len = cur_len + len;
			self.kind.set_inline_len(new_len);
		} else {
			assert!(len <= self.cap - self.len);
			self.len += len;
		}
	}

	/// set length (usually after writing some data to `self.reserved()`, but can also truncate)
	///
	/// can't increase beyond capacity, doesn't allocate.
	///
	/// mainly for compat with `bytes` crate, prefer `self.inc_len(..)`.
	#[inline]
	pub unsafe fn set_len(&mut self, len: usize) {
		if self.kind.is_inline() {
			assert!(len <= INLINE_CAPACITY);
			self.kind.set_inline_len(len);
		} else {
			assert!(len <= self.cap);
			self.len = len;
		}
	}

	fn reserve_from_vec(mut data: Vec<u8>, offset: usize, additional: usize) -> Storage {
		let content_len = data.len() - offset;
		let required = content_len + additional;
		if data.capacity() < required {
			if offset > 0 && content_len > 0 {
				// move to front
				let begin = data.as_mut_ptr();
				let ptr = (begin as usize + offset) as *const u8;
				unsafe {
					ptr::copy(ptr, begin, content_len);
				};
			}
			unsafe {
				data.set_len(content_len);
			}
			// now offset is 0
			KindVec{offset: 0}.store(data)
		} else if offset < 32 {
			data.reserve(additional);
			KindVec{offset}.store(data)
		} else {
			Storage::with_capacity_and_data(required, &data[offset..])
		}
	}

	/// returns a mutable slice of length `self.capacity() - self.len()`, i.e.
	/// the bytes that are not written yet.
	///
	/// after writing (some of) those bytes you should call `self.inc_len(..)`.
	#[inline]
	pub fn reserved(&mut self) -> &mut [u8] {
		debug_assert!(!self.kind.is_static(), "can't get mutable reference to static data");
		if let Some(len) = self.kind.decode_inline_len() {
			let begin = (self.inline_ptr() as usize + len) as *mut u8;
			unsafe { slice::from_raw_parts_mut(begin, INLINE_CAPACITY) }
		} else {
			let begin = (self.ptr as usize + self.len) as *mut u8;
			unsafe { slice::from_raw_parts_mut(begin, self.cap) }
		}
	}

	#[inline]
	pub fn reserved_len(&self) -> usize {
		debug_assert!(!self.kind.is_static(), "can't get mutable reference to static data");
		if let Some(len) = self.kind.decode_inline_len() {
			INLINE_CAPACITY - len
		} else {
			self.cap - self.len
		}
	}

	/// makes sure the capacity is big enough to write `additional` bytes
	///
	/// storage needs to mutable already, panics otherwise.
	///
	/// afterwards `self.reserved().len() >= additional`.
	pub fn reserve(&mut self, additional: usize) {
		if 0 == additional {
			return;
		}
		match self.kind.decode() {
			Kind::Static => {
				panic!("can't reserve on static data");
			},
			Kind::Vec(v) => {
				let new_capacity = self.len + additional;
				if new_capacity > self.cap {
					let data = v.rebuild_vec(self);
					*self = Storage::reserve_from_vec(data, v.offset, additional);
				}
			}
			Kind::Inline(i) => {
				let new_capacity = (i.len as usize) + additional;
				if new_capacity > INLINE_CAPACITY {
					let data = unsafe { slice::from_raw_parts(self.inline_ptr(), i.len as usize) };
					let storage = Storage::with_capacity_and_data(new_capacity, data);
					*self = storage;
				}
			},
			Kind::Shared(s) => {
				let new_capacity = self.len + additional;
				if new_capacity > self.cap {
					if let Some((data, offset)) = s.try_into_vec(self) {
						*self = Storage::reserve_from_vec(data, offset, additional);
					} else {
						let data = unsafe { slice::from_raw_parts(self.ptr, self.len) };
						let storage = Storage::with_capacity_and_data(new_capacity, data);
						*self = storage;
					}
				}
			},
		}
	}

	/// try to merge to storage references if they point to connected slices
	///
	/// also succeeds if one of them is empty
	pub fn try_unsplit(&mut self, other: Self) -> Result<(), Self> {
		if other.is_empty() {
			return Ok(());
		}

		if let Some(is_empty) = self.kind.is_inline_empty() {
			if is_empty {
				// empty, just replace
				*self = other;
				return Ok(());
			}
			return Err(other);
		}

		// self not inline:

		if 0 == self.len {
			// empty, just replace
			*self = other;
			return Ok(());
		}

		let self_end = (self.ptr as usize + self.len) as *mut u8;

		let other_start = if other.kind.is_inline() {
			// inline, can't merge (other and self are not empty, checked above)
			return Err(other);
		} else {
			other.ptr
		};

		// vectors can't be shared, and can't be merged. if they are contiguos
		// that's just bad luck...
		if self_end == other_start && !self.kind.is_vec() && !other.kind.is_vec() {
			// merge
			self.cap = self.len + other.cap;
			self.len += other.cap;
			Ok(())
		} else {
			Err(other)
		}
	}

	/// advance slice; panics if `skip > self.len()`
	///
	/// doesn't move data to inline storage if there are still some bytes used
	/// from a shared storage, but resets to an empty (inline) storage if `skip
	/// == self.cap()`.
	///
	/// Noop if `skip == 0`.
	pub fn advance(&mut self, skip: usize) {
		if let Some(len) = self.kind.decode_inline_len() {
			assert!(skip <= len);
			let new_len = len - skip;
			let data = self.inline_ptr();
			unsafe {
				ptr::copy((data as usize + skip) as *const u8, data as *mut u8, new_len);
			}
			self.kind.set_inline_len(new_len);
		} else {
			assert!(skip <= self.len);
			if skip == self.cap {
				// now empty
				self.set_empty();
				return;
			}

			if self.kind.is_vec() {
				self.kind.advance_vec(skip);
			}

			self.ptr = (self.ptr as usize + skip) as *mut u8;
			self.len -= skip;
			self.cap -= skip;
		}
	}

	/// splits of a new reference; the new (returned) one gets all data from
	/// position `at` (including the reserved data)
	///
	/// self gets truncated (including the capacity) to `at`.
	///
	/// panics if `at > self.len()`
	#[inline]
	pub fn split_off(&mut self, at: usize) -> Self {
		let mut tail = self.shallow_clone();
		tail.advance(at); // this panics if `at > self.len()`
		self.truncate_capacity(at);
		tail
	}

	/// returns current data, only leaves reserved space (same as
	/// `self.split_to(self.len()))`).
	#[inline]
	pub fn take(&mut self) -> Self {
		let len = self.len();
		self.split_to(len)
	}

	/// mirror operation to `split_off`: return the initial slice, and makes
	/// `self` the trailing part.
	///
	/// panics if `at > self.len()`
	#[inline]
	pub fn split_to(&mut self, at: usize) -> Self {
		let mut tail = self.shallow_clone();
		self.advance(at); // this panics if `at > self.len()`
		tail.truncate_capacity(at);
		tail
	}

	/// upgrade capacity to maximum if unique owner of storage
	/// returns true if unique owner of storage
	pub fn upgrade(&mut self) -> bool {
		match self.kind.decode() {
			Kind::Static => false,
			Kind::Shared(s) => {
				let shared = unsafe { s.rc.as_ref() };
				if 1 == shared.ref_count {
					let offset = self.ptr as usize - shared.ptr as usize;
					self.cap = shared.cap - offset;
					true
				} else {
					false
				}
			},
			Kind::Inline(_) | Kind::Vec(_) => true,
		}
	}

	/// try to get backing buffer of storage including the current offset into
	/// it
	///
	/// (also applies current length to vector; internally the vectors length if
	/// at always at full capacity)
	///
	/// Fails for inlined/static storage or not uniquely owned storage.
	pub fn try_into_vec(mut self) -> Result<(Vec<u8>, usize), Self> {
		match self.kind.decode() {
			Kind::Static | Kind::Inline(_) => Err(self),
			Kind::Vec(v) => {
				Ok((v.rebuild_vec(&mut self), v.offset))
			},
			Kind::Shared(s) => {
				s.try_into_vec(&mut self).ok_or(self)
			}
		}
	}

	/// extend mutable storage (might allocate)
	#[inline]
	pub fn extend(&mut self, data: &[u8]) {
		self.reserve(data.len());
		self.reserved()[..data.len()].copy_from_slice(data);
		self.inc_len(data.len())
	}

	#[inline]
	pub fn append(&mut self, data: u8) {
		self.reserve(1);
		self.reserved()[0] = data;
		self.inc_len(1)
	}

	/// never allocates, panics if not enough space
	#[inline]
	pub fn put_slice(&mut self, data: &[u8]) {
		self.reserved()[..data.len()].copy_from_slice(data);
		self.inc_len(data.len())
	}

	#[inline]
	pub fn put_u8(&mut self, data: u8) {
		self.reserved()[0] = data;
		self.inc_len(1)
	}

	/// self[begin..][..len]
	///
	/// converts to inline storage if len is small enough
	///
	/// panics if indices are out of range
	fn slice_len(&self, begin: usize, len: usize) -> Self {
		if 0 == len {
			Storage::new()
		} else if len <= INLINE_CAPACITY {
			Storage::from_data_inline(&self.data()[begin..][..len])
		} else {
			assert!(!self.kind.is_inline()); // wouldn't be big enough
			assert!(begin < self.len && begin + len < self.len);
			let mut new = self.shallow_clone();
			debug_assert!(!self.kind.is_vec()); // just shared; can't own it
			new.ptr = (new.ptr as usize + begin) as *mut u8;
			new.len = len;
			new.cap = len;
			new
		}
	}

	/// self[begin..end]
	///
	/// converts to inline storage if `end - begin` is small enough
	///
	/// panics if `begin > end || end > self.len()`
	#[inline]
	pub fn slice(&self, begin: usize, end: usize) -> Self {
		assert!(begin <= end);
		self.slice_len(begin, end - begin)
	}

	/// self[begin..]
	///
	/// converts to inline storage if resulting length is small enough
	///
	/// panics if `begin > self.len()`
	#[inline]
	pub fn slice_from(&self, begin: usize) -> Self {
		let len = self.len();
		// this is gonna get checked anyway: assert!(begin <= len);
		self.slice_len(begin, len - begin)
	}

	/// self[..end]
	///
	/// converts to inline storage if end is small enough
	///
	/// panics if `end > self.len()`
	#[inline]
	pub fn slice_to(&self, end: usize) -> Self {
		self.slice_len(0, end)
	}

	pub fn shallow_clone(&self) -> Self {
		match self.kind.decode() {
			Kind::Static | Kind::Inline(_) => (),
			Kind::Shared(s) => {
				s.acquire();
			}
			Kind::Vec(v) => {
				// upgrade to shared storage
				let data = v.rebuild_vec(self);
				let s = KindShared::new2(data);
				self.kind.set(s.into());
			},
		}
		unsafe { ptr::read(self) }
	}
}

impl Drop for Storage {
	fn drop(&mut self) {
		match self.kind.decode() {
			Kind::Static | Kind::Inline(_) => (),
			Kind::Vec(v) => {
				drop(v.rebuild_vec(self));
			},
			Kind::Shared(s) => {
				s.release();
			}
		}
	}
}

impl FromIterator<u8> for Storage {
	fn from_iter<T: IntoIterator<Item = u8>>(into_iter: T) -> Self {
		let iter = into_iter.into_iter();
		let (min, maybe_max) = iter.size_hint();

		let mut out = Storage::with_capacity(maybe_max.unwrap_or(min));

		for i in iter {
			out.append(i);
		}

		out
	}
}
