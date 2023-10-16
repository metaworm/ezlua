//! Implementation to lua value

use alloc::{borrow::Cow, vec::Vec};
use core::ffi::{c_char, c_void};
use core::ops;

use crate::{
    convert::{Index, *},
    error::*,
    ffi::{self, lua_Integer, lua_Number, lua_tostring},
    luaapi::{Reference, Type, UnsafeLuaApi},
    marker::RegVal,
    prelude::ArgRef,
    state::*,
    str::CStr,
    userdata::UserData,
};

/// Value reference on the lua stack
pub struct ValRef<'a> {
    pub(crate) state: &'a State,
    pub(crate) index: Index,
}

impl Clone for ValRef<'_> {
    #[inline(always)]
    fn clone(&self) -> Self {
        self.state.val(self.index)
    }
}

impl<'a> core::fmt::Debug for ValRef<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut ds = f.debug_struct("ValRef");
        ds.field("index", &self.index)
            .field("type", &self.type_of());
        match self.type_of() {
            Type::Boolean => ds.field("value", &self.to_bool()),
            Type::Userdata | Type::LightUserdata => {
                ds.field("value", &self.state.to_userdata(self.index))
            }
            Type::Number => ds.field("value", &self.to_number()),
            Type::String => ds.field("value", &self.to_string_lossy().unwrap_or_default()),
            Type::Table | Type::Thread | Type::Function => {
                ds.field("value", &self.state.to_pointer(self.index))
            }
            _ => ds.field("value", &()),
        }
        .finish()
    }
}

impl Drop for ValRef<'_> {
    #[track_caller]
    fn drop(&mut self) {
        self.state.drop_valref(self);
    }
}

impl<'a> ValRef<'a> {
    #[inline]
    pub fn state(&self) -> &'a State {
        self.state
    }

    /// Type of this value
    #[inline]
    pub fn type_of(&self) -> Type {
        self.state.type_of(self.index)
    }

    #[inline]
    pub fn is_nil(&self) -> bool {
        self.state.is_nil(self.index)
    }

    #[inline]
    pub fn is_integer(&self) -> bool {
        self.state.is_integer(self.index)
    }

    #[inline]
    pub fn is_table(&self) -> bool {
        self.state.is_table(self.index)
    }

    #[inline]
    pub fn is_function(&self) -> bool {
        self.state.is_function(self.index)
    }

    pub fn check_safe_index(&self) -> Result<()> {
        if self.state.safe_index(self.index) {
            Ok(())
        } else {
            Err("ref is not safe").lua_result()
        }
    }

    pub fn to_safe_bytes(&self) -> Result<&'a [u8]> {
        self.check_type(Type::String)?;
        self.state
            .to_safe_bytes(self.index)
            .ok_or_else(|| Error::Convert("safe bytes".into()))
    }

    #[inline]
    pub fn to_safe_str(&self) -> Result<&'a str> {
        core::str::from_utf8(self.to_safe_bytes()?).lua_result()
    }

    #[inline]
    pub fn to_bytes(&self) -> Option<&[u8]> {
        self.check_type(Type::String).ok()?;
        self.state.to_bytes(self.index)
    }

    #[inline]
    pub fn to_str(&self) -> Option<&str> {
        self.check_type(Type::String).ok()?;
        self.state.to_str(self.index)
    }

    #[inline]
    pub fn to_string_lossy(&self) -> Option<Cow<str>> {
        self.state.to_string_lossy(self.index)
    }

    #[inline]
    pub fn to_bool(&self) -> bool {
        self.state.to_bool(self.index)
    }

    #[inline]
    pub fn to_integer(&self) -> lua_Integer {
        self.state.to_integer(self.index)
    }

    #[inline]
    pub fn to_number(&self) -> lua_Number {
        self.state.to_number(self.index)
    }

    #[inline]
    pub fn to_pointer(&self) -> *const c_void {
        self.state.to_pointer(self.index)
    }

    #[inline]
    pub fn to_cstr_ptr(&self) -> *const c_char {
        unsafe { lua_tostring(self.state.state, self.index) }
    }

    /// Index number of this value on the lua stack
    #[inline]
    pub fn index(&self) -> Index {
        self.index
    }

    #[inline(always)]
    pub fn check_type(&self, ty: Type) -> Result<()> {
        self.state.check_type(self.index, ty)
    }

    pub fn check_type2(&self, ty1: Type, ty2: Type) -> Result<()> {
        let ty = self.type_of();
        if ty == ty1 || ty == ty2 {
            Ok(())
        } else {
            Err(Error::TypeNotMatch(ty))
        }
    }

    /// Cast a lua value to its rust type, wrapper of [`FromLua::from_lua`]
    ///
    /// See [`FromLua`]
    #[inline(always)]
    pub fn cast_into<T: FromLua<'a> + 'a>(self) -> Result<T> {
        FromLua::from_lua(self.state, self)
    }

    /// Alias to `cast_into()`, not take the ownship, but only convert to static-lifetime types
    #[inline]
    pub fn cast<T: FromLua<'a> + 'static>(&self) -> Result<T> {
        self.clone().cast_into()
    }

    pub(crate) fn getf(&self, k: &CStr) -> ValRef {
        self.state.check_stack(1).expect("stack");
        self.state.get_field(self.index, k);
        self.state.top_val()
    }

    #[inline]
    pub(crate) fn setf<V: ToLua>(&self, k: &CStr, v: V) -> Result<()> {
        self.state.check_stack(1)?;
        self.state.push(v)?;
        self.state.set_field(self.index, k);
        Ok(())
    }

    /// Get value associated to integer key, equivalent to `return self[i]` in lua
    pub fn geti(&self, i: impl Into<lua_Integer>) -> Result<ValRef<'a>> {
        if self.has_metatable() {
            unsafe extern "C" fn protect_get(l: *mut ffi::lua_State) -> i32 {
                ffi::lua_geti(l, 1, ffi::lua_tointeger(l, 2));
                1
            }
            self.state
                .protect_call((ArgRef(self.index), i.into()), protect_get)
        } else {
            self.check_type2(Type::Table, Type::Userdata)?;
            self.state.check_stack(1)?;
            self.state.geti(self.index, i.into());
            Ok(self.state.top_val())
        }
    }

    /// Set value with integer key, equivalent to `self[i] = v` in lua
    pub fn seti<V: ToLua>(&self, i: impl Into<lua_Integer>, v: V) -> Result<()> {
        if self.has_metatable() {
            unsafe extern "C" fn protect_set(l: *mut ffi::lua_State) -> i32 {
                ffi::lua_seti(l, 1, ffi::lua_tointeger(l, 2));
                0
            }
            self.state
                .protect_call((ArgRef(self.index), i.into(), v), protect_set)
        } else {
            self.check_type2(Type::Table, Type::Userdata)?;
            self.state.check_stack(1)?;
            self.state.push(v)?;
            self.state.seti(self.index, i.into());
            Ok(())
        }
    }

    /// Get length of the value, equivalent to `return #self` in lua
    #[inline]
    pub fn len(&self) -> Result<ValRef<'a>> {
        if self.has_metatable() {
            unsafe extern "C" fn protect(l: *mut ffi::lua_State) -> i32 {
                ffi::lua_len(l, 1);
                0
            }
            self.state.protect_call(ArgRef(self.index), protect)
        } else {
            self.state.len(self.index);
            Ok(self.state.top_val())
        }
    }

    /// Set value with any key, equivalent to `self[k] = v` in lua
    pub fn set<K: ToLua, V: ToLua>(&self, k: K, v: V) -> Result<()> {
        if self.has_metatable() {
            unsafe extern "C" fn protect_set(l: *mut ffi::lua_State) -> i32 {
                ffi::lua_settable(l, 1);
                0
            }
            self.state
                .protect_call((ArgRef(self.index), k, v), protect_set)
        } else {
            self.as_table()
                .ok_or_else(|| Error::TypeNotMatch(self.type_of()))?
                .raw_set(k, v)
        }
    }

    /// Get value associated to key, equivalent to `return self[k]` in lua
    pub fn get<K: ToLua>(&self, key: K) -> Result<ValRef<'a>> {
        if self.has_metatable() {
            unsafe extern "C" fn protect_get(l: *mut ffi::lua_State) -> i32 {
                ffi::lua_gettable(l, 1);
                1
            }
            self.state
                .protect_call((ArgRef(self.index), key), protect_get)
        } else {
            self.as_table()
                .ok_or_else(|| Error::TypeNotMatch(self.type_of()))?
                .raw_get(key)
        }
    }

    #[inline]
    pub fn getopt<K: ToLua, V: FromLua<'a> + 'a>(&self, k: K) -> Result<Option<V>> {
        Ok(self.get(k)?.cast_into().ok())
    }

    /// Call this value as a function
    #[inline(always)]
    pub fn pcall<T: ToLuaMulti, R: FromLuaMulti<'a>>(&self, args: T) -> Result<R> {
        self.state.pcall_trace(ArgRef(self.index), args)
    }

    /// Invoke `pcall()` without return value
    #[inline(always)]
    pub fn pcall_void<T: ToLuaMulti>(&self, args: T) -> Result<()> {
        self.pcall(args)
    }

    pub fn has_metatable(&self) -> bool {
        let result = self.state.check_stack(1).is_ok() && self.state.get_metatable(self.index);
        if result {
            self.state.pop(1);
        }
        result
    }

    /// Get metatable of lua table or userdata
    pub fn metatable(&self) -> Result<Option<Table<'a>>> {
        self.state.check_stack(1)?;
        Ok(if self.state.get_metatable(self.index) {
            Some(self.state.top_val().try_into()?)
        } else {
            None
        })
    }

    /// Set metatable for lua table or userdata
    pub fn set_metatable(&self, t: Table) -> Result<()> {
        self.state.check_stack(1)?;
        self.state.pushval(t.0);
        self.state.set_metatable(self.index);
        Ok(())
    }

    /// Remove metatable for lua table or userdata
    pub fn remove_metatable(&self) {
        self.state.check_stack(1).expect("check");
        // TODO: thread lock
        self.state.push_nil();
        self.state.set_metatable(self.index);
    }

    /// Call a metamethod
    #[inline(always)]
    pub fn call_metamethod<T: ToLuaMulti, R: FromLuaMulti<'a>>(
        &self,
        m: &str,
        args: T,
    ) -> Result<R> {
        self.metatable()?
            .ok_or_else(|| Error::runtime("no metatable"))?
            .raw_get(m)?
            .pcall(args)
    }

    /// Close this value, if userdata, the subsequent access to it in lua is invalid
    #[inline(always)]
    pub fn close(self) -> Result<()> {
        self.call_metamethod("__close", ArgRef(self.index))
    }

    /// Tests whether two lua values are equal without metamethod triggers
    pub fn raw_equal(&self, other: &Self) -> bool {
        self.state.raw_equal(self.index, other.index)
    }

    /// Get length of the string/userdata/table without metamethod triggers
    #[inline]
    pub fn raw_len(&self) -> usize {
        self.state.raw_len(self.index)
    }

    pub fn checked_into_value(self) -> Option<Value<'a>> {
        Some(match self.type_of() {
            Type::None | Type::Invalid => return None,
            Type::Nil => Value::Nil,
            Type::Number => {
                if self.is_integer() {
                    Value::Integer(self.to_integer())
                } else {
                    Value::Number(self.to_number())
                }
            }
            Type::Boolean => Value::Bool(self.to_bool()),
            Type::LightUserdata => Value::LightUserdata(self.state.to_userdata(self.index)),
            Type::String => Value::String(LuaString(self)),
            Type::Table => Value::Table(Table(self)),
            Type::Function => Value::Function(Function(self)),
            Type::Userdata => Value::UserData(LuaUserData(self)),
            Type::Thread => Value::Thread(LuaThread(self)),
        })
    }

    pub fn into_registry_value(self) -> Result<RegVal> {
        let s = self.state;
        s.registry_value(self)
    }

    pub fn into_value(self) -> Value<'a> {
        match self.type_of() {
            Type::None | Type::Invalid => Value::None,
            Type::Nil => Value::Nil,
            Type::Number => {
                if self.is_integer() {
                    Value::Integer(self.to_integer())
                } else {
                    Value::Number(self.to_number())
                }
            }
            Type::Boolean => Value::Bool(self.to_bool()),
            Type::LightUserdata => Value::LightUserdata(self.state.to_userdata(self.index)),
            Type::String => Value::String(LuaString(self)),
            Type::Table => Value::Table(Table(self)),
            Type::Function => Value::Function(Function(self)),
            Type::Userdata => Value::UserData(LuaUserData(self)),
            Type::Thread => Value::Thread(LuaThread(self)),
        }
    }

    /// Return Some(self) if type is neither Type::Invalid nor Type::None
    pub fn check_valid(self) -> Option<Self> {
        match self.type_of() {
            Type::Invalid | Type::None => None,
            _ => Some(self),
        }
    }

    /// [+(0|1), 0, -]
    pub(crate) fn ensure_top(self) {
        if self.index < self.state.get_top() {
            self.state.push_value(self.index);
        } else {
            debug_assert!(self.index == self.state.get_top());
            core::mem::forget(self);
        }
    }
}

/// Iterator for table traversing, like `pairs` in lua
pub struct TableIter<'a, V: AsRef<Table<'a>>> {
    val: V,
    key: Option<ValRef<'a>>,
}

impl<'a, V: AsRef<Table<'a>>> Iterator for TableIter<'a, V> {
    type Item = (ValRef<'a>, ValRef<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        let t = self.val.as_ref();
        t.state.check_stack(3).expect("stack");
        self.key.take().expect("next key must exists").ensure_top();
        if t.state.next(t.index) {
            let (k, val) = if let Some(val) = t.state.try_replace_top() {
                (val.state.top_val(), val)
            } else {
                (t.state.val_without_push(-2), t.state.val_without_push(-1))
            };
            let key = k.clone();
            self.key.replace(k);
            Some((key, val))
        } else {
            None
        }
    }
}

/// Typed enumeration for lua value
#[derive(Debug, Clone, Default)]
pub enum Value<'a> {
    None,
    #[default]
    Nil,
    Bool(bool),
    Integer(lua_Integer),
    Number(lua_Number),
    LightUserdata(*mut c_void),
    String(LuaString<'a>),
    Table(Table<'a>),
    Function(Function<'a>),
    UserData(LuaUserData<'a>),
    Thread(LuaThread<'a>),
}

#[cfg(feature = "unsafe_send_sync")]
unsafe impl Send for Value<'_> {}
#[cfg(feature = "unsafe_send_sync")]
unsafe impl Sync for Value<'_> {}

impl<'a> Value<'a> {
    pub fn light_userdata<T: Sized>(p: *const T) -> Self {
        Value::LightUserdata(p as usize as _)
    }
}

/// Represents a lua table on the stack
#[derive(Debug, Clone, derive_more::Deref)]
pub struct Table<'l>(pub(crate) ValRef<'l>);

/// Represents a lua function on the stack
#[derive(Debug, Clone, derive_more::Deref)]
pub struct Function<'l>(pub(crate) ValRef<'l>);

/// Represents a lua string on the stack
#[derive(Debug, Clone, derive_more::Deref)]
pub struct LuaString<'l>(pub(crate) ValRef<'l>);

/// Represents a lua thread on the stack
#[derive(Debug, Clone, derive_more::Deref)]
pub struct LuaThread<'l>(pub(crate) ValRef<'l>);

/// Represents a lua userdata on the stack
#[derive(Debug, Clone, derive_more::Deref)]
pub struct LuaUserData<'l>(pub(crate) ValRef<'l>);

macro_rules! impl_wrap {
    ($t:ty, $lt:expr, $m:ident) => {
        impl<'a> TryFrom<ValRef<'a>> for $t {
            type Error = crate::error::Error;

            fn try_from(val: ValRef<'a>) -> Result<Self> {
                let t = val.type_of();
                if t == $lt {
                    Ok(Self(val))
                } else {
                    Err(Error::TypeNotMatch(t))
                }
            }
        }

        // impl<'a> From<ValRef<'a>> for $t {
        //     fn from(val: ValRef<'a>) -> Self {
        //         assert_eq!(val.type_of(), $lt);
        //         Self(val)
        //     }
        // }

        impl<'a> Into<ValRef<'a>> for $t {
            fn into(self) -> ValRef<'a> {
                self.0
            }
        }

        impl<'a> ToLua for $t {
            const __PUSH: Option<fn(Self, &State) -> Result<()>> =
                Some(|this, s: &State| Ok(s.pushval(this.0)));
        }

        impl<'a> FromLua<'a> for $t {
            fn from_lua(_: &'a State, val: ValRef<'a>) -> Result<Self> {
                val.check_type($lt).map(|_| Self(val))
            }
        }

        impl<'a> ValRef<'a> {
            pub fn $m(&self) -> Option<&$t> {
                if self.type_of() == $lt {
                    unsafe { (self as *const _ as *const $t).as_ref() }
                } else {
                    None
                }
            }
        }
    };
}

impl_wrap!(Table<'a>, Type::Table, as_table);
impl_wrap!(Function<'a>, Type::Function, as_function);
impl_wrap!(LuaString<'a>, Type::String, as_string);
impl_wrap!(LuaThread<'a>, Type::Thread, as_thread);
impl_wrap!(LuaUserData<'a>, Type::Userdata, as_userdata);

impl<'l> Table<'l> {
    /// Get value with a lightuserdata key, commonly is a function pointer
    #[inline]
    pub fn getp<T>(&self, p: *const T) -> Result<ValRef> {
        self.state.check_stack(1)?;
        self.state.raw_getp(self.index, p);
        Ok(self.state.top_val())
    }

    /// Set value with a lightuserdata key
    #[inline]
    pub fn setp<T, V: ToLua>(&self, k: *const T, v: V) -> Result<()> {
        self.state.check_stack(1)?;
        self.state.push(v)?;
        self.state.raw_setp(self.index, k);
        Ok(())
    }

    #[inline]
    pub fn reference<V: ToLua>(&self, v: V) -> Result<Reference> {
        self.state.check_stack(1)?;
        self.state.push(v)?;
        Ok(self.state.reference(self.index))
    }

    #[inline]
    pub fn unreference(&self, r: Reference) {
        self.state.unreference(self.index, r);
    }

    /// Count of the table entries
    pub fn entry_count(&self) -> usize {
        let mut count = 0usize;
        self.state.push_nil();
        while self.state.next(self.index) {
            count += 1;
            self.state.pop(1);
        }
        count
    }

    /// Iterator to the table entries
    pub fn iter<'t>(&'t self) -> Result<TableIter<'l, &'t Self>> {
        Ok(TableIter {
            val: self,
            key: Some(self.state.new_val(())?),
        })
    }

    /// Like `iter()`, but take the ownership
    pub fn into_iter(self) -> Result<TableIter<'l, Self>> {
        let key = self.state.new_val(())?;
        Ok(TableIter {
            val: self,
            key: Some(key),
        })
    }

    /// Get value by number index without metamethod triggers
    #[inline]
    pub fn raw_geti(&self, i: impl Into<lua_Integer>) -> Result<ValRef<'l>> {
        self.state.check_stack(1)?;
        self.state.raw_geti(self.index, i.into());
        Ok(self.state.top_val())
    }

    /// Set value by number index without metamethod triggers
    #[inline]
    pub fn raw_seti<V: ToLua>(&self, i: impl Into<lua_Integer>, v: V) -> Result<()> {
        self.state.check_stack(2)?;
        self.state.push(v)?;
        self.state.raw_seti(self.index, i.into());
        Ok(())
    }

    pub fn raw_take_ref(&self, r: Reference) -> Result<ValRef<'l>> {
        let res = self.raw_geti(r.0)?;
        self.raw_seti(r.0, ())?;
        Ok(res)
    }

    /// Get the value associated to `key` without metamethod triggers
    #[inline]
    pub fn raw_get<K: ToLua>(&self, key: K) -> Result<ValRef<'l>> {
        self.state.check_stack(2)?;
        self.state.push(key)?;
        self.state.check_nil_pop()?;
        self.state.raw_get(self.index);
        Ok(self.state.top_val())
    }

    /// Set value by any key without metamethod triggers
    #[inline]
    pub fn raw_set<K: ToLua, V: ToLua>(&self, k: K, v: V) -> Result<()> {
        self.state.check_stack(3)?;
        self.state.push(k)?;
        self.state.check_nil_pop()?;
        self.state.push(v)?;
        self.state.raw_set(self.index);
        Ok(())
    }

    /// Insert an element into the array table, equivalent to `table.insert` in lua
    #[inline(always)]
    pub fn raw_insert<V: ToLua>(&self, i: usize, val: V) -> Result<()> {
        self.raw_move_vals(i)?;
        self.raw_seti(i as i64, val)
    }

    #[doc(hidden)]
    pub fn raw_move_vals(&self, i: usize) -> Result<()> {
        for i in i..=self.raw_len() {
            self.raw_seti((i + 1) as i64, self.raw_get(i as i64)?)?;
        }
        Ok(())
    }

    /// Push an element to end of the array part of a table, alias to `self.raw_seti((self.raw_len() + 1) as i64, val)`
    #[inline(always)]
    pub fn push<V: ToLua>(&self, val: V) -> Result<()> {
        self.raw_seti((self.raw_len() + 1) as i64, val)
    }

    /// Iterator to the table entries
    #[inline(always)]
    pub fn pairs(&self) -> Result<impl Iterator<Item = (Value, Value)>> {
        Ok(self.iter()?.map(|(k, v)| (k.into_value(), v.into_value())))
    }

    /// Alias to `self.set(name, lua.new_closure(func))`
    #[inline(always)]
    pub fn set_closure<'a, K: ToLua, A: 'a, R: 'a, F: LuaMethod<'a, (), A, R> + 'static>(
        &self,
        name: K,
        func: F,
    ) -> Result<&Self> {
        self.raw_set(name, self.state.new_closure(func)?)
            .map(|_| self)
    }
}

impl<'a> Function<'a> {
    #[inline]
    pub fn get_upvalue(&self, i: Index) -> Result<Option<ValRef<'a>>> {
        Ok(self
            .state
            .get_upvalue(self.index, i)
            .map(|_| self.state.top_val()))
    }

    #[inline]
    pub fn get_upvalue_name(&self, i: Index) -> Result<Option<&'a str>> {
        Ok(self.state.get_upvalue(self.index, i))
    }

    #[inline]
    pub fn set_upvalue(&self, i: Index, val: impl ToLua) -> Result<()> {
        self.state.push(val)?;
        self.state.set_upvalue(self.index, i);
        Ok(())
    }

    /// Dumps the function as a binary chunk.
    ///
    /// If `strip` is true, the binary representation may not include all debug information
    /// about the function, to save space.
    pub fn dump(&self, strip: bool) -> Vec<u8> {
        let mut data: Vec<u8> = Vec::new();
        self.state.dump(|buf| data.extend_from_slice(buf), strip);
        data
    }
}

impl<'a> AsRef<Table<'a>> for Table<'a> {
    fn as_ref(&self) -> &Table<'a> {
        self
    }
}

impl<'a> LuaString<'a> {
    #[inline]
    pub fn to_string_lossy(&self) -> Cow<str> {
        self.state.to_string_lossy(self.index).unwrap_or_default()
    }
}

impl<'a> LuaUserData<'a> {
    /// Set uservalue
    #[inline]
    pub fn set_uservalue<V: ToLua>(&self, v: V) -> Result<()> {
        self.state.push(v)?;
        self.state.set_uservalue(self.index);
        Ok(())
    }

    /// Get the uservalue stored in uservalue
    #[inline]
    pub fn get_uservalue(&self) -> Result<ValRef<'a>> {
        self.state.get_uservalue(self.index);
        Ok(self.state.top_val())
    }

    /// Set n-th uservalue
    #[inline]
    pub fn set_iuservalue<V: ToLua>(&self, n: i32, v: V) -> Result<()> {
        self.state.push(v)?;
        self.state.set_iuservalue(self.index, n);
        Ok(())
    }

    /// Get n-th uservalue stored in uservalue
    #[inline]
    pub fn get_iuservalue(&self, n: i32) -> Result<ValRef<'a>> {
        self.state.get_iuservalue(self.index, n);
        Ok(self.state.top_val())
    }

    /// Take the ownership, and subsequent access in lua is invalid
    pub fn take<U: UserData>(self) -> Option<U::Trans> {
        unsafe {
            self.userdata_ref::<U>().map(|p| {
                self.remove_metatable();
                core::ptr::read(p)
            })
        }
    }

    pub fn userdata_pointer(&self) -> *mut c_void {
        self.state.to_userdata(self.index)
    }

    pub unsafe fn userdata_bytes(&self) -> &[u8] {
        core::slice::from_raw_parts(self.userdata_pointer().cast::<u8>(), self.raw_len())
    }

    pub unsafe fn get_ref_unchecked<U: UserData>(&self) -> Option<&mut U::Trans> {
        self.state
            .to_userdata(self.index)
            .cast::<U::Trans>()
            .as_mut()
    }

    pub fn userdata_ref<U: UserData>(&self) -> Option<&U::Trans> {
        unsafe {
            self.state
                .test_userdata_meta::<U::Trans>(self.index, U::metatable_key())
                .map(|x| x as _)
        }
    }

    pub unsafe fn userdata_ref_mut<U: UserData>(&self) -> Option<&mut U::Trans> {
        self.state
            .test_userdata_meta::<U::Trans>(self.index, U::metatable_key())
    }
}

macro_rules! protect_airth {
    ($op:expr) => {{
        unsafe extern "C" fn protect(l: *mut ffi::lua_State) -> i32 {
            ffi::lua_arith(l, $op);
            1
        }
        protect
    }};
}

macro_rules! impl_binop {
    ($t:ident, $trait:ty, $name:ident, $method:ident, $op:expr) => {
        #[inline]
        pub fn $method(&self, rhs: impl ToLua) -> Result<Self> {
            impl<'l, $t: ToLua> $trait for &ValRef<'l> {
                type Output = Result<ValRef<'l>>;

                fn $name(self, rhs: T) -> Self::Output {
                    self.$method(rhs)
                }
            }

            impl<'l, $t: ToLua> $trait for ValRef<'l> {
                type Output = Result<ValRef<'l>>;

                fn $name(self, rhs: T) -> Self::Output {
                    self.$method(rhs)
                }
            }

            self.state.protect_call((self, rhs), protect_airth!($op))
        }
    };
}

macro_rules! impl_op {
    ($trait:ty, $name:ident, $method:ident, $op:expr) => {
        #[inline]
        pub fn $method(&self) -> Result<Self> {
            impl<'l> $trait for &ValRef<'l> {
                type Output = Result<ValRef<'l>>;

                fn $name(self) -> Self::Output {
                    self.$method()
                }
            }

            impl<'l> $trait for ValRef<'l> {
                type Output = Result<ValRef<'l>>;

                fn $name(self) -> Self::Output {
                    self.$method()
                }
            }

            self.state.protect_call(self, protect_airth!($op))
        }
    };
}

impl ValRef<'_> {
    impl_binop!(T, ops::Add<T>, add, airth_add, ffi::LUA_OPADD);
    impl_binop!(T, ops::Sub<T>, sub, airth_sub, ffi::LUA_OPSUB);
    impl_binop!(T, ops::Mul<T>, mul, airth_mul, ffi::LUA_OPMUL);
    impl_binop!(T, ops::Div<T>, div, airth_div, ffi::LUA_OPDIV);
    impl_binop!(T, ops::Rem<T>, rem, airth_rem, ffi::LUA_OPMOD);
    impl_binop!(T, ops::BitAnd<T>, bitand, airth_bitand, ffi::LUA_OPBAND);
    impl_binop!(T, ops::BitOr<T>, bitor, airth_bitor, ffi::LUA_OPBOR);
    impl_binop!(T, ops::BitXor<T>, bitxor, airth_bitxor, ffi::LUA_OPBXOR);
    impl_binop!(T, ops::Shl<T>, shl, airth_shl, ffi::LUA_OPSHL);
    impl_binop!(T, ops::Shr<T>, shr, airth_shr, ffi::LUA_OPSHR);

    impl_op!(ops::Neg, neg, arith_neg, ffi::LUA_OPUNM);
    impl_op!(ops::Not, not, arith_not, ffi::LUA_OPBNOT);

    pub fn idiv(&self, rhs: impl ToLua) -> Result<Self> {
        self.state
            .protect_call((self, rhs), protect_airth!(ffi::LUA_OPIDIV))
    }

    pub fn pow(&self, rhs: impl ToLua) -> Result<Self> {
        self.state
            .protect_call((self, rhs), protect_airth!(ffi::LUA_OPPOW))
    }
}

macro_rules! protect_compare {
    ($op:expr) => {{
        unsafe extern "C" fn protect(l: *mut ffi::lua_State) -> i32 {
            ffi::lua_pushboolean(l, ffi::lua_compare(l, 1, 2, $op));
            1
        }
        protect
    }};
}

impl PartialEq for ValRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.state
            .protect_call((self, other), protect_compare!(ffi::LUA_OPEQ))
            .unwrap_or_default()
    }
}
