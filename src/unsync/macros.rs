use Buf;
use BufMut;
use IntoBuf;

use unsync::UnBytesMut;
use unsync::storage::INLINE_CAPACITY;

pub(crate) fn from_buf<T>(buf: T) -> UnBytesMut
	where T: IntoBuf,
{
	let buf = buf.into_buf();
	if buf.remaining() <= INLINE_CAPACITY {
		let mut ret = UnBytesMut::new();
		ret.put(buf);
		ret
	} else {
		let mut ret = Vec::with_capacity(buf.remaining());
		ret.put(buf);
		ret.into()
	}
}

macro_rules! impl_cmp {
	($ty:ident: $($({$tag:tt})* $other:ty,)*) => {
		impl_cmp!(() {single} $ty: ::unsync::UnBytes);
		impl_cmp!(() {single} $ty: ::unsync::UnBytesMut);
		impl_cmp!(() {single} $ty: ::unsync::UnBytesExt);

		$(
			impl_cmp!(() $({$tag})* $ty: $other);
		)*

		impl<'a, T> PartialEq<&'a T> for $ty
		where
			T: ?Sized,
			$ty: PartialEq<T>,
		{
			#[inline]
			fn eq(&self, other: &&'a T) -> bool {
				*self == **other
			}
		}

		impl<'a, T> PartialOrd<&'a T> for $ty
		where
			T: ?Sized,
			$ty: PartialOrd<T>,
		{
			#[inline]
			fn partial_cmp(&self, other: &&'a T) -> Option<::std::cmp::Ordering> {
				self.partial_cmp(*other)
			}
		}
	};
	(() $ty:ident: $other:ty) => {
		impl PartialEq<$other> for $ty {
			#[inline]
			fn eq(&self, other: &$other) -> bool {
				(self.as_ref() as &[u8]) == (other.as_ref() as &[u8])
			}
		}

		impl PartialEq<$ty> for $other {
			#[inline]
			fn eq(&self, other: &$ty) -> bool {
				(self.as_ref() as &[u8]) == (other.as_ref() as &[u8])
			}
		}

		impl PartialOrd<$other> for $ty {
			#[inline]
			fn partial_cmp(&self, other: &$other) -> Option<::std::cmp::Ordering> {
				(self.as_ref() as &[u8]).partial_cmp(other.as_ref() as &[u8])
			}
		}

		impl PartialOrd<$ty> for $other {
			#[inline]
			fn partial_cmp(&self, other: &$ty) -> Option<::std::cmp::Ordering> {
				(self.as_ref() as &[u8]).partial_cmp(other.as_ref() as &[u8])
			}
		}
	};
	(() {ref} $ty:ident: $other:ty) => {
		impl<'a> PartialEq<$ty> for &'a $other {
			#[inline]
			fn eq(&self, other: &$ty) -> bool {
				(self.as_ref() as &[u8]) == (other.as_ref() as &[u8])
			}
		}

		impl<'a> PartialOrd<$ty> for &'a $other {
			#[inline]
			fn partial_cmp(&self, other: &$ty) -> Option<::std::cmp::Ordering> {
				(self.as_ref() as &[u8]).partial_cmp(other.as_ref() as &[u8])
			}
		}
	};
	(() {single} $ty:ident: $other:ty) => {
		impl PartialEq<$other> for $ty {
			#[inline]
			fn eq(&self, other: &$other) -> bool {
				(self.as_ref() as &[u8]) == (other.as_ref() as &[u8])
			}
		}

		impl PartialOrd<$other> for $ty {
			#[inline]
			fn partial_cmp(&self, other: &$other) -> Option<::std::cmp::Ordering> {
				(self.as_ref() as &[u8]).partial_cmp(other.as_ref() as &[u8])
			}
		}
	};
}

macro_rules! impl_common {
	($ty:ident) => {
		impl ::std::fmt::Debug for $ty {
			fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
				self.as_ref().fmt(f)
			}
		}

		impl AsRef<[u8]> for $ty {
			#[inline]
			fn as_ref(&self) -> &[u8] {
				self.0.data()
			}
		}

		impl ::std::borrow::Borrow<[u8]> for $ty {
			#[inline]
			fn borrow(&self) -> &[u8] {
				self.as_ref()
			}
		}

		impl Deref for $ty {
			type Target = [u8];

			#[inline]
			fn deref(&self) -> &Self::Target {
				self.0.data()
			}
		}

		impl From<Vec<u8>> for $ty {
			fn from(v: Vec<u8>) -> Self {
				$ty(Storage::from_vec(v, 0))
			}
		}

		impl From<String> for $ty {
			fn from(v: String) -> Self {
				$ty(Storage::from_vec(v.into(), 0))
			}
		}

		impl From<::Bytes> for $ty {
			fn from(v: ::Bytes) -> Self {
				$ty(Storage::from_data(&v))
			}
		}

		impl From<$ty> for ::Bytes {
			fn from(v: $ty) -> Self {
				::BytesMut::from(v).into()
			}
		}

		impl From<::BytesMut> for $ty {
			fn from(v: ::BytesMut) -> Self {
				$ty(Storage::from_data(&v))
			}
		}

		impl From<$ty> for ::BytesMut {
			fn from(v: $ty) -> Self {
				match v.try_into_vec() {
					Ok((v, pos)) => {
						let mut b = ::BytesMut::from(v);
						b.advance(pos);
						b
					},
					Err(v) => {
						::BytesMut::from(v.as_ref())
					}
				}
			}
		}

		impl<'a> From<&'a [u8]> for $ty {
			fn from(v: &'a [u8]) -> Self {
				$ty(Storage::from_data(v))
			}
		}

		impl<'a> From<&'a str> for $ty {
			fn from(v: &'a str) -> Self {
				$ty(Storage::from_data(v.as_bytes()))
			}
		}

		impl FromIterator<u8> for $ty {
			fn from_iter<T: IntoIterator<Item = u8>>(into_iter: T) -> Self {
				$ty(Storage::from_iter(into_iter))
			}
		}

		impl ::Buf for $ty {
			#[inline]
			fn remaining(&self) -> usize {
				self.0.len()
			}

			#[inline]
			fn bytes(&self) -> &[u8] {
				self.0.data()
			}

			#[inline]
			fn advance(&mut self, cnt: usize) {
				self.0.advance(cnt)
			}

			#[inline]
			fn has_remaining(&self) -> bool {
				!self.0.is_empty()
			}
		}

		impl ::buf::FromBuf for $ty {
			fn from_buf<T>(buf: T) -> Self
				where T: ::IntoBuf,
			{
				::unsync::macros::from_buf(buf).into()
			}
		}

		impl_cmp!($ty:
			[u8],
			{ref} [u8],
			str,
			{ref} str,
			Vec<u8>,
			String,
			::Bytes,
			::BytesMut,
		);

		impl Eq for $ty {}

		impl Ord for $ty {
			#[inline]
			fn cmp(&self, other: &$ty) -> ::std::cmp::Ordering {
				(self.as_ref() as &[u8]).cmp(other.as_ref() as &[u8])
			}
		}

		impl Default for $ty {
			#[inline]
			fn default() -> Self {
				$ty(Storage::new())
			}
		}

		impl ::std::hash::Hash for $ty {
			#[inline]
			fn hash<H>(&self, state: &mut H) where H: ::std::hash::Hasher {
				let s: &[u8] = self.as_ref();
				s.hash(state);
			}
		}

		impl IntoIterator for $ty {
			type Item = u8;
			type IntoIter = ::unsync::Iter<$ty>;

			#[inline]
			fn into_iter(self) -> Self::IntoIter {
				::unsync::Iter::new(self)
			}
		}

		impl<'a> IntoIterator for &'a $ty {
			type Item = u8;
			type IntoIter = ::unsync::SliceIter<'a>;

			#[inline]
			fn into_iter(self) -> Self::IntoIter {
				::unsync::SliceIter::new(self)
			}
		}
	};
}

macro_rules! impl_common_mut {
	($ty:ident) => {
		impl_common!($ty);

		impl Clone for $ty {
			fn clone(&self) -> Self {
				$ty(Storage::from_data(self))
			}
		}

		impl AsMut<[u8]> for $ty {
			#[inline]
			fn as_mut(&mut self) -> &mut [u8] {
				self.0.data_mut()
			}
		}

		impl DerefMut for $ty {
			#[inline]
			fn deref_mut(&mut self) -> &mut Self::Target {
				self.0.data_mut()
			}
		}

		impl Extend<u8> for $ty {
			fn extend<T>(&mut self, iter: T) where T: IntoIterator<Item = u8> {
				let iter = iter.into_iter();

				let (lower, _) = iter.size_hint();
				self.reserve(lower);

				for item in iter {
					self.0.append(item);
				}
			}
		}

		impl<'a> Extend<&'a u8> for $ty {
			fn extend<T>(&mut self, iter: T) where T: IntoIterator<Item = &'a u8> {
				self.extend(iter.into_iter().map(|b| *b))
			}
		}
	}
}
