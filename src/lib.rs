#![doc = include_str!("../README.md")]
#![no_std]
#![feature(
    min_specialization,
    thread_id_value,
    associated_const_equality,
    macro_metavar_expr,
    const_type_name,
    unboxed_closures,
    fn_traits,
    tuple_trait,
    auto_traits,
    negative_impls,
    associated_type_defaults
)]
#![allow(dead_code, unused_variables)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;
#[macro_use]
extern crate derive_more;

#[cfg(feature = "std")]
#[macro_export]
macro_rules! cstr {
    ($lit:expr) => {
        unsafe {
            ::std::ffi::CStr::from_ptr(
                concat!($lit, "\0").as_ptr() as *const ::std::os::raw::c_char
            )
        }
    };
}

#[cfg(not(feature = "std"))]
pub use cstr_core::cstr;

#[cfg(feature = "std")]
pub mod str {
    pub use std::ffi::CStr;
    pub use std::ffi::CString;
}

#[cfg(not(feature = "std"))]
pub mod str {
    pub use cstr_core::CStr;
    pub use cstr_core::CString;
}

pub mod binding;
pub mod convert;
pub mod error;
#[doc(hidden)]
pub mod ffi;
#[doc(hidden)]
pub mod lua;
#[doc(hidden)]
pub mod luaapi;
pub mod marker;
pub mod prelude;
pub mod serde;
pub mod userdata;
pub mod value;

#[cfg(feature = "async")]
mod r#async;
mod coroutine;
mod luaconf;
mod state;

#[cfg(feature = "thread")]
pub mod lock;
