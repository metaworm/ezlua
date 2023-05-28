//! Main implementation for type conversion between lua and rust

use crate::{
    error::{Error, Result},
    ffi::{self, *},
    luaapi::*,
    marker::{IterMap, IterVec, Pushed, Strict},
    state::State,
    userdata::{UserData, UserDataTrans},
    value::*,
};

use alloc::{
    borrow::{Cow, ToOwned},
    ffi::CString,
    string::String,
    sync::Arc,
    vec::Vec,
};
use alloc::{boxed::Box, vec};
use core::cell::RefCell;
use core::ops::{Deref, DerefMut};
use core::{fmt::Debug, marker::Tuple};

#[cfg(feature = "std")]
use std::{collections::HashMap, hash::Hash};

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
    fn from_index(s: &State, i: Index) -> Option<Self> {
        Some(serde_bytes::ByteBuf::from(
            <&[u8] as FromLua>::from_index(s, i)?.to_vec(),
        ))
    }
}

pub trait LuaMethod<'a, THIS: 'a, ARGS: 'a, RET: 'a> {
    fn call_method(&self, lua: &'a State) -> Result<Pushed>;
}

/// Trait for types that can be pushed onto the stack of a Lua
pub trait ToLua: Sized {
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

#[cfg(feature = "std")]
impl ToLua for &std::ffi::OsStr {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> =
        Some(|this, s: &State| Ok(s.push_bytes(this.to_string_lossy().as_bytes())));
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
    const __PUSH: Option<fn(Self, &State) -> Result<()>> =
        Some(|this, s| match (T::__PUSH, this) {
            (Some(push), Some(this)) => push(this, s),
            (_, None) => Ok(s.push_nil()),
            (None, Some(this)) => Ok(this.to_lua(s)?.ensure_top()),
        });

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

/// Trait for types that can be taken from the Lua stack.
pub trait FromLua<'a>: Sized {
    const TYPE_NAME: &'static str = core::any::type_name::<Self>();

    fn from_index(lua: &'a State, i: Index) -> Option<Self> {
        Self::from_lua(lua, lua.val(i))
    }

    fn from_lua(lua: &'a State, val: ValRef<'a>) -> Option<Self> {
        None
    }

    fn check(lua: &'a State, i: Index) -> Result<Self> {
        Self::from_index(lua, i).ok_or_else(|| {
            Error::convert(alloc::format!(
                "cast #{i}({}) failed, expect {}",
                lua.type_of(i),
                Self::TYPE_NAME
            ))
        })
    }
}

impl<'a> FromLua<'a> for ValRef<'a> {
    #[inline(always)]
    fn from_index(s: &'a State, i: Index) -> Option<Self> {
        s.val(i).check_valid()
    }
}

impl<'a> FromLua<'a> for Value<'a> {
    #[inline(always)]
    fn from_index(s: &'a State, i: Index) -> Option<Value<'a>> {
        s.val(i).checked_into_value()
    }
}

impl FromLua<'_> for String {
    #[inline(always)]
    fn from_index(s: &State, i: Index) -> Option<String> {
        s.to_str(i).map(ToOwned::to_owned)
    }
}

impl<'a> FromLua<'a> for &'a str {
    #[inline(always)]
    fn from_index(s: &'a State, i: Index) -> Option<&'a str> {
        s.to_safe_bytes(i)
            .and_then(|b| core::str::from_utf8(b).ok())
    }
}

impl<'a> FromLua<'a> for &'a [u8] {
    #[inline(always)]
    fn from_index(s: &'a State, i: Index) -> Option<&'a [u8]> {
        s.to_safe_bytes(i).or_else(|| unsafe {
            let p: *mut core::ffi::c_void = s.to_userdata(i);
            if p.is_null() {
                None
            } else {
                Some(core::slice::from_raw_parts(
                    p.cast::<u8>(),
                    s.raw_len(i) as _,
                ))
            }
        })
    }
}

impl<'a, V: FromLua<'a> + 'static> FromLua<'a> for Vec<V> {
    fn from_lua(s: &'a State, val: ValRef<'a>) -> Option<Self> {
        let t = val.as_table()?;

        let mut result = Vec::new();
        for i in 1..=t.raw_len() {
            result.push(t.raw_geti(i as i64).cast::<V>()?);
        }

        Some(result)
    }
}

#[cfg(feature = "std")]
impl<'a, K: FromLua<'a> + Eq + Hash + 'static, V: FromLua<'a> + 'static> FromLua<'a>
    for HashMap<K, V>
{
    fn from_lua(s: &'a State, val: ValRef<'a>) -> Option<Self> {
        let t = val.as_table()?;

        let mut result = HashMap::new();
        for (k, v) in t.iter().ok()? {
            result.insert(k.cast::<K>()?, v.cast::<V>()?);
        }

        Some(result)
    }
}

impl FromLua<'_> for f64 {
    #[inline(always)]
    fn from_index(s: &State, i: Index) -> Option<f64> {
        s.to_numberx(i)
    }
}

impl FromLua<'_> for f32 {
    #[inline(always)]
    fn from_index(s: &State, i: Index) -> Option<f32> {
        s.to_numberx(i).map(|r| r as f32)
    }
}

impl FromLua<'_> for bool {
    #[inline(always)]
    fn from_index(s: &State, i: Index) -> Option<bool> {
        Some(s.to_bool(i))
    }
}

impl<'a, T: FromLua<'a>> FromLua<'a> for Option<T> {
    #[inline(always)]
    fn from_index(s: &'a State, i: Index) -> Option<Option<T>> {
        Some(T::from_index(s, i))
    }
}

macro_rules! impl_integer {
    ($($t:ty) *) => {
        $(
        impl ToLua for $t {
            const __PUSH: Option<fn(Self, &State) -> Result<()>> = Some(|this, s: &State| Ok(s.push_integer(this as _)));
        }

        impl FromLua<'_> for $t {
            fn from_index(s: &State, i: Index) -> Option<$t> {
                if s.is_integer(i) {
                    Some(s.to_integer(i) as $t)
                } else if s.is_number(i) {
                    Some(s.to_number(i) as $t)
                } else {
                    None
                }
            }
        }

        impl FromLua<'_> for Strict<$t> {
            fn from_index(s: &State, i: Index) -> Option<Strict<$t>> {
                if s.is_integer(i) {
                    Some(Self(s.to_integer(i) as $t))
                } else {
                    None
                }
            }
        }
        )*
    }
}

impl_integer!(isize usize u8 u16 u32 u64 i8 i16 i32 i64);

pub trait ToLuaMulti: Sized {
    fn value_count(&self) -> Option<usize> {
        None
    }

    fn push_multi(self, s: &State) -> Result<usize> {
        Ok(0)
    }
}

#[derive(Debug)]
pub struct MultiRet<T>(pub Vec<T>);

impl<T> Default for MultiRet<T> {
    fn default() -> Self {
        Self(vec![])
    }
}

impl<T: ToLua> ToLuaMulti for MultiRet<T> {
    fn value_count(&self) -> Option<usize> {
        Some(self.0.len())
    }

    fn push_multi(self, s: &State) -> Result<usize> {
        let len = self.0.len();
        for val in self.0 {
            s.push(val)?;
        }
        Ok(len as _)
    }
}

pub type MultiValue<'a> = MultiRet<Value<'a>>;
pub type MultiValRef<'a> = MultiRet<ValRef<'a>>;

impl<'a, T: FromLua<'a> + 'a> FromLua<'a> for MultiRet<T> {
    const TYPE_NAME: &'static str = core::any::type_name::<Self>();

    fn from_index(s: &'a State, i: Index) -> Option<Self> {
        let mut result = vec![];
        for i in i..=s.get_top() {
            result.push(T::from_index(s, i)?);
        }
        Some(Self(result))
    }
}

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
    #[inline]
    fn value_count(&self) -> Option<usize> {
        1.into()
    }

    #[inline]
    fn push_multi(self, s: &State) -> Result<usize> {
        s.push(self).map(|_| 1)
    }
}

impl<'a, T: FromLua<'a>> FromLuaMulti<'a> for T {
    const COUNT: usize = 1;

    #[inline(always)]
    fn from_lua_multi(s: &'a State, begin: Index) -> Result<Self> {
        T::check(s, begin)
    }
}

impl<T: ToLuaMulti, E: Debug + Send + Sync + 'static> ToLuaMulti for core::result::Result<T, E> {
    #[inline(always)]
    default fn push_multi(self, s: &State) -> Result<usize> {
        self.map_err(Error::runtime_debug)?.push_multi(s)
    }
}

impl<T: ToLuaMulti> ToLuaMulti for core::result::Result<T, ()> {
    #[inline(always)]
    default fn push_multi(self, s: &State) -> Result<usize> {
        match self {
            Ok(result) => result.push_multi(s),
            Err(_) => Ok(0),
        }
    }
}

macro_rules! impl_method {
    ($(($x:ident, $i:tt)) *) => (
        // For Deref
        #[allow(unused_parens)]
        impl<'a,
            FN: Fn(&T $(,$x)*) -> RET,
            T: 'a,
            THIS: Deref<Target = T> + ?Sized + 'a,
            RET: ToLuaMulti + 'a,
            $($x: FromLua<'a> + 'a,)*
        > LuaMethod<'a, (THIS, &'a T), ($($x,)*), RET> for FN
            where THIS: FromLua<'a>,
        {
            #[inline(always)]
            fn call_method(&self, s: &'a State) -> Result<Pushed> {
                let this = <THIS as FromLua>::check(s, 1)?;
                s.pushed(self(this.deref(), $($x::check(s, 2 + $i)?,)*))
            }
        }

        // For Deref Deref
        #[allow(unused_parens)]
        impl<'a,
            FN: Fn(&T $(,$x)*) -> RET,
            T: ?Sized + 'a,
            THIS: UserData + Deref<Target = T> + 'a,
            RET: ToLuaMulti + 'a,
            $($x: FromLua<'a> + 'a,)*
        > LuaMethod<'a, (THIS, &'a T, &'a T), ($($x,)*), RET> for FN
            where <THIS::Trans as UserDataTrans<THIS>>::Read<'a>: Deref<Target = THIS> + FromLua<'a> + 'a,
        {
            #[inline(always)]
            fn call_method(&self, s: &'a State) -> Result<Pushed> {
                let this = <<THIS::Trans as UserDataTrans<THIS>>::Read<'a> as FromLua>::check(s, 1)?;
                s.pushed(self(this.deref().deref(), $($x::check(s, 2 + $i)?,)*))
            }
        }

        // For &State, Deref
        #[allow(unused_parens)]
        impl<'a,
            FN: for<'b> Fn(&'b State, &'b T $(,$x)*) -> RET,
            T: 'a,
            THIS: Deref<Target = T> + ?Sized + 'a,
            RET: ToLuaMulti + 'a,
            $($x: FromLua<'a> + 'a,)*
        > LuaMethod<'a, (THIS, &'a T), (&'a State, $($x,)*), RET> for FN
            where THIS: FromLua<'a>,
        {
            #[inline(always)]
            fn call_method(&self, s: &'a State) -> Result<Pushed> {
                let this = <THIS as FromLua>::check(s, 1)?;
                s.pushed(self(s, this.deref(), $($x::check(s, 2 + $i)?,)*))
            }
        }

        // For DerefMut
        #[allow(unused_parens)]
        impl<'a,
            FN: Fn(&mut T $(,$x)*) -> RET,
            T: 'a,
            THIS: DerefMut<Target = T> + 'a,
            RET: ToLuaMulti + 'a,
            $($x: FromLua<'a> + 'a,)*
        > LuaMethod<'a, (THIS, &'a mut T), ($($x,)*), RET> for FN
            where THIS: FromLua<'a>,
        {
            #[inline(always)]
            fn call_method(&self, s: &'a State) -> Result<Pushed> {
                let mut this = <THIS as FromLua>::check(s, 1)?;
                s.pushed(self(this.deref_mut(), $($x::check(s, 2 + $i)?,)*))
            }
        }

        // For DerefMut DerefMut
        #[allow(unused_parens)]
        impl<'a,
            FN: Fn(&mut T $(,$x)*) -> RET,
            T: ?Sized + 'a,
            THIS: UserData + DerefMut<Target = T> + 'a,
            RET: ToLuaMulti + 'a,
            $($x: FromLua<'a> + 'a,)*
        > LuaMethod<'a, (THIS, &'a mut T, &'a mut T), ($($x,)*), RET> for FN
            where <THIS::Trans as UserDataTrans<THIS>>::Read<'a>: DerefMut<Target = THIS> + FromLua<'a> + 'a,
        {
            #[inline(always)]
            fn call_method(&self, s: &'a State) -> Result<Pushed> {
                let mut this = <<THIS::Trans as UserDataTrans<THIS>>::Read<'a> as FromLua>::check(s, 1)?;
                s.pushed(self(this.deref_mut().deref_mut(), $($x::check(s, 2 + $i)?,)*))
            }
        }

        // For &State, DerefMut
        #[allow(unused_parens)]
        impl<'a,
            FN: Fn(&'a State, &mut T $(,$x)*)->RET,
            T: 'a,
            THIS: DerefMut<Target = T> + 'a,
            RET: ToLuaMulti + 'a,
            $($x: FromLua<'a> + 'a,)*
        > LuaMethod<'a, (THIS, &'a mut T), (&'a State, $($x,)*), RET> for FN
            where THIS: FromLua<'a>,
        {
            #[inline(always)]
            fn call_method(&self, s: &'a State) -> Result<Pushed> {
                let mut this = <THIS as FromLua>::check(s, 1)?;
                s.pushed(self(s, this.deref_mut(), $($x::check(s, 2 + $i)?,)*))
            }
        }
    );
}

impl_method!();

macro_rules! impl_tuple {
    ($(($x:ident, $i:tt)) +) => (
        impl<$($x,)*> ToLuaMulti for ($($x,)*) where $($x: ToLua,)* {
            #[inline(always)]
            fn value_count(&self) -> Option<usize> {
                Some(${count(x)})
            }

            #[inline(always)]
            fn push_multi(self, s: &State) -> Result<usize> {
                $(s.push(self.$i)?;)*
                Ok(${count(x)} as _)
            }
        }

        impl<$($x,)*> ToLuaMulti for Option<($($x,)*)> where $($x: ToLua,)* {
            #[inline(always)]
            fn value_count(&self) -> Option<usize> {
                self.as_ref().map(|_| ${count(x)})
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
            const COUNT: usize = ${count(x)};

            #[inline(always)]
            fn from_lua_multi(s: &'a State, begin: Index) -> Result<Self> {
                Ok(( $($x::check(s, begin + $i)?,)* ))
            }
        }

        impl<'a, $($x,)*> FromLuaMulti<'a> for (&'a State, $($x,)*) where $($x: FromLua<'a>,)* {
            const COUNT: usize = ${count(x)};

            #[inline(always)]
            fn from_lua_multi(s: &'a State, begin: Index) -> Result<Self> {
                Ok((s, $($x::check(s, begin + $i)?,)* ))
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
impl_tuple!((A, 0)(B, 1)(C, 2)(D, 3)(E, 4)(F, 5)(G, 6)(H, 7)(I, 8)(J, 9)(K, 10)(L, 11)(M, 12));

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
            self.bind_closure(move |s: &'l State| Result::Ok(f(s, $($x::check(s, $i + 1)?,)*)), 0)
        }
    );
}

impl State {
    #[inline(always)]
    pub(crate) fn arg<'a, T: FromLua<'a>>(&'a self, index: Index) -> Option<T> {
        T::from_index(self, index)
    }

    #[inline(always)]
    pub(crate) fn push_multi<'a, T: ToLuaMulti>(&'a self, t: T) -> Result<usize> {
        t.push_multi(self)
    }

    /// Create an iterator with non-static reference, you should ensure that these references
    /// is valid via the `refs` argument, which is be referenced by the iter closure's upvalues
    #[inline(always)]
    pub unsafe fn new_iter<
        'l,
        R: ToLuaMulti + 'l,
        I: Iterator<Item = R> + 'l,
        REF: ToLua,
        const C: usize,
    >(
        &self,
        iter: I,
        refs: [REF; C],
    ) -> Result<Function<'_>> {
        let iter = RefCell::new(iter);
        let val = self.bind_closure(
            move |s| iter.try_borrow_mut().map(|mut iter| iter.next().ok_or(())),
            C,
        )?;
        for (i, v) in refs.into_iter().enumerate() {
            val.set_upvalue(2 + i as i32, v)?;
        }
        Ok(val)
    }

    /// Like [`State::new_iter`], and you can specify a map function
    #[inline(always)]
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

    /// Converts a rust closure to lua function without any captured variableds
    #[inline(always)]
    pub fn function_wrapper<
        'l,
        A: FromLuaMulti<'l> + Tuple,
        R: ToLuaMulti + 'l,
        F: Fn<A, Output = R>,
    >(
        fun: F,
    ) -> lua_CFunction {
        Self::to_function_wrapper(move |s: &'l State| {
            Result::Ok(fun.call(A::from_lua_multi(s, 1)?))
        })
    }

    #[inline(always)]
    pub fn to_function_wrapper<'l, R: ToLuaMulti + 'l, F: Fn(&'l State) -> R>(
        _f: F,
    ) -> lua_CFunction {
        assert!(core::mem::size_of::<F>() == 0);
        Some(closure_wrapper::<'l, R, F>)
    }

    /// Bind a rust function(closure) with uniform argument types
    #[inline(always)]
    pub fn new_function<
        'l,
        ARGS: FromLuaMulti<'l>,
        RET: ToLuaMulti + 'l,
        F: Fn(&'l State, ARGS) -> RET + 'static,
    >(
        &self,
        fun: F,
    ) -> Result<Function<'_>> {
        self.bind_closure(
            move |s: &'l State| Result::Ok(fun(s, ARGS::from_lua_multi(s, 1)?)),
            0,
        )
    }

    /// Bind a rust function(closure) with flexible argument types
    #[inline(always)]
    pub fn new_closure<
        'l,
        A: FromLuaMulti<'l> + Tuple,
        R: ToLuaMulti + 'l,
        F: Fn<A, Output = R> + 'static,
    >(
        &self,
        fun: F,
    ) -> Result<Function<'_>> {
        self.bind_closure(
            move |s: &'l State| Result::Ok(fun.call(A::from_lua_multi(s, 1)?)),
            0,
        )
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
            self.push_cclosure(Some(closure_wrapper::<'l, R, F>), 0);
        } else {
            self.push_userdatauv(f, 0)?;
            let mt = self.new_table_with_size(0, 1)?;
            mt.set("__gc", __gc::<F> as CFunction)?;
            mt.0.ensure_top();
            self.set_metatable(-2);
            self.set_top(self.get_top() + extra_upval as i32);
            self.push_cclosure(Some(closure_wrapper::<'l, R, F>), 1 + extra_upval as i32);
        }
        self.top_val().try_into()
    }
}

pub unsafe extern "C" fn closure_wrapper<'l, R: ToLuaMulti + 'l, F: Fn(&'l State) -> R>(
    l: *mut lua_State,
) -> i32 {
    let state = State::from_raw_state(l);
    let s: &'l State = core::mem::transmute(&state);
    #[allow(unused_assignments)]
    let mut pfn = core::mem::transmute(1usize);
    let f: &F = if core::mem::size_of::<F>() == 0 {
        core::mem::transmute(pfn)
    } else {
        pfn = s.to_userdata(ffi::lua_upvalueindex(1));
        core::mem::transmute(pfn)
    };

    state.return_result(f(s)) as _
}
