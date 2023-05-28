//! Implementation to lua value

use alloc::borrow::Cow;
use core::ffi::c_void;

use crate::{
    convert::*,
    error::*,
    ffi::{self, lua_Integer, lua_Number},
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

    #[inline]
    pub fn to_safe_bytes(&self) -> Option<&'a [u8]> {
        self.state.to_safe_bytes(self.index)
    }

    #[inline]
    pub fn to_safe_str(&self) -> Option<&'a str> {
        self.to_safe_bytes()
            .and_then(|b| core::str::from_utf8(b).ok())
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
    pub fn index(&self) -> Index {
        self.index
    }

    #[inline(always)]
    pub fn check_type(&self, ty: Type) -> Result<()> {
        self.state.check_type(self.index, ty)
    }

    #[inline]
    pub fn cast<'v, T: FromLua<'a> + 'v>(&'v self) -> Option<T> {
        self.state.arg(self.index)
    }

    #[inline]
    pub fn check_cast<T: FromLua<'a>>(&self) -> Result<T> {
        FromLua::check(self.state, self.index)
    }

    pub(crate) fn getf(&self, k: &CStr) -> ValRef {
        self.state.get_field(self.index, k);
        self.state.top_val()
    }

    #[inline]
    pub(crate) fn setf<V: ToLua>(&self, k: &CStr, v: V) -> Result<()> {
        self.state.push(v)?;
        self.state.set_field(self.index, k);
        Ok(())
    }

    #[inline]
    pub fn geti(&self, i: impl Into<lua_Integer>) -> Result<ValRef<'a>> {
        if self.has_metatable() {
            unsafe extern "C" fn protect_get(l: *mut ffi::lua_State) -> i32 {
                ffi::lua_geti(l, 1, ffi::lua_tointeger(l, 2));
                1
            }
            self.state
                .protect_call((ArgRef(self.index), i.into()), protect_get)
        } else {
            self.state.geti(self.index, i.into());
            Ok(self.state.top_val())
        }
    }

    #[inline]
    pub fn seti<V: ToLua>(&self, i: impl Into<lua_Integer>, v: V) -> Result<()> {
        if self.has_metatable() {
            unsafe extern "C" fn protect_set(l: *mut ffi::lua_State) -> i32 {
                ffi::lua_seti(l, 1, ffi::lua_tointeger(l, 2));
                0
            }
            self.state
                .protect_call((ArgRef(self.index), i.into(), v), protect_set)
        } else {
            self.state.push(v)?;
            self.state.seti(self.index, i.into());
            Ok(())
        }
    }

    /// Get length of the value, like `return #self` in lua
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

    /// Set value, equivalent to `self[k] = v` in lua
    #[inline]
    pub fn set<K: ToLua, V: ToLua>(&self, k: K, v: V) -> Result<()> {
        if self.has_metatable() {
            unsafe extern "C" fn protect_set(l: *mut ffi::lua_State) -> i32 {
                ffi::lua_settable(l, 1);
                0
            }
            self.state
                .protect_call((ArgRef(self.index), k, v), protect_set)
        } else {
            self.state.push(k)?;
            self.state.push(v)?;
            self.state.set_table(self.index);
            Ok(())
        }
    }

    /// Get value associated, equivalent to  `return self[k]` in lua
    #[inline]
    pub fn get<K: ToLua>(&self, k: K) -> Result<ValRef<'a>> {
        if self.has_metatable() {
            unsafe extern "C" fn protect_get(l: *mut ffi::lua_State) -> i32 {
                ffi::lua_gettable(l, 1);
                1
            }
            self.state
                .protect_call((ArgRef(self.index), k), protect_get)
        } else {
            self.state.push(k)?;
            self.state.get_table(self.index);
            Ok(self.state.top_val())
        }
    }

    #[inline]
    pub fn getopt<K: ToLua, V: FromLua<'a>>(&self, k: K) -> Result<Option<V>> {
        Ok(self.get(k)?.cast())
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
        let result = self.state.get_metatable(self.index);
        if result {
            self.state.pop(1);
        }
        result
    }

    /// Get metatable of lua table or userdata
    pub fn metatable(&self) -> Result<Option<Table<'a>>> {
        Ok(if self.state.get_metatable(self.index) {
            Some(self.state.top_val().try_into()?)
        } else {
            None
        })
    }

    /// Set metatable for lua table or userdata
    pub fn set_metatable(&self, t: Table) -> Result<()> {
        self.state.pushval(t.0);
        self.state.set_metatable(self.index);
        Ok(())
    }

    /// Remove metatable for lua table or userdata
    pub fn remove_metatable(&self) {
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

    pub fn checked_into_value(self) -> Option<Value<'a>> {
        Some(match self.type_of() {
            Type::None | Type::Invalid => return None,
            Type::Nil => Value::Nil,
            Type::Number => {
                if self.is_integer() {
                    Value::Integer(self.cast().unwrap())
                } else {
                    Value::Number(self.cast().unwrap())
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
                    Value::Integer(self.cast().unwrap())
                } else {
                    Value::Number(self.cast().unwrap())
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
    #[inline(always)]
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
        self.key.take().expect("next key must exists").ensure_top();
        let t = self.val.as_ref();
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
            #[inline(always)]
            fn from_index(s: &'a State, i: Index) -> Option<Self> {
                let val = s.val(i);
                (val.type_of() == $lt).then_some(Self(val))
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
    pub fn getp<T>(&self, p: *const T) -> ValRef {
        self.state.raw_getp(self.index, p);
        self.state.top_val()
    }

    /// Set value with a lightuserdata key
    #[inline]
    pub fn setp<T, V: ToLua>(&self, k: *const T, v: V) -> Result<()> {
        self.state.push(v)?;
        self.state.raw_setp(self.index, k);
        Ok(())
    }

    #[inline]
    pub fn reference<V: ToLua>(&self, v: V) -> Result<Reference> {
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
    pub fn raw_geti(&self, i: impl Into<lua_Integer>) -> ValRef<'l> {
        self.state.raw_geti(self.index, i.into());
        self.state.top_val()
    }

    /// Set value by number index without metamethod triggers
    #[inline]
    pub fn raw_seti<V: ToLua>(&self, i: impl Into<lua_Integer>, v: V) -> Result<()> {
        self.state.push(v)?;
        self.state.raw_seti(self.index, i.into());
        Ok(())
    }

    /// Get the value associated to `key` without metamethod triggers
    #[inline]
    pub fn raw_get<K: ToLua>(&self, key: K) -> Result<ValRef<'l>> {
        self.state.check_stack(3)?;
        self.state.push(key)?;
        self.state.raw_get(self.index);
        Ok(self.state.top_val())
    }

    /// Set value by any key without metamethod triggers
    #[inline]
    pub fn raw_set<K: ToLua, V: ToLua>(&self, k: K, v: V) -> Result<()> {
        // TODO: protect call?
        self.state.check_stack(3)?;
        self.state.push(k)?;
        self.state.push(v)?;
        self.state.raw_set(self.index);
        Ok(())
    }

    /// Get length of the table without metamethod triggers
    #[inline]
    pub fn raw_len(&self) -> usize {
        self.state.raw_len(self.index)
    }

    #[inline(always)]
    pub fn raw_insert<V: ToLua>(&self, i: usize, val: V) -> Result<()> {
        self.raw_move_vals(i)?;
        self.raw_seti(i as i64, val)
    }

    pub fn raw_move_vals(&self, i: usize) -> Result<()> {
        for i in i..=self.raw_len() {
            self.raw_seti((i + 1) as i64, self.raw_get(i as i64)?)?;
        }
        Ok(())
    }

    #[inline(always)]
    pub fn push<V: ToLua>(&self, val: V) -> Result<()> {
        self.raw_seti((self.raw_len() + 1) as i64, val)
    }

    #[inline(always)]
    pub fn pairs(&self) -> Result<impl Iterator<Item = (Value, Value)>> {
        Ok(self.iter()?.map(|(k, v)| (k.into_value(), v.into_value())))
    }

    #[inline(always)]
    pub fn set_closure<
        'a,
        K: ToLua,
        A: FromLuaMulti<'a> + core::marker::Tuple,
        R: ToLuaMulti + 'a,
        F: Fn<A, Output = R> + 'static,
    >(
        &self,
        k: K,
        v: F,
    ) -> Result<&Self> {
        self.raw_set(k, self.state.new_closure(v)?).map(|_| self)
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

    #[inline(always)]
    pub fn userdata_ref<U: UserData>(&self) -> Option<&'a U::Trans> {
        unsafe {
            self.state
                .test_userdata_meta::<U::Trans>(self.index, crate::userdata::init_wrapper::<U>)
                .map(|x| x as _)
        }
    }

    #[inline(always)]
    pub unsafe fn userdata_ref_mut<U: UserData>(&self) -> Option<&mut U::Trans> {
        self.state
            .test_userdata_meta::<U::Trans>(self.index, crate::userdata::init_wrapper::<U>)
    }
}
