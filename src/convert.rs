//! Main implementation for type conversion between lua and rust

use crate::{
    error::{Error, Result, ToLuaResult},
    ffi::{self, *},
    luaapi::*,
    marker::{IterMap, IterVec, Pushed, Strict},
    prelude::StaticIter,
    state::State,
    userdata::{UserData, UserDataTrans},
    value::*,
};

use alloc::{
    borrow::{Cow, ToOwned},
    boxed::Box,
    ffi::CString,
    string::String,
    sync::Arc,
    vec::Vec,
};
use core::{
    cell::RefCell,
    ffi::CStr,
    fmt::Debug,
    ops::{Deref, DerefMut},
};

#[cfg(feature = "std")]
use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    hash::Hash,
};

pub type Index = i32;
pub type MetatableKey = fn(&Table) -> Result<()>;

#[cfg(feature = "serde_bytes")]
impl ToLua for &serde_bytes::Bytes {
    fn to_lua<'a>(self, s: &'a State) -> Result<ValRef<'a>> {
        ToLua::to_lua(self.as_ref(), s)
    }
}

#[cfg(feature = "serde_bytes")]
impl ToLua for serde_bytes::ByteBuf {
    fn to_lua<'a>(self, s: &'a State) -> Result<ValRef<'a>> {
        ToLua::to_lua(self.as_ref(), s)
    }
}

#[cfg(feature = "serde_bytes")]
impl FromLua<'_> for serde_bytes::ByteBuf {
    fn from_lua(s: &State, val: ValRef) -> Result<Self> {
        Ok(serde_bytes::ByteBuf::from(
            <&[u8] as FromLua>::from_lua(s, val)?.to_vec(),
        ))
    }
}

#[cfg(feature = "bytes")]
impl ToLua for &bytes::Bytes {
    fn to_lua<'a>(self, s: &'a State) -> Result<ValRef<'a>> {
        ToLua::to_lua(self.as_ref(), s)
    }
}

#[cfg(feature = "bytes")]
impl FromLua<'_> for bytes::Bytes {
    fn from_lua(s: &State, val: ValRef) -> Result<Self> {
        Ok(bytes::Bytes::from(
            val.to_bytes()
                .ok_or_else(|| Error::TypeNotMatch(val.type_of()))?
                .to_vec(),
        ))
    }
}

/// Trait for (closure)types that can be binded as C function or as method in metatable
///
/// See [`State::new_closure`] [`Table::set_closure`]
pub trait LuaMethod<'a, THIS: 'a, ARGS: 'a, RET: 'a> {
    fn call_method(&self, lua: &'a State) -> Result<Pushed>;
}

/// Trait for types that can be pushed onto the stack of a Lua
pub trait ToLua: Sized {
    #[doc(hidden)]
    const __PUSH: Option<fn(Self, &State) -> Result<()>> = None;

    fn to_lua<'a>(self, lua: &'a State) -> Result<ValRef<'a>> {
        if let Some(push) = Self::__PUSH {
            push(self, lua)?;
            Ok(lua.top_val())
        } else {
            lua.new_val(())
        }
    }
}

impl ToLua for () {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> = Some(|_, s| Ok(s.push_nil()));
}

impl ToLua for &[u8] {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> = Some(|this, s| Ok(s.push_bytes(this)));
}

macro_rules! impl_as_bytes {
    ($t:ty) => {
        impl ToLua for $t {
            const __PUSH: Option<fn(Self, &State) -> Result<()>> =
                Some(|this, s: &State| Ok(s.push_bytes(<Self as AsRef<[u8]>>::as_ref(&this))));
        }
    };
}

// impl_as_bytes!(Vec<u8>);
impl_as_bytes!(Cow<'_, [u8]>);

macro_rules! impl_as_str {
    ($t:ty) => {
        impl ToLua for $t {
            const __PUSH: Option<fn(Self, &State) -> Result<()>> =
                Some(|this, s: &State| Ok(s.push_bytes(this.as_bytes())));
        }
    };
}

impl_as_str!(&str);
impl_as_str!(Arc<str>);
impl_as_str!(Box<str>);
impl_as_str!(Cow<'_, str>);
impl_as_str!(String);
impl_as_str!(CString);

impl ToLua for &CStr {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> =
        Some(|this, s: &State| Ok(s.push_bytes(this.to_bytes())));
}

#[cfg(feature = "std")]
impl ToLua for &OsStr {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> =
        Some(|this, s: &State| Ok(s.push_bytes(this.as_encoded_bytes())));
}

#[cfg(feature = "std")]
impl ToLua for OsString {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> =
        Some(|this, s: &State| Ok(s.push_bytes(this.as_encoded_bytes())));
}

#[cfg(feature = "std")]
impl<'a> FromLua<'a> for &'a OsStr {
    fn from_lua(lua: &'a State, val: ValRef<'a>) -> Result<Self> {
        <&'a str>::from_lua(lua, val).map(OsStr::new)
    }
}

#[cfg(feature = "std")]
impl<'a> FromLua<'a> for OsString {
    fn from_lua(lua: &'a State, val: ValRef<'a>) -> Result<Self> {
        Ok(<&'a str>::from_lua(lua, val)
            .map(OsStr::new)?
            .to_os_string())
    }
}

impl ToLua for Value<'_> {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> =
        Some(|this, s: &State| Ok(s.pushv(this)));
}

impl ToLua for ValRef<'_> {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> = Some(|this, s: &State| {
        s.pushval(this);
        Ok(())
    });
}

impl ToLua for &ValRef<'_> {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> = Some(|this, s: &State| {
        s.pushvalref(this);
        Ok(())
    });
}

pub unsafe extern "C" fn __gc<T>(l: *mut lua_State) -> i32 {
    let s = State::from_raw_state(l);
    s.to_userdata_typed::<T>(1)
        .map(|p| core::ptr::drop_in_place(p));
    return 0;
}

impl ToLua for f64 {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> =
        Some(|this, s: &State| Ok(s.push_number(this)));
}

impl ToLua for f32 {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> =
        Some(|this, s: &State| Ok(s.push_number(this as _)));
}

impl ToLua for bool {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> =
        Some(|this, s: &State| Ok(s.push_bool(this)));
}

impl ToLua for CFunction {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> =
        Some(|this, s: &State| Ok(s.push_fn(Some(this))));
}

impl<T: ToLua> ToLua for Option<T> {
    fn to_lua<'a>(self, s: &'a State) -> Result<ValRef<'a>> {
        match self {
            Some(t) => t.to_lua(s),
            _ => ().to_lua(s),
        }
    }
}

impl<T: ToLua> ToLua for Vec<T> {
    fn to_lua<'a>(self, s: &'a State) -> Result<ValRef<'a>> {
        s.new_val(IterVec(self.into_iter()))
    }
}

#[cfg(feature = "std")]
impl<K: ToLua, V: ToLua> ToLua for HashMap<K, V> {
    fn to_lua<'a>(self, s: &'a State) -> Result<ValRef<'a>> {
        s.new_val(IterMap(self.into_iter()))
    }
}

/// Trait for types that can be taken from the Lua stack
///
/// For the reference types such as `&[u8]`, `&str`, the conversion will fail if `val` not an argument passed by lua.
/// For the compound reference types such as `Vec<&[u8]>`, the conversion always fail,
/// because it will create some temporary `ValRef`s on the stack, which can not hold the reference's lifetime.
///
/// In order to convert to reference type, you can use the [`ValRef::deserialize`] method with the `serde` feature enabled,
/// it can guarantee the lifetime of the reference type is same as `&ValRef`
pub trait FromLua<'a>: Sized {
    const TYPE_NAME: &'static str = core::any::type_name::<Self>();

    fn from_lua(lua: &'a State, val: ValRef<'a>) -> Result<Self>;
}

pub(crate) fn check_from_lua<'a, T: FromLua<'a>>(lua: &'a State, i: Index) -> Result<T> {
    lua.from_index.set(i);
    T::from_lua(lua, lua.val(i)).map_err(|err| {
        Error::convert(alloc::format!(
            "cast #{i}({}) failed, expect {}: {err:?}",
            lua.type_of(i),
            T::TYPE_NAME
        ))
    })
}

impl<'a> FromLua<'a> for ValRef<'a> {
    #[inline(always)]
    fn from_lua(s: &'a State, val: ValRef<'a>) -> Result<Self> {
        val.check_valid().ok_or("value is invalid").lua_result()
    }
}

impl<'a> FromLua<'a> for Value<'a> {
    #[inline(always)]
    fn from_lua(s: &'a State, val: ValRef<'a>) -> Result<Value<'a>> {
        val.checked_into_value()
            .ok_or("value is invalid")
            .lua_result()
    }
}

impl FromLua<'_> for String {
    const TYPE_NAME: &'static str = "string";

    #[inline(always)]
    fn from_lua(s: &State, val: ValRef) -> Result<String> {
        val.to_str()
            .map(ToOwned::to_owned)
            .ok_or_else(|| Error::TypeNotMatch(val.type_of()))
    }
}

impl FromLua<'_> for Box<str> {
    const TYPE_NAME: &'static str = "string";

    #[inline(always)]
    fn from_lua(s: &State, val: ValRef) -> Result<Self> {
        val.to_str()
            .map(Into::into)
            .ok_or_else(|| Error::TypeNotMatch(val.type_of()))
    }
}

impl FromLua<'_> for Arc<str> {
    const TYPE_NAME: &'static str = "string";

    #[inline(always)]
    fn from_lua(s: &State, val: ValRef) -> Result<Self> {
        val.to_str()
            .map(Into::into)
            .ok_or_else(|| Error::TypeNotMatch(val.type_of()))
    }
}

impl<'a> FromLua<'a> for Cow<'a, str> {
    const TYPE_NAME: &'static str = "string";

    fn from_lua(s: &'a State, val: ValRef<'a>) -> Result<Cow<'a, str>> {
        val.to_safe_str()
            .ok()
            .map(Cow::Borrowed)
            .or_else(|| val.to_string_lossy().map(Cow::into_owned).map(Cow::Owned))
            .ok_or_else(|| Error::TypeNotMatch(val.type_of()))
    }
}

impl<'a> FromLua<'a> for &'a str {
    const TYPE_NAME: &'static str = "string";

    #[inline(always)]
    fn from_lua(s: &'a State, val: ValRef<'a>) -> Result<&'a str> {
        core::str::from_utf8(val.to_safe_bytes()?).lua_result()
    }
}

impl<'a> FromLua<'a> for &'a [u8] {
    const TYPE_NAME: &'static str = "bytes";

    fn from_lua(lua: &'a State, val: ValRef<'a>) -> Result<&'a [u8]> {
        val.to_safe_bytes().or_else(|_| unsafe {
            // Treat userdata without metatable as bytes
            let p: *mut core::ffi::c_void = lua.to_userdata(val.index);
            if p.is_null() || val.has_metatable() {
                None
            } else {
                Some(core::slice::from_raw_parts(p.cast::<u8>(), val.raw_len()))
            }
            .ok_or(Error::TypeNotMatch(val.type_of()))
        })
    }
}

impl<'a, V: FromLua<'a> + 'static> FromLua<'a> for Vec<V> {
    fn from_lua(s: &'a State, val: ValRef<'a>) -> Result<Self> {
        let t = val.as_table().ok_or("").lua_result()?;

        let mut result = Vec::new();
        for i in 1..=t.raw_len() {
            result.push(t.raw_geti(i as i64)?.cast_into::<V>()?);
        }

        Ok(result)
    }
}

#[cfg(feature = "std")]
impl<'a, K: FromLua<'a> + Eq + Hash + 'static, V: FromLua<'a> + 'static> FromLua<'a>
    for HashMap<K, V>
{
    fn from_lua(s: &'a State, val: ValRef<'a>) -> Result<Self> {
        let t = val.as_table().ok_or("").lua_result()?;

        let mut result = HashMap::new();
        for (k, v) in t.iter()? {
            result.insert(k.cast_into::<K>()?, v.cast_into::<V>()?);
        }

        Ok(result)
    }
}

impl FromLua<'_> for f64 {
    #[inline(always)]
    fn from_lua(lua: &State, val: ValRef) -> Result<f64> {
        lua.to_numberx(val.index)
            .ok_or_else(|| Error::TypeNotMatch(val.type_of()))
    }
}

impl FromLua<'_> for f32 {
    #[inline(always)]
    fn from_lua(s: &State, val: ValRef) -> Result<f32> {
        s.to_numberx(val.index)
            .map(|r| r as f32)
            .ok_or_else(|| Error::TypeNotMatch(val.type_of()))
    }
}

impl FromLua<'_> for bool {
    #[inline(always)]
    fn from_lua(lua: &State, val: ValRef) -> Result<bool> {
        Some(lua.to_bool(val.index)).ok_or_else(|| Error::TypeNotMatch(val.type_of()))
    }
}

impl<'a, T: FromLua<'a>> FromLua<'a> for Option<T> {
    #[inline(always)]
    fn from_lua(lua: &'a State, val: ValRef<'a>) -> Result<Option<T>> {
        Ok(T::from_lua(lua, val).ok())
    }
}

macro_rules! impl_integer {
    ($($t:ty) *) => {
        $(
        impl ToLua for $t {
            const __PUSH: Option<fn(Self, &State) -> Result<()>> = Some(|this, s: &State| Ok(s.push_integer(this as _)));
        }

        impl FromLua<'_> for $t {
            fn from_lua(lua: &State, val: ValRef) -> Result<$t> {
                let i = val.index;
                if lua.is_integer(i) {
                    Ok(lua.to_integer(i) as $t)
                } else if lua.is_number(i) {
                    Ok(lua.to_number(i) as $t)
                } else {
                    Err(Error::TypeNotMatch(val.type_of()))
                }
            }
        }

        impl FromLua<'_> for Strict<$t> {
            fn from_lua(lua: &State, val: ValRef) -> Result<Strict<$t>> {
                let i = val.index;
                if lua.is_integer(i) {
                    Ok(Self(lua.to_integer(i) as $t))
                } else {
                    Err(Error::TypeNotMatch(val.type_of()))
                }
            }
        }
        )*
    }
}

impl_integer!(isize usize u8 u16 u32 u64 i8 i16 i32 i64);

/// Types which can be pushed onto lua stack,
/// as returned multiple values to lua function,
/// or as passed multiple arguments to lua function
pub trait ToLuaMulti: Sized {
    /// Count of values to be pushed to lua stack
    const VALUE_COUNT: Option<usize> = None;

    /// Get the count of values to be pushed to lua stack, with self instance
    fn value_count(&self) -> Option<usize> {
        Self::VALUE_COUNT
    }

    /// Define how to push values onto lua stack
    fn push_multi(self, lua: &State) -> Result<usize> {
        Ok(0)
    }
}

/// Types which can be converted from values passed from lua,
/// or returned results from lua function invocation
pub trait FromLuaMulti<'a>: Sized {
    const COUNT: usize = 0;

    fn from_lua_multi(lua: &'a State, _begin: Index) -> Result<Self>;
}

impl FromLuaMulti<'_> for () {
    const COUNT: usize = 0;

    fn from_lua_multi(lua: &State, _begin: Index) -> Result<Self> {
        Ok(())
    }
}

impl<T: ToLua> ToLuaMulti for T {
    const VALUE_COUNT: Option<usize> = Some(1);

    #[inline]
    fn push_multi(self, s: &State) -> Result<usize> {
        s.push(self).map(|_| 1)
    }
}

impl<'a, T: FromLua<'a>> FromLuaMulti<'a> for T {
    const COUNT: usize = 1;

    #[inline(always)]
    fn from_lua_multi(s: &'a State, begin: Index) -> Result<Self> {
        check_from_lua(s, begin)
    }
}

impl<T: ToLuaMulti, E: Debug + Send + Sync + 'static> ToLuaMulti for core::result::Result<T, E> {
    #[inline(always)]
    fn push_multi(self, s: &State) -> Result<usize> {
        match self {
            Ok(result) => result.push_multi(s),
            Err(_) if core::any::TypeId::of::<()>() == core::any::TypeId::of::<E>() => Ok(0),
            Err(err) => Err(Error::runtime_debug(err)),
        }
    }
}

macro_rules! impl_method {
    ($(($x:ident, $i:tt)) *) => (
        // For normal function
        #[allow(unused_parens)]
        impl<'a,
            FN: Fn($($x),*) -> RET + 'static,
            RET: ToLuaMulti + 'a,
            $($x: FromLua<'a> + 'a,)*
        > LuaMethod<'a, (), ($($x,)*), RET> for FN {
            #[inline(always)]
            fn call_method(&self, s: &'a State) -> Result<Pushed> {
                s.pushed(self($(check_from_lua::<$x>(s, 1 + $i)?,)*))
            }
        }

        // For normal function with &LuaState
        #[allow(unused_parens)]
        impl<'a,
            FN: Fn(&'a State, $($x),*) -> RET + 'static,
            RET: ToLuaMulti + 'a,
            $($x: FromLua<'a> + 'a,)*
        > LuaMethod<'a, (), (&'a State, $($x,)*), RET> for FN {
            #[inline(always)]
            fn call_method(&self, s: &'a State) -> Result<Pushed> {
                s.pushed(self(s, $(check_from_lua::<$x>(s, 1 + $i)?,)*))
            }
        }

        // For Deref
        #[allow(unused_parens)]
        impl<'a,
            FN: Fn(&T $(,$x)*) -> RET + 'static,
            T: 'a,
            THIS: Deref<Target = T> + ?Sized + 'a,
            RET: ToLuaMulti + 'a,
            $($x: FromLua<'a> + 'a,)*
        > LuaMethod<'a, (THIS, &'a T), ($($x,)*), RET> for FN
            where THIS: FromLua<'a>,
        {
            #[inline(always)]
            fn call_method(&self, s: &'a State) -> Result<Pushed> {
                let this = check_from_lua::<THIS>(s, 1)?;
                s.pushed(self(this.deref(), $(check_from_lua::<$x>(s, 2 + $i)?,)*))
            }
        }

        // For Deref Deref
        #[allow(unused_parens)]
        impl<'a,
            FN: Fn(&T $(,$x)*) -> RET + 'static,
            T: ?Sized + 'a,
            THIS: UserData + Deref<Target = T> + 'a,
            RET: ToLuaMulti + 'a,
            $($x: FromLua<'a> + 'a,)*
        > LuaMethod<'a, (THIS, &'a T, &'a T), ($($x,)*), RET> for FN
            where <THIS::Trans as UserDataTrans<THIS>>::Read<'a>: Deref<Target = THIS> + FromLua<'a> + 'a,
        {
            #[inline(always)]
            fn call_method(&self, s: &'a State) -> Result<Pushed> {
                let this = check_from_lua::<<THIS::Trans as UserDataTrans<THIS>>::Read<'a>>(s, 1)?;
                s.pushed(self(this.deref().deref(), $(check_from_lua::<$x>(s, 2 + $i)?,)*))
            }
        }

        // For &State, Deref
        #[allow(unused_parens)]
        impl<'a,
            FN: for<'b> Fn(&'b State, &'b T $(,$x)*) -> RET + 'static,
            T: 'a,
            THIS: Deref<Target = T> + ?Sized + 'a,
            RET: ToLuaMulti + 'a,
            $($x: FromLua<'a> + 'a,)*
        > LuaMethod<'a, (THIS, &'a T), (&'a State, $($x,)*), RET> for FN
            where THIS: FromLua<'a>,
        {
            #[inline(always)]
            fn call_method(&self, s: &'a State) -> Result<Pushed> {
                let this = check_from_lua::<THIS>(s, 1)?;
                s.pushed(self(s, this.deref(), $(check_from_lua::<$x>(s, 2 + $i)?,)*))
            }
        }

        // For DerefMut
        #[allow(unused_parens)]
        impl<'a,
            FN: Fn(&mut T $(,$x)*) -> RET + 'static,
            T: 'a,
            THIS: DerefMut<Target = T> + 'a,
            RET: ToLuaMulti + 'a,
            $($x: FromLua<'a> + 'a,)*
        > LuaMethod<'a, (THIS, &'a mut T), ($($x,)*), RET> for FN
            where THIS: FromLua<'a>,
        {
            #[inline(always)]
            fn call_method(&self, s: &'a State) -> Result<Pushed> {
                let mut this = check_from_lua::<THIS>(s, 1)?;
                s.pushed(self(this.deref_mut(), $(check_from_lua::<$x>(s, 2 + $i)?,)*))
            }
        }

        // For DerefMut DerefMut
        #[allow(unused_parens)]
        impl<'a,
            FN: Fn(&mut T $(,$x)*) -> RET + 'static,
            T: ?Sized + 'a,
            THIS: UserData + DerefMut<Target = T> + 'a,
            RET: ToLuaMulti + 'a,
            $($x: FromLua<'a> + 'a,)*
        > LuaMethod<'a, (THIS, &'a mut T, &'a mut T), ($($x,)*), RET> for FN
            where <THIS::Trans as UserDataTrans<THIS>>::Read<'a>: DerefMut<Target = THIS> + FromLua<'a> + 'a,
        {
            #[inline(always)]
            fn call_method(&self, s: &'a State) -> Result<Pushed> {
                let mut this = check_from_lua::<<THIS::Trans as UserDataTrans<THIS>>::Read<'a>>(s, 1)?;
                s.pushed(self(this.deref_mut().deref_mut(), $(check_from_lua::<$x>(s, 2 + $i)?,)*))
            }
        }

        // For &State, DerefMut
        #[allow(unused_parens)]
        impl<'a,
            FN: Fn(&'a State, &mut T $(,$x)*) -> RET + 'static,
            T: 'a,
            THIS: DerefMut<Target = T> + 'a,
            RET: ToLuaMulti + 'a,
            $($x: FromLua<'a> + 'a,)*
        > LuaMethod<'a, (THIS, &'a mut T), (&'a State, $($x,)*), RET> for FN
            where THIS: FromLua<'a>,
        {
            #[inline(always)]
            fn call_method(&self, s: &'a State) -> Result<Pushed> {
                let mut this = check_from_lua::<THIS>(s, 1)?;
                s.pushed(self(s, this.deref_mut(), $(check_from_lua::<$x>(s, 2 + $i)?,)*))
            }
        }
    );
}

impl_method!();

macro_rules! impl_tuple {
    ($(($x:ident, $i:tt)) +) => (
        impl<$($x,)*> ToLuaMulti for ($($x,)*) where $($x: ToLua,)* {
            const VALUE_COUNT: Option<usize> = Some(${count($x)});

            #[inline(always)]
            fn push_multi(self, s: &State) -> Result<usize> {
                $(s.push(self.$i)?;)*
                Ok(${count($x)} as _)
            }
        }

        impl<$($x,)*> ToLuaMulti for Option<($($x,)*)> where $($x: ToLua,)* {
            #[inline(always)]
            fn value_count(&self) -> Option<usize> {
                self.as_ref().map(|_| ${count($x)})
            }

            #[inline(always)]
            fn push_multi<'a>(self, s: &'a State) -> Result<usize> {
                match self {
                    Some(val) => val.push_multi(s),
                    None      => Ok(0),
                }
            }
        }

        impl<'a, $($x,)*> FromLuaMulti<'a> for ($($x,)*) where $($x: FromLua<'a>,)* {
            const COUNT: usize = ${count($x)};

            #[inline(always)]
            fn from_lua_multi(s: &'a State, begin: Index) -> Result<Self> {
                Ok(( $(check_from_lua::<$x>(s, begin + $i)?,)* ))
            }
        }

        impl<'a, $($x,)*> FromLuaMulti<'a> for (&'a State, $($x,)*) where $($x: FromLua<'a>,)* {
            const COUNT: usize = ${count($x)};

            #[inline(always)]
            fn from_lua_multi(s: &'a State, begin: Index) -> Result<Self> {
                Ok((s, $(check_from_lua::<$x>(s, begin + $i)?,)* ))
            }
        }

        impl_method!($(($x, $i))+);
    );
}

impl<'a> FromLuaMulti<'a> for (&'a State,) {
    const COUNT: usize = 0;

    #[inline(always)]
    fn from_lua_multi(s: &'a State, _: Index) -> Result<Self> {
        Ok((s,))
    }
}

impl_tuple!((A, 0));
impl_tuple!((A, 0)(B, 1));
impl_tuple!((A, 0)(B, 1)(C, 2));
impl_tuple!((A, 0)(B, 1)(C, 2)(D, 3));
impl_tuple!((A, 0)(B, 1)(C, 2)(D, 3)(E, 4));
impl_tuple!((A, 0)(B, 1)(C, 2)(D, 3)(E, 4)(F, 5));
impl_tuple!((A, 0)(B, 1)(C, 2)(D, 3)(E, 4)(F, 5)(G, 6));
impl_tuple!((A, 0)(B, 1)(C, 2)(D, 3)(E, 4)(F, 5)(G, 6)(H, 7));
impl_tuple!((A, 0)(B, 1)(C, 2)(D, 3)(E, 4)(F, 5)(G, 6)(H, 7)(I, 8));
impl_tuple!((A, 0)(B, 1)(C, 2)(D, 3)(E, 4)(F, 5)(G, 6)(H, 7)(I, 8)(J, 9));
impl_tuple!((A, 0)(B, 1)(C, 2)(D, 3)(E, 4)(F, 5)(G, 6)(H, 7)(I, 8)(J, 9)(K, 10));
impl_tuple!((A, 0)(B, 1)(C, 2)(D, 3)(E, 4)(F, 5)(G, 6)(H, 7)(I, 8)(J, 9)(K, 10)(L, 11));

macro_rules! impl_closure {
    ($name:ident ($($x:ident:$i:literal)*)) => (
        #[inline(always)]
        pub fn $name<'l,
            RET: ToLuaMulti + 'l,
            $($x: FromLua<'l>,)*
            FN: Fn(&'l State, $($x,)*) -> RET + 'static,
        >(
            &'l self,
            f: FN,
        ) -> Result<Function<'l>> {
            self.bind_closure(move |s: &'l State| Result::Ok(f(s, $(check_from_lua::<$x>(s, $i + 1)?,)*)), 0)
        }
    );
}

impl State {
    #[inline(always)]
    pub(crate) fn push_multi<'a, T: ToLuaMulti>(&'a self, t: T) -> Result<usize> {
        t.push_multi(self)
    }

    /// Create an iterator with non-static reference, you should ensure that these references
    /// is valid via the `refs` argument, which is be referenced by the iter closure's upvalues
    #[inline(always)]
    pub unsafe fn new_iter<'l, R: ToLuaMulti + 'l, I: Iterator<Item = R> + 'l, REF: ToLuaMulti>(
        &self,
        iter: I,
        refs: REF,
    ) -> Result<LuaUserData<'_>> {
        self.new_userdata_with_values(
            StaticIter {
                iter: Box::new(iter),
            },
            refs,
        )
    }

    /// Like [`State::new_iter`], and you can specify a map function.
    /// Deprecated: use coroutine for cross-state scene
    #[inline(always)]
    #[deprecated]
    pub unsafe fn new_iter_map<
        'l,
        R: 'l,
        I: Iterator<Item = R> + 'l,
        MR: ToLuaMulti + 'l,
        M: Fn(&'l State, R) -> MR,
        REF: ToLua,
        const C: usize,
    >(
        &'l self,
        iter: I,
        map: M,
        refs: [REF; C],
    ) -> Result<Function> {
        let iter = RefCell::new(iter);
        let val = self.bind_closure(
            move |s| {
                iter.try_borrow_mut()
                    .map(|mut iter| iter.next().map(|x| map(s, x)).ok_or(()))
            },
            C,
        )?;
        for (i, v) in refs.into_iter().enumerate() {
            val.set_upvalue(2 + i as i32, v)?;
        }
        Ok(val)
    }

    /// Bind a rust function(closure) with uniform argument types
    #[inline(always)]
    pub fn new_function<
        'l,
        ARGS: FromLuaMulti<'l>,
        RET: ToLuaMulti + 'l,
        F: Fn(&'l State, ARGS) -> RET + 'static,
    >(
        &'l self,
        fun: F,
    ) -> Result<Function<'_>> {
        self.bind_closure(
            move |s: &'l State| Result::Ok(fun(s, ARGS::from_lua_multi(s, 1)?)),
            0,
        )
    }

    /// Bind a rust function(closure) with flexible argument types
    #[inline(always)]
    pub fn new_closure<'l, A: 'l, R: 'l, F: LuaMethod<'l, (), A, R> + 'static>(
        &self,
        fun: F,
    ) -> Result<Function<'_>> {
        self.bind_closure(move |s: &'l State| fun.call_method(s), 0)
    }

    impl_closure!(new_closure0());
    impl_closure!(new_closure1(A:0));
    impl_closure!(new_closure2(A:0 B:1));
    impl_closure!(new_closure3(A:0 B:1 C:2));
    impl_closure!(new_closure4(A:0 B:1 C:2 D:3));
    impl_closure!(new_closure5(A:0 B:1 C:2 D:3 E:4));
    impl_closure!(new_closure6(A:0 B:1 C:2 D:3 E:4 F:5));
    impl_closure!(new_closure7(A:0 B:1 C:2 D:3 E:4 F:5 G:6));
    impl_closure!(new_closure8(A:0 B:1 C:2 D:3 E:4 F:5 G:6 H:7));
    impl_closure!(new_closure9(A:0 B:1 C:2 D:3 E:4 F:5 G:6 H:7 I:8));
    impl_closure!(new_closure10(A:0 B:1 C:2 D:3 E:4 F:5 G:6 H:7 I:8 J:9));

    #[doc(hidden)]
    #[inline(always)]
    pub fn bind_closure<'l, R: ToLuaMulti + 'l, F: Fn(&'l State) -> R>(
        &self,
        f: F,
        extra_upval: usize,
    ) -> Result<Function<'_>> {
        if core::mem::size_of::<F>() == 0 {
            self.check_stack(1)?;
            self.push_cclosure(Some(closure_wrapper::<'l, R, F>), 0);
        } else {
            self.check_stack(2 + extra_upval as i32)?;
            self.push_userdatauv(f, 0)?;
            self.push_binding(closure_wrapper::<'l, R, F>, __gc::<F>, extra_upval)?;
        }
        self.top_val().try_into()
    }

    pub(crate) fn push_binding(
        &self,
        cfunc: CFunction,
        gc: CFunction,
        upvals: usize,
    ) -> Result<()> {
        let mt = self.new_table_with_size(0, 2)?;
        mt.set("__gc", gc)?;
        mt.set("__close", gc)?;
        mt.0.ensure_top();
        self.set_metatable(-2);
        self.set_top(self.get_top() + upvals as i32);
        self.push_cclosure(Some(cfunc), 1 + upvals as i32);

        Ok(())
    }
}

/// Converts a rust closure to lua C function, for module creation purpose
#[inline(always)]
pub fn module_function_wrapper<'l, F: Fn(&'l State) -> Result<Table<'l>> + 'static>(
    fun: F,
) -> CFunction {
    assert!(core::mem::size_of::<F>() == 0);
    function_wrapper(fun)
}

/// Converts a rust closure to lua C function
#[inline(always)]
pub fn function_wrapper<'l, A: 'l, R: ToLuaMulti + 'l, F: LuaMethod<'l, (), A, R>>(
    fun: F,
) -> CFunction {
    #[inline(always)]
    fn to_wrapper<'l, R: ToLuaMulti + 'l, F: Fn(&'l State) -> R>(_f: F) -> CFunction {
        closure_wrapper::<'l, R, F>
    }

    to_wrapper(move |lua: &'l State| fun.call_method(lua))
}

pub unsafe extern "C" fn closure_wrapper<'l, R: ToLuaMulti + 'l, F: Fn(&'l State) -> R>(
    l: *mut lua_State,
) -> i32 {
    let state = State::from_raw_state(l);
    let s: &'l State = core::mem::transmute(&state);
    #[allow(unused_assignments)]
    let mut pfn = core::mem::transmute(1usize);
    let func: &F = if core::mem::size_of::<F>() == 0 {
        core::mem::transmute(pfn)
    } else {
        pfn = s.to_userdata(ffi::lua_upvalueindex(1));
        core::mem::transmute(pfn)
    };

    // Confusingly, if I use one statement, i.e. `state.return_result(func(s)) as _`, the `state`
    // seems to have been copied twice, causing the free array to fail to be released during drop, resulting in a memory leak
    let result = func(s);
    state.return_result(result) as _
}
