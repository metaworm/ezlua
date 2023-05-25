//! Helpers to simplify the type conversion between rust and lua

use alloc::string::ToString;
use alloc::{boxed::Box, vec::Vec};

use crate::{
    convert::{FromLua, Index, ToLua, ToLuaMulti},
    error::Result,
    ffi,
    luaapi::{Reference, UnsafeLuaApi},
    prelude::{ArcLuaInner, LuaResult, LuaType},
    state::State,
    value::ValRef,
};

/// Mark an error result return as `nil, error` rather than raising it
pub struct NilError<T: ToLuaMulti>(pub T);

impl<T: ToLuaMulti> ToLuaMulti for NilError<T> {
    fn value_count(&self) -> Option<usize> {
        self.0.value_count()
    }

    fn push_multi(self, s: &crate::state::State) -> LuaResult<usize> {
        match self.0.push_multi(s) {
            Ok(res) => Ok(res),
            Err(err) => ((), err.to_string()).push_multi(s),
        }
    }
}

/// Represents an argument passed from lua on the stack
#[derive(Clone, Copy, Debug)]
pub struct ArgRef(pub Index);

/// Represents a value in the C registry
#[derive(Debug)]
pub struct RegVal {
    pub reference: Reference,
    pub(crate) inner: ArcLuaInner,
}

/// Represents a strict typed value, such as an integer value
#[derive(Clone, Copy)]
pub struct Strict<I>(pub I);

/// Represents a strict typed boolean value
pub type StrictBool = Strict<bool>;

/// Represents an iterator will be converted to a lua array table
pub struct IterVec<T: ToLua, I: Iterator<Item = T>>(pub I);

impl<T: ToLua, I: IntoIterator<Item = T>> From<I> for IterVec<T, I::IntoIter> {
    fn from(value: I) -> Self {
        Self(value.into_iter())
    }
}

/// Represents an iterator will be converted to a lua table
pub struct IterMap<K: ToLua, V: ToLua, I: Iterator<Item = (K, V)>>(pub I);

impl<K: ToLua, V: ToLua, I: IntoIterator<Item = (K, V)>> From<I> for IterMap<K, V, I::IntoIter> {
    fn from(value: I) -> Self {
        Self(value.into_iter())
    }
}

impl<T: ToLua, I: Iterator<Item = T>> ToLua for IterVec<T, I> {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> = Some(|this, s: &State| {
        let res = s.new_table_with_size(this.0.size_hint().1.unwrap_or(0) as _, 0)?;
        let mut i = 1;
        for e in this.0 {
            res.raw_seti(i, e)?;
            i += 1;
        }
        res.0.ensure_top();
        Ok(())
    });
}

impl<K: ToLua, V: ToLua, I: Iterator<Item = (K, V)>> ToLua for IterMap<K, V, I> {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> = Some(|this, s: &State| {
        let res = s.new_table_with_size(this.0.size_hint().1.unwrap_or(0) as _, 0)?;
        for e in this.0 {
            res.raw_set(e.0, e.1)?;
        }
        res.0.ensure_top();
        Ok(())
    });
}

impl FromLua<'_> for StrictBool {
    fn from_index(s: &State, i: Index) -> Option<StrictBool> {
        if s.is_bool(i) {
            Some(Strict(s.to_bool(i)))
        } else {
            None
        }
    }
}

impl<'a> FromLua<'a> for Strict<&'a [u8]> {
    fn from_index(s: &'a State, i: Index) -> Option<Self> {
        if s.type_of(i) == LuaType::String {
            s.to_safe_bytes(i).map(Strict)
        } else {
            None
        }
    }
}

impl<'a> FromLua<'a> for Strict<&'a str> {
    fn from_index(s: &'a State, i: Index) -> Option<Self> {
        core::str::from_utf8(Strict::<&'a [u8]>::from_index(s, i)?.0)
            .ok()
            .map(Strict)
    }
}

impl ToLua for ArgRef {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> =
        Some(|this, s: &State| Ok(s.push_value(this.0)));
}

impl ToLua for &RegVal {
    const __PUSH: Option<fn(Self, &State) -> LuaResult<()>> = Some(|this, s| unsafe {
        ffi::lua_rawgeti(s.raw_state(), ffi::LUA_REGISTRYINDEX, this.reference.0 as _);
        Ok(())
    });
}

impl Drop for RegVal {
    fn drop(&mut self) {
        self.inner
            .0
            .unreference(ffi::LUA_REGISTRYINDEX, self.reference);
    }
}

/// Represents an iterator
pub struct StaticIter<'a, T>(Box<dyn Iterator<Item = T> + 'a>);

impl<T: 'static, I: Iterator<Item = T> + 'static> From<I> for StaticIter<'static, T> {
    fn from(iter: I) -> Self {
        Self(Box::new(iter))
    }
}

impl<T: ToLuaMulti + 'static> StaticIter<'static, T> {
    pub fn new(iter: impl Iterator<Item = T> + 'static) -> Self {
        Self(Box::new(iter))
    }
}

impl<T: ToLuaMulti> ToLua for StaticIter<'static, T> {
    fn to_lua<'a>(self, s: &'a State) -> LuaResult<ValRef<'a>> {
        unsafe { s.new_iter(self.0, [(); 0]) }.map(Into::into)
    }
}

/// Represents results which are already pushed to the stack
#[derive(Debug, Default)]
pub struct Pushed(usize);

impl ToLuaMulti for Pushed {
    fn value_count(&self) -> Option<usize> {
        Some(self.0)
    }

    fn push_multi(self, s: &crate::state::State) -> Result<usize> {
        Ok(self.0)
    }
}

impl State {
    /// Push results to stack
    pub fn pushed<T: ToLuaMulti>(&self, results: T) -> Result<Pushed> {
        results.push_multi(self).map(Pushed)
    }
}

/// Represents a bytes buffer
#[derive(Debug, derive_more::From)]
pub struct LuaBytes(pub Vec<u8>);

impl ToLua for LuaBytes {
    fn to_lua<'a>(self, s: &'a State) -> LuaResult<ValRef<'a>> {
        self.0.as_slice().to_lua(s)
    }
}

impl FromLua<'_> for LuaBytes {
    fn from_index(s: &State, i: Index) -> Option<Self> {
        Some(Self(<&[u8] as FromLua>::from_index(s, i)?.to_vec()))
    }
}
