//! Implementation to lua value

use alloc::borrow::Cow;
use core::ffi::c_void;

use crate::{
    convert::*,
    error::*,
    ffi::{lua_Integer, lua_Number},
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

impl<'a> AsRef<ValRef<'a>> for ValRef<'a> {
    fn as_ref(&self) -> &ValRef<'a> {
        self
    }
}

impl Clone for ValRef<'_> {
    fn clone(&self) -> Self {
        self.state.val(self.index)
    }
}

impl<'a> core::fmt::Debug for ValRef<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ValRef")
            .field("index", &self.index)
            .field("type", &self.type_of())
            .finish()
    }
}

impl Drop for ValRef<'_> {
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
        self.state.to_bytes(self.index)
    }

    #[inline]
    pub fn to_str(&self) -> Option<&str> {
        self.state.to_str(self.index)
    }

    #[inline]
    pub fn to_str_lossy(&self) -> Option<Cow<str>> {
        self.state.to_str_lossy(self.index)
    }

    #[inline]
    pub fn to_string_lossy(&self) -> Cow<str> {
        self.state.to_str_lossy(self.index).unwrap_or_default()
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

    #[inline]
    pub fn geti(&self, i: impl Into<lua_Integer>) -> ValRef<'a> {
        self.state.geti(self.index, i.into());
        self.state.top_val()
    }

    #[inline]
    pub fn seti<V: ToLua>(&self, i: impl Into<lua_Integer>, v: V) -> Result<()> {
        self.state.push(v)?;
        self.state.seti(self.index, i.into());
        Ok(())
    }

    pub fn getf(&self, k: &CStr) -> ValRef {
        self.state.get_field(self.index, k);
        self.state.top_val()
    }

    #[inline]
    pub fn len(&self) -> ValRef<'a> {
        self.state.len(self.index);
        self.state.top_val()
    }

    #[inline]
    pub fn set_field(&self, k: &CStr) {
        self.state.set_field(self.index, k);
    }

    #[inline]
    pub fn setf<V: ToLua>(&self, k: &CStr, v: V) -> Result<()> {
        self.state.push(v)?;
        self.set_field(k);
        Ok(())
    }

    #[inline]
    pub fn getp<T>(&self, p: *const T) -> ValRef {
        self.state.raw_getp(self.index, p);
        self.state.top_val()
    }

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

    #[inline]
    pub fn set<K: ToLua, V: ToLua>(&self, k: K, v: V) -> Result<()> {
        self.state.push(k)?;
        self.state.push(v)?;
        self.state.set_table(self.index);
        Ok(())
    }

    #[inline]
    pub fn get<K: ToLua>(&self, k: K) -> Result<ValRef<'a>> {
        self.state.push(k)?;
        self.state.get_table(self.index);
        Ok(self.state.top_val())
    }

    #[inline]
    pub fn getopt<K: ToLua, V: FromLua<'a>>(&self, k: K) -> Result<Option<V>> {
        Ok(self.get(k)?.cast())
    }

    #[inline(always)]
    pub fn pcall<T: ToLuaMulti, R: FromLuaMulti<'a>>(&self, args: T) -> Result<R> {
        self.state.pcall_trace(self.index, args)
    }

    #[inline(always)]
    pub fn pcall_void<T: ToLuaMulti>(&self, args: T) -> Result<()> {
        self.pcall(args)
    }

    pub fn metatable(&self) -> Result<Option<Table<'a>>> {
        Ok(if self.state.get_metatable(self.index) {
            Some(self.state.top_val().try_into().unwrap())
        } else {
            None
        })
    }

    pub fn set_metatable(&self, t: Table) -> Result<()> {
        self.state.pushval(t.0);
        self.state.set_metatable(self.index);
        Ok(())
    }

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

    #[inline(always)]
    pub fn close_userdata(self) -> Result<()> {
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

    pub fn iter<'t>(&'t self) -> Result<ValIter<'a, &'t Self>> {
        self.check_type(Type::Table)?;
        Ok(ValIter {
            val: self,
            key: Some(self.state.new_val(())?),
        })
    }

    pub fn into_iter(self) -> Result<ValIter<'a, Self>> {
        self.check_type(Type::Table)?;
        let key = self.state.new_val(())?;
        Ok(ValIter {
            val: self,
            key: Some(key),
        })
    }

    /// return Some(self) if type is neither Type::Invalid nor Type::None
    pub fn check_valid(self) -> Option<Self> {
        match self.type_of() {
            Type::Invalid | Type::None => None,
            _ => Some(self),
        }
    }

    /// [+(0|1), 0, -]
    pub(crate) fn ensure_top(self) {
        if self.index != self.state.stack_top() {
            self.state.push_value(self.index);
        } else {
            core::mem::forget(self);
        }
    }

    pub(crate) fn forgot_top(self) {
        assert_eq!(self.index, self.state.stack_top(), "forget top");
        core::mem::forget(self);
    }
}

/// Iterator for table traversing, like `pairs` in lua
pub struct ValIter<'a, V: AsRef<ValRef<'a>>> {
    val: V,
    key: Option<ValRef<'a>>,
}

impl<'a, V: AsRef<ValRef<'a>>> Iterator for ValIter<'a, V> {
    type Item = (ValRef<'a>, ValRef<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        self.key.take().expect("next key must exists").ensure_top();
        let val = self.val.as_ref();
        if val.state.next(val.index) {
            let top = val.state.get_top();
            let (k, val) = if let Some(val) = val.state.try_replace_top(top) {
                (val.state.top_val(), val)
            } else {
                (
                    val.state.val_without_push(-2),
                    val.state.val_without_push(-1),
                )
            };
            let key = k.clone();
            self.key.replace(k);
            Some((key, val))
        } else {
            None
        }
    }
}

/// Enumeration for lua value
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

#[derive(Debug, Clone, derive_more::Deref)]
pub struct Table<'l>(pub(crate) ValRef<'l>);

#[derive(Debug, Clone, derive_more::Deref)]
pub struct Function<'l>(pub(crate) ValRef<'l>);

#[derive(Debug, Clone, derive_more::Deref)]
pub struct LuaString<'l>(pub(crate) ValRef<'l>);

#[derive(Debug, Clone, derive_more::Deref)]
pub struct LuaThread<'l>(pub(crate) ValRef<'l>);

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

impl<'a> Table<'a> {
    pub fn entry_count(&self) -> usize {
        let mut count = 0usize;
        self.state.push_nil();
        while self.state.next(self.index) {
            count += 1;
            self.state.pop(1);
        }
        count
    }

    #[inline]
    pub fn raw_geti(&self, i: impl Into<lua_Integer>) -> ValRef<'a> {
        self.state.raw_geti(self.index, i.into());
        self.state.top_val()
    }

    #[inline]
    pub fn raw_seti<V: ToLua>(&self, i: impl Into<lua_Integer>, v: V) -> Result<()> {
        self.state.push(v)?;
        self.state.raw_seti(self.index, i.into());
        Ok(())
    }

    #[inline]
    pub fn raw_get<K: ToLua>(&self, k: K) -> Result<ValRef<'a>> {
        self.state.push(k)?;
        self.state.raw_get(self.index);
        Ok(self.state.top_val())
    }

    #[inline]
    pub fn raw_set<K: ToLua, V: ToLua>(&self, k: K, v: V) -> Result<()> {
        self.state.push(k)?;
        self.state.push(v)?;
        self.state.raw_set(self.index);
        Ok(())
    }

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

impl<'a> LuaUserData<'a> {
    #[inline]
    pub fn set_uservalue<V: ToLua>(&self, v: V) -> Result<()> {
        self.state.push(v)?;
        self.state.set_uservalue(self.index);
        Ok(())
    }

    #[inline]
    pub fn get_uservalue(&self) -> Result<ValRef<'a>> {
        self.state.get_uservalue(self.index);
        Ok(self.state.top_val())
    }

    #[inline]
    pub fn set_iuservalue<V: ToLua>(&self, n: i32, v: V) -> Result<()> {
        self.state.push(v)?;
        self.state.set_iuservalue(self.index, n);
        Ok(())
    }

    #[inline]
    pub fn get_iuservalue(&self, n: i32) -> Result<ValRef<'a>> {
        self.state.get_iuservalue(self.index, n);
        Ok(self.state.top_val())
    }

    #[inline(always)]
    pub fn userdata_ref<U: UserData>(&self) -> Option<&'a U::Trans> {
        unsafe {
            self.state
                .test_userdata_meta::<U::Trans>(self.index, U::INIT)
                .map(|x| x as _)
        }
    }

    #[inline(always)]
    pub unsafe fn userdata_ref_mut<U: UserData>(&self) -> Option<&mut U::Trans> {
        self.state
            .test_userdata_meta::<U::Trans>(self.index, U::INIT)
    }
}
