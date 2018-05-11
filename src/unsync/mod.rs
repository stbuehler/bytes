#![warn(warnings)]
#![allow(missing_docs, missing_debug_implementations)]

#[macro_use]
mod macros;

mod bytes_ext;
mod bytes_ext_impls;
mod bytes_mut;
mod bytes_mut_impls;
mod bytes_ro;
mod bytes_ro_impls;
mod iter;
mod storage;

pub use self::bytes_ext::UnBytesExt;
pub use self::bytes_mut::UnBytesMut;
pub use self::bytes_ro::UnBytes;
pub use self::iter::{
	Iter,
	SliceIter,
};
