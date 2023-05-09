//! Helpers to simplify the type conversion between rust and lua

use alloc::boxed::Box;
use alloc::string::ToString;
use core::mem;

use crate::error::Result;
use crate::ffi::{self, lua_State, CFunction};
use crate::luaapi::{Reference, UnsafeLuaApi};
use crate::prelude::{LuaResult, LuaType};
use crate::state::State;
use crate::{
    convert::{FromLua, Index, ToLua, ToLuaMulti},
    prelude::ArcLuaInner,
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

/// Represents an iterator will be converted to a lua table
pub struct IterMap<K: ToLua, V: ToLua, I: Iterator<Item = (K, V)>>(pub I);

impl<T: ToLua, I: Iterator<Item = T>> ToLua for IterVec<T, I> {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> = Some(|this, s: &State| {
        let r = s.create_table(this.0.size_hint().1.unwrap_or(0) as _, 0)?;
        let mut i = 1;
        for e in this.0 {
            r.raw_seti(i, e)?;
            i += 1;
        }
        Ok(())
    });
}

impl<K: ToLua, V: ToLua, I: Iterator<Item = (K, V)>> ToLua for IterMap<K, V, I> {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> = Some(|this, s: &State| {
        let r = s.create_table(this.0.size_hint().1.unwrap_or(0) as _, 0)?;
        for e in this.0 {
            s.push(e.0)?;
            s.push(e.1)?;
            s.raw_set(-2);
        }
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

impl<'a, T: ToLuaMulti + 'a> StaticIter<'a, T> {
    unsafe extern "C" fn lua_fn(l: *mut lua_State) -> i32 {
        let s = State::from_raw_state(l);
        let p = s.to_userdata(ffi::lua_upvalueindex(1));
        let iter: &mut StaticIter<'a, T> = mem::transmute(p);
        if let Some(v) = iter.0.next() {
            s.return_result(v) as _
        } else {
            0
        }
    }
}

impl<'a, T: ToLuaMulti + 'a> ToLua for StaticIter<'a, T> {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> = Some(|this, s| {
        s.push_userdatauv(this, 0)?;
        let mt = s.create_table(0, 1)?;
        mt.set(
            "__gc",
            crate::convert::__gc::<StaticIter<'static, usize>> as CFunction,
        )?;
        mt.ensure_top();
        s.set_metatable(-2);
        s.push_cclosure(Some(Self::lua_fn), 1);
        Ok(())
    });
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
