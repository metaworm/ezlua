//! Helpers to simplify the type conversion between rust and lua

use core::cell::RefCell;

use alloc::string::ToString;
use alloc::{boxed::Box, vec::Vec};

use crate::{
    convert::{FromLua, Index, ToLua, ToLuaMulti},
    error::{Error, Result},
    ffi,
    luaapi::{Reference, UnsafeLuaApi},
    prelude::{ArcLuaInner, LuaType, ToLuaResult},
    state::State,
    userdata::UserData,
    value::{LuaUserData, ValRef, Value},
};

/// Mark an error result return as `nil, error` rather than raising it
pub struct NilError<T: ToLuaMulti>(pub T);

impl<T: ToLuaMulti> ToLuaMulti for NilError<T> {
    fn value_count(&self) -> Option<usize> {
        self.0.value_count()
    }

    fn push_multi(self, s: &crate::state::State) -> Result<usize> {
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
    fn to_lua<'a>(self, lua: &'a State) -> Result<ValRef<'a>> {
        let res = lua.new_table_with_size(self.0.size_hint().1.unwrap_or(0) as _, 0)?;
        let mut i = 1;
        for e in self.0 {
            res.raw_seti(i, e)?;
            i += 1;
        }
        Ok(res.into())
    }
}

impl<K: ToLua, V: ToLua, I: Iterator<Item = (K, V)>> ToLua for IterMap<K, V, I> {
    fn to_lua<'a>(self, lua: &'a State) -> Result<ValRef<'a>> {
        let res = lua.new_table_with_size(self.0.size_hint().1.unwrap_or(0) as _, 0)?;
        for e in self.0 {
            res.raw_set(e.0, e.1)?;
        }
        Ok(res.into())
    }
}

impl FromLua<'_> for StrictBool {
    fn from_lua(lua: &State, val: ValRef) -> Result<StrictBool> {
        val.check_type(LuaType::Boolean)?;
        Ok(Strict(val.to_bool()))
    }
}

impl<'a> FromLua<'a> for Strict<&'a [u8]> {
    fn from_lua(s: &'a State, val: ValRef<'a>) -> Result<Self> {
        val.to_safe_bytes().map(Strict)
    }
}

impl<'a> FromLua<'a> for Strict<&'a str> {
    fn from_lua(s: &'a State, val: ValRef<'a>) -> Result<Self> {
        val.to_safe_str().map(Strict)
    }
}

impl ToLua for ArgRef {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> =
        Some(|this, s: &State| Ok(s.push_value(this.0)));
}

impl ToLua for &RegVal {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> = Some(|this, s| unsafe {
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
pub struct StaticIter<'a, T> {
    pub(crate) iter: Box<dyn Iterator<Item = T> + 'a>,
}

impl<T: ToLuaMulti> UserData for StaticIter<'_, T> {
    type Trans = RefCell<Self>;

    fn methods(methods: crate::userdata::UserdataRegistry<Self>) -> Result<()> {
        methods.add_method_mut("next", |_, this, ()| this.iter.next().ok_or(()))?;
        methods.add_method_mut("nth", |_, this, i: Option<usize>| {
            i.and_then(|i| this.iter.nth(i)).ok_or(())
        })?;
        methods.set(
            "last",
            methods.state().new_closure1(|_, this: LuaUserData| {
                this.take::<Self>()
                    .and_then(|this| this.into_inner().iter.last())
                    .ok_or(())
            })?,
        )?;
        methods.set(
            "count",
            methods.state().new_closure1(|_, this: LuaUserData| {
                this.take::<Self>()
                    .map(|this| this.into_inner().iter.count())
                    .ok_or(())
            })?,
        )?;
        methods.add_method_mut("size_hint", |_, this, ()| this.iter.size_hint())?;

        Ok(())
    }

    fn metatable(mt: crate::userdata::UserdataRegistry<Self>) -> Result<()> {
        mt.set("__call", mt.get("__method")?.get("next")?)?;
        mt.set("__index", mt.get("__method")?.get("nth")?)?;

        Ok(())
    }
}

impl<T: 'static, I: Iterator<Item = T> + 'static> From<I> for StaticIter<'static, T> {
    fn from(iter: I) -> Self {
        Self {
            iter: Box::new(iter),
        }
    }
}

impl<T: ToLuaMulti + 'static> StaticIter<'static, T> {
    pub fn new(iter: impl Iterator<Item = T> + 'static) -> Self {
        Self::from(iter)
    }
}

/// Represents results which are already pushed to the stack
///
/// Notice: this type can only be used at the end of a function
#[derive(Debug, Default)]
pub struct Pushed(usize);

impl ToLuaMulti for Pushed {
    #[inline(always)]
    fn value_count(&self) -> Option<usize> {
        Some(self.0)
    }

    #[inline(always)]
    fn push_multi(self, s: &crate::state::State) -> Result<usize> {
        Ok(self.0)
    }
}

impl State {
    /// Push results to stack
    #[inline(always)]
    pub fn pushed<T: ToLuaMulti>(&self, results: T) -> Result<Pushed> {
        results.push_multi(self).map(Pushed)
    }
}

/// Represents a bytes buffer
#[derive(Debug, derive_more::From)]
pub struct LuaBytes(pub Vec<u8>);

impl ToLua for LuaBytes {
    fn to_lua<'a>(self, s: &'a State) -> Result<ValRef<'a>> {
        self.0.as_slice().to_lua(s)
    }
}

impl FromLua<'_> for LuaBytes {
    fn from_lua(lua: &State, val: ValRef) -> Result<Self> {
        Ok(Self(
            val.to_bytes()
                .ok_or_else(|| Error::TypeNotMatch(val.type_of()))?
                .to_vec(),
        ))
    }
}

/// Wrapper to multiple value, which can be passed from lua as variable arguments and return multiple values to lua
///
/// ```rust
/// lua.global().set(
///     "echo_strs",
///     lua.new_function(|_, args: MultiRet<&str>| args)?,
/// )?;
/// lua.global().set("echo_vals", lua.new_function(|_, args: MultiValue| args)?)?;
///
/// lua.do_string("print(echo_strs('1', '2', '3'))", None)?;
/// lua.do_string("print(echo_vals('1', true, 2))", None)?;
/// ```
#[derive(Debug, Deref, DerefMut, From, Into)]
pub struct MultiRet<T>(pub Vec<T>);

impl<T> Default for MultiRet<T> {
    fn default() -> Self {
        Self(Vec::new())
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

/// Alias to `MultiRet<Value<'a>>`
pub type MultiValue<'a> = MultiRet<Value<'a>>;

/// Alias to `MultiRet<ValRef<'a>>`
pub type MultiValRef<'a> = MultiRet<ValRef<'a>>;

impl<'a, T: FromLua<'a> + 'a> FromLua<'a> for MultiRet<T> {
    const TYPE_NAME: &'static str = core::any::type_name::<Self>();

    fn from_lua(lua: &'a State, val: ValRef<'a>) -> Result<Self> {
        let index = lua.from_index.get();
        debug_assert_ne!(index, 0);
        let mut top = lua.get_top();
        if top == 1 && lua.is_none_or_nil(top) {
            top = 0;
        }
        let count = (top + 1 - index).max(0);
        let mut result = Vec::with_capacity(count as _);
        for i in index..=top {
            result.push(T::from_lua(lua, lua.val(i))?);
        }
        Ok(Self(result))
    }
}

/// Represents an userdata which ownedship is taken
pub struct OwnedUserdata<T>(pub T);

impl<'a, T: UserData<Trans = T>> FromLua<'a> for OwnedUserdata<T> {
    const TYPE_NAME: &'static str = T::TYPE_NAME;

    fn from_lua(lua: &'a State, val: ValRef<'a>) -> Result<OwnedUserdata<T>> {
        let u = LuaUserData::try_from(val)?;
        u.take::<T>()
            .ok_or("userdata not match")
            .lua_result()
            .map(OwnedUserdata)
    }
}
