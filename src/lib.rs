#![doc = include_str!("../README.md")]
#![no_std]
#![feature(
    macro_metavar_expr,
    const_type_name,
    associated_type_defaults,
    async_closure,
    box_into_inner
)]
#![cfg_attr(feature = "std", feature(thread_id_value))]
#![allow(dead_code, unused_variables)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;
#[macro_use]
extern crate derive_more;

#[macro_export]
macro_rules! cstr {
    ($lit:expr) => {
        unsafe { ::core::ffi::CStr::from_ptr(concat!($lit, "\0").as_ptr() as _) }
    };
}

pub mod str {
    pub use alloc::ffi::CString;
    pub use core::ffi::CStr;
}

pub mod binding;
#[cfg(feature = "compact")]
pub mod compat;
pub mod convert;
pub mod error;
#[doc(hidden)]
pub mod ffi;
#[doc(hidden)]
pub mod lua;
#[doc(hidden)]
pub mod luaapi;
pub mod macros;
pub mod marker;
pub mod prelude;
#[cfg(all(feature = "std", feature = "serde"))]
pub mod serde;
pub mod userdata;
pub mod value;

#[cfg(feature = "async")]
mod r#async;
mod coroutine;
mod state;

#[cfg(feature = "thread")]
pub mod lock;
