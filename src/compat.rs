//! Simple compatibility layer to mlua/rlua

use core::cell::RefCell;

use crate::{ffi, prelude::*};

#[extend::ext(pub, name = LuaStateCompact)]
impl LuaState {
    /// Wraps a Rust function or closure, creating a callable Lua function handle to it.
    ///
    /// The function's return value is always a `LuaResult`: If the function returns `Err`, the error
    /// is raised as a Lua error, which can be caught using `(x)pcall` or bubble up to the Rust code
    /// that invoked the Lua code. This allows using the `?` operator to propagate errors through
    /// intermediate Lua code.
    ///
    /// If the function returns `Ok`, the contained value will be converted to one or more Lua
    /// values. For details on Rust-to-Lua conversions, refer to the [`ToLua`] and [`ToLuaMulti`]
    /// traits.
    #[inline(always)]
    fn create_function<
        'l,
        ARGS: FromLuaMulti<'l>,
        RET: ToLuaMulti + 'l,
        F: Fn(&'l LuaState, ARGS) -> LuaResult<RET> + 'static,
    >(
        &'l self,
        func: F,
    ) -> LuaResult<LuaFunction> {
        self.new_function(func)
    }

    /// Wraps a Rust mutable closure, creating a callable Lua function handle to it.
    ///
    /// This is a version of [`create_function`] that accepts a FnMut argument. Refer to
    /// [`create_function`] for more information about the implementation.
    ///
    /// [`create_function`]: #method.create_function
    fn create_function_mut<
        'l,
        ARGS: FromLuaMulti<'l>,
        RET: ToLuaMulti + 'l,
        F: Fn(&'l LuaState, ARGS) -> LuaResult<RET> + 'static,
    >(
        &'l self,
        func: F,
    ) -> LuaResult<LuaFunction> {
        let func = RefCell::new(func);
        self.create_function(move |lua, args| (*func.try_borrow_mut().lua_result()?)(lua, args))
    }

    /// Creates a table from an iterator of values, using `1..` as the keys.
    #[inline(always)]
    fn create_sequence_from<V: ToLua, I: IntoIterator<Item = V>>(
        &self,
        iter: I,
    ) -> LuaResult<LuaTable> {
        self.new_val(IterVec(iter.into_iter()))
            .and_then(TryInto::try_into)
    }

    /// Creates a table and fills it with values from an iterator.
    #[inline(always)]
    fn create_table_from<K, V, I>(&self, iter: I) -> LuaResult<LuaTable>
    where
        K: ToLua,
        V: ToLua,
        I: IntoIterator<Item = (K, V)>,
    {
        self.new_val(IterMap(iter.into_iter()))
            .and_then(TryInto::try_into)
    }

    /// Create and return an interned Lua string. Lua strings can be arbitrary [u8] data including
    /// embedded nulls, so in addition to `&str` and `&String`, you can also pass plain `&[u8]`
    /// here.
    #[inline(always)]
    fn create_string<S: AsRef<[u8]>>(&self, s: S) -> LuaResult<LuaString> {
        self.new_string(s)
    }

    /// Creates and returns a new empty table.
    #[inline(always)]
    fn create_table(&self) -> LuaResult<LuaTable> {
        self.new_table()
    }

    /// Converts `T` into a [`Value`] instance.
    ///
    /// Requires `feature = "serde"`
    #[cfg(feature = "serde")]
    #[inline(always)]
    fn to_value<V: serde::Serialize>(&self, val: V) -> LuaResult<LuaValue> {
        self.new_val(crate::serde::SerdeValue(val))
            .map(ValRef::into_value)
    }

    /// Converts `T` into a [`Value`] instance with options.
    ///
    /// Requires `feature = "serde"`
    #[cfg(feature = "serde")]
    #[inline(always)]
    fn from_value<'a, R: serde::Deserialize<'a>>(&self, val: &'a ValRef) -> LuaResult<R> {
        val.deserialize().lua_result()
    }

    /// Wraps a C function, creating a callable Lua function handle to it.
    ///
    /// # Safety
    /// This function is unsafe because provides a way to execute unsafe C function.
    #[inline(always)]
    unsafe fn create_c_function(&self, func: ffi::CFunction) -> LuaResult<LuaFunction> {
        self.new_val(func).and_then(TryInto::try_into)
    }

    /// Returns a handle to the global environment.
    #[inline(always)]
    fn globals(&self) -> LuaTable {
        self.global()
    }

    /// Wraps a Lua function into a new thread (or coroutine).
    ///
    /// Equivalent to `coroutine.create`.
    fn create_thread<'lua>(&'lua self, func: LuaFunction<'lua>) -> LuaResult<Coroutine> {
        Coroutine::new(func.into())
    }

    /// Create a Lua userdata object from a custom userdata type.
    ///
    /// All userdata instances of type `T` shares the same metatable.
    #[inline]
    fn create_userdata<T>(&self, data: T) -> LuaResult<LuaUserData>
    where
        T: 'static + UserData,
    {
        self.new_userdata(data)
    }

    /// Converts a value that implements `ToLua` into a `Value` instance.
    #[inline(always)]
    fn pack<'lua, T: ToLua>(&'lua self, t: T) -> LuaResult<LuaValue<'lua>> {
        self.new_value(t)
    }

    /// Converts a `Value` instance into a value that implements `FromLua`.
    #[inline(always)]
    fn unpack<'lua, T: FromLua<'lua>>(&'lua self, value: LuaValue<'lua>) -> LuaResult<T> {
        T::from_lua(self, self.new_val(value)?)
    }

    /// Converts a value that implements `ToLuaMulti` into a `MultiValue` instance.
    #[inline(always)]
    fn pack_multi<'lua, T: ToLuaMulti>(&'lua self, t: T) -> LuaResult<MultiValue<'lua>> {
        let guard = self.stack_guard();
        t.push_multi(self)?;
        let top = guard.top();
        self.to_multi_balance(guard, top)
    }

    /// Converts a `MultiValue` instance into a value that implements `FromLuaMulti`.
    #[inline(always)]
    fn unpack_multi<'lua, T: FromLuaMulti<'lua>>(
        &'lua self,
        value: MultiValue<'lua>,
    ) -> LuaResult<T> {
        let guard = self.stack_guard();
        value.push_multi(self)?;
        let top = guard.top();
        self.to_multi_balance(guard, top)
    }
}

pub type LuaMultiValue<'a> = MultiValue<'a>;

#[extend::ext(pub, name = MultiValueCompact)]
impl<'a> LuaMultiValue<'a> {
    fn from_vec(vec: alloc::vec::Vec<LuaValue<'a>>) -> Self {
        Self(vec)
    }
}
