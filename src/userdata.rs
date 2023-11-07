//! Implementation to userdata binding

use alloc::{boxed::Box, format};
use core::{
    cell::{Ref, RefCell, RefMut},
    ffi::c_int,
    marker::PhantomData,
    mem,
    ops::{Deref, DerefMut},
};

use crate::{
    convert::*,
    error::{Error, Result, ToLuaResult},
    ffi::{
        luaL_checktype, lua_State, lua_pushinteger, lua_rawlen, lua_upvalueindex, CFunction,
        LUA_REGISTRYINDEX, LUA_TUSERDATA,
    },
    luaapi::Type,
    state::State,
    value::*,
};

unsafe fn get_weak_meta(s: &State) -> Result<()> {
    use crate::luaapi::UnsafeLuaApi;

    let top = s.get_top();
    s.push_light_userdata(get_weak_meta as usize as *mut ());
    if s.get_table(LUA_REGISTRYINDEX) != Type::Table {
        s.pop(1);
        s.create_table(0, 0);
        s.push("__mode")?;
        s.push("v")?;
        s.set_table(-3);
        s.push_light_userdata(get_weak_meta as usize as *mut ());
        s.push_value(-2);
        s.raw_set(LUA_REGISTRYINDEX);
    }
    assert_eq!(s.get_top(), top + 1);

    Ok(())
}

pub trait UserDataTrans<T: UserData>: Sized {
    type Read<'a>: Deref<Target = T>
    where
        T: 'a,
        Self: 'a;
    type Write<'a>
    where
        T: 'a;

    const INIT_USERDATA: Option<fn(&State, &mut Self)> = None;

    fn trans(udata: T) -> Self;

    fn read(&self) -> Self::Read<'_>;
}

impl<T: UserData> UserDataTrans<T> for T {
    type Read<'a> = &'a Self where T: 'a;
    type Write<'a> = &'a mut Self where T: 'a;

    fn trans(udata: T) -> Self {
        udata
    }

    fn read(&self) -> Self::Read<'_> {
        self
    }
}

impl<T: UserData> UserDataTrans<T> for RefCell<T> {
    type Read<'a> = Ref<'a, T> where T: 'a;
    type Write<'a> = RefMut<'a, T> where T: 'a;

    fn trans(udata: T) -> Self {
        RefCell::new(udata)
    }

    fn read(&self) -> Self::Read<'_> {
        self.borrow()
    }
}

impl<'a, T: UserData<Trans = RefCell<T>>> FromLua<'a> for &'a RefCell<T> {
    fn from_lua(s: &'a State, val: ValRef<'a>) -> Result<Self> {
        let u = LuaUserData::try_from(val)?;
        u.check_safe_index()?;
        u.userdata_ref::<T>()
            .ok_or("userdata not match")
            .lua_result()
            // Safety: check_safe_index
            .map(|x| unsafe { core::mem::transmute(x) })
    }
}

impl<'a, T: UserData<Trans = RefCell<T>>> FromLua<'a> for Ref<'a, T> {
    fn from_lua(s: &'a State, val: ValRef<'a>) -> Result<Self> {
        let u = LuaUserData::try_from(val)?;
        u.check_safe_index()?;
        u.userdata_ref::<T>()
            .ok_or("userdata not match")
            .lua_result()?
            .try_borrow()
            .lua_result()
            // Safety: check_safe_index
            .map(|x| unsafe { core::mem::transmute(x) })
    }
}

impl<'a, T: UserData<Trans = RefCell<T>>> FromLua<'a> for RefMut<'a, T> {
    fn from_lua(s: &'a State, val: ValRef<'a>) -> Result<Self> {
        let u = LuaUserData::try_from(val)?;
        u.check_safe_index()?;
        u.userdata_ref::<T>()
            .ok_or("userdata not match")
            .lua_result()?
            .try_borrow_mut()
            .lua_result()
            // Safety: check_safe_index
            .map(|x| unsafe { core::mem::transmute(x) })
    }
}

#[cfg(feature = "parking_lot")]
impl<T: UserData> UserDataTrans<T> for parking_lot::RwLock<T> {
    type Read<'a> = parking_lot::RwLockReadGuard<'a, T> where T: 'a;
    type Write<'a> = parking_lot::RwLockWriteGuard<'a, T> where T: 'a;

    fn trans(udata: T) -> Self {
        parking_lot::RwLock::new(udata)
    }

    fn read(&self) -> Self::Read<'_> {
        self.try_read().expect("")
    }
}

#[cfg(feature = "parking_lot")]
impl<'a, T: UserData<Trans = parking_lot::RwLock<T>>> FromLua<'a>
    for parking_lot::RwLockReadGuard<'a, T>
{
    fn from_lua(s: &'a State, val: ValRef<'a>) -> Result<Self> {
        let u = LuaUserData::try_from(val)?;
        u.check_safe_index()?;
        u.userdata_ref::<T>()
            .ok_or("userdata not match")
            .lua_result()?
            .try_read()
            .ok_or("RwLock::try_read")
            .lua_result()
            // Safety: check_safe_index
            .map(|x| unsafe { core::mem::transmute(x) })
    }
}

#[cfg(feature = "parking_lot")]
impl<'a, T: UserData<Trans = parking_lot::RwLock<T>>> FromLua<'a>
    for parking_lot::RwLockWriteGuard<'a, T>
{
    fn from_lua(s: &'a State, val: ValRef<'a>) -> Result<Self> {
        let u = LuaUserData::try_from(val)?;
        u.check_safe_index()?;
        u.userdata_ref::<T>()
            .ok_or("userdata not match")
            .lua_result()?
            .try_write()
            .ok_or("RwLock::try_write")
            .lua_result()
            // Safety: check_safe_index
            .map(|x| unsafe { core::mem::transmute(x) })
    }
}

#[derive(Clone, Copy)]
pub struct MaybePtrRef<'a, T>(pub &'a T);

impl<'a, T> Deref for MaybePtrRef<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

#[repr(C)]
pub struct MaybePointer<T>(*const T, Option<Box<T>>);

impl<T: UserData> UserDataTrans<T> for MaybePointer<T> {
    type Read<'a> = MaybePtrRef<'a, T> where T: 'a;
    type Write<'a> = () where T: 'a;

    fn trans(udata: T) -> Self {
        Self(core::ptr::null(), Some(Box::new(udata)))
    }

    fn read(&self) -> Self::Read<'_> {
        MaybePtrRef(unsafe { self.get_ptr().as_ref().unwrap() })
    }
}

impl<T> Drop for MaybePointer<T> {
    fn drop(&mut self) {
        if !self.0.is_null() {
            core::mem::forget(self.1.take());
        }
    }
}

impl<T> MaybePointer<T> {
    pub fn get_ptr(&self) -> *const T {
        if self.0.is_null() {
            self.1
                .as_ref()
                .map(Box::as_ref)
                .map(|x| x as *const _)
                .unwrap_or(core::ptr::null())
        } else {
            self.0
        }
    }
}

impl<'a, T: UserData<Trans = MaybePointer<T>>> FromLua<'a> for MaybePtrRef<'a, T> {
    const TYPE_NAME: &'static str = T::TYPE_NAME;

    fn from_lua(lua: &'a State, val: ValRef<'a>) -> Result<Self> {
        let u = LuaUserData::try_from(val)?;
        u.check_safe_index()?;
        u.userdata_ref::<T>()
            .ok_or("userdata not match")
            // Safety: check_safe_index
            .and_then(|x| unsafe { x.get_ptr().as_ref().ok_or("null ptr") }.map(MaybePtrRef))
            .lua_result()
    }
}

pub fn init_wrapper<U: UserData>(mt: &Table) -> Result<()> {
    use crate::luaapi::UnsafeLuaApi;

    debug_assert_eq!(mt.type_of(), Type::Table);

    mt.setf(crate::cstr!("__name"), U::TYPE_NAME)?;
    mt.setf(crate::cstr!("__gc"), U::__close as CFunction)?;
    mt.setf(crate::cstr!("__close"), U::__close as CFunction)?;

    if U::RAW_LEN {
        mt.setf(crate::cstr!("__len"), __len as CFunction)?;
    }

    {
        let setter = mt.state.new_table_with_size(0, 0)?;
        mt.set("__setter", setter.clone())?;
        mt.state
            .balance_with(|_| U::setter(UserdataRegistry::new(&setter)))?;
        setter.0.ensure_top();
        mt.state.push_cclosure(Some(U::__newindex), 1);
        mt.state.set_field(mt.index, crate::cstr!("__newindex"));
    }

    {
        let methods = mt.state.new_table_with_size(0, 0)?;
        mt.set("__method", methods.clone())?;
        let getter = mt.state.new_table_with_size(0, 0)?;
        mt.set("__getter", getter.clone())?;
        mt.state.balance_with(|_| {
            U::methods(UserdataRegistry::new(&methods))?;
            U::getter(UserdataRegistry::new(&getter))
        })?;

        U::metatable(UserdataRegistry::new(mt))?;
        getter.0.ensure_top();
        methods.0.ensure_top();
        mt.get("__index")?.ensure_top();
        mt.state.push_cclosure(Some(U::__index), 3);
        mt.state.set_field(mt.index, crate::cstr!("__index"));
    }

    Ok(())
}

pub fn clear_cached<U: UserData>(ud: &U, s: &State) -> Result<()> {
    use crate::luaapi::UnsafeLuaApi;

    s.get_or_init_metatable(U::metatable_key())?;
    assert!(s.get_metatable(-1));
    let key = ud.key_to_cache();
    s.push_light_userdata(key as usize as *mut ());
    s.push_nil();
    s.raw_set(-3);
    s.pop(2);

    Ok(())
}

fn get_cahced<U: UserData>(s: &State, key: *const ()) -> Result<bool> {
    use crate::luaapi::UnsafeLuaApi;

    s.get_or_init_metatable(U::metatable_key())?;
    // use metatable of userdata's metatable as cache table
    if !s.get_metatable(-1) {
        UnsafeLuaApi::new_table(s);
        s.push_value(-1);
        s.set_metatable(-3);
        if U::WEAK_REF_CACHE {
            unsafe {
                get_weak_meta(s)?;
            }
            s.set_metatable(-2);
        }
    }
    s.push_light_userdata(key as usize as *mut ());
    if s.raw_get(-2) == Type::Userdata {
        s.replace(-3);
        s.pop(1);
        return Ok(true);
    }
    s.pop(1);
    s.push_light_userdata(key as usize as *mut ());

    Ok(false)
}

fn cache_userdata<U: UserData>(s: &State, _key: *const ()) {
    use crate::luaapi::UnsafeLuaApi;

    // meta | meta's meta | key | userdata
    s.push_value(-1);
    s.replace(-5);
    s.raw_set(-3);
    s.pop(1);
}

/// Bind rust types as lua userdata, which can make lua access rust methods as lua methods or properties
pub trait UserData: Sized {
    /// `__name` field in metatable
    const TYPE_NAME: &'static str = core::any::type_name::<Self>();

    /// get/set value from the first uservalue when read/write property
    const INDEX_USERVALUE: bool = false;

    /// set the `__len` metamethod, if true, return the size of this userdata
    const RAW_LEN: bool = false;

    /// set the cache table is a weaked reference if key_to_cache enabled
    const WEAK_REF_CACHE: bool = true;

    /// whether raising error when accessing non-exists property
    const ACCESS_ERROR: bool = true;

    const METATABLE_KEY: MetatableKey = init_wrapper::<Self>;

    fn metatable_key() -> MetatableKey {
        Self::METATABLE_KEY
    }

    type Trans: UserDataTrans<Self> = Self;

    /// add methods
    fn methods(methods: UserdataRegistry<Self>) -> Result<()> {
        Ok(())
    }

    /// add fields getter
    fn getter(fields: UserdataRegistry<Self>) -> Result<()> {
        Ok(())
    }

    /// add fields setter
    fn setter(fields: UserdataRegistry<Self>) -> Result<()> {
        Ok(())
    }

    /// add else meta methods, and you can acquire or overwrite the generated metamethods
    fn metatable(mt: UserdataRegistry<Self>) -> Result<()> {
        Ok(())
    }

    /// initialize userdata on the top of lua stack
    fn init_userdata(this: &Self::Trans, s: &State, udata: &LuaUserData) -> Result<()> {
        if Self::INDEX_USERVALUE {
            udata.set_uservalue(s.new_table()?)
        } else {
            Ok(())
        }
    }

    /* Auxiliary method */

    /// get a pointer whose type is lightuserdata as the key in cache table
    #[inline]
    fn key_to_cache(&self) -> *const () {
        core::ptr::null()
    }

    #[inline]
    fn uservalue_count(&self, s: &State) -> i32 {
        Self::INDEX_USERVALUE as _
    }

    /* MetaMmethod implementation */

    unsafe extern "C" fn __index(l: *mut lua_State) -> c_int {
        use crate::luaapi::UnsafeLuaApi;

        let s = State::from_raw_state(l);

        // access getter table
        s.push_value(2);
        if s.get_table(lua_upvalueindex(1)) == Type::Function {
            s.push_value(1);
            s.push_value(2);
            s.tailcall(2, 1);
            return 1;
        }

        // access method table
        s.push_value(2);
        if !s.get_table(lua_upvalueindex(2)).is_none_or_nil() {
            return 1;
        }

        // access user value as table
        if Self::INDEX_USERVALUE {
            s.get_uservalue(1);
            s.push_value(2);
            if !s.get_table(-2).is_none_or_nil() {
                return 1;
            }
        }

        // access custom __index function
        s.push_value(lua_upvalueindex(3));
        if s.type_of(-1) == Type::Function {
            s.push_value(1);
            s.push_value(2);
            s.tailcall(2, 1);
            return 1;
        }

        if Self::ACCESS_ERROR {
            let field = s.to_string_lossy(2).unwrap_or_default();
            let error = format!("index non-exists field: {field:?}");
            s.error_string(error);
        }

        return 0;
    }

    unsafe extern "C" fn __newindex(l: *mut lua_State) -> c_int {
        use crate::luaapi::UnsafeLuaApi;

        let s = State::from_raw_state(l);

        // access setter table
        s.push_value(2);
        if s.get_table(lua_upvalueindex(1)) == Type::Function {
            s.push_value(1); // self
            s.push_value(3); // value
            s.push_value(2); // key
            s.tailcall(3, 0);
            return 0;
        }

        // access user value as table
        if Self::INDEX_USERVALUE {
            s.get_uservalue(1);
            s.push_value(2);
            s.push_value(3);
            s.set_table(-3);
        } else if Self::ACCESS_ERROR {
            let field = s.to_string_lossy(2).unwrap_or_default();
            let error = format!("index non-exists field: {field:?}");
            s.error_string(error);
        }

        return 0;
    }

    unsafe extern "C" fn __close(l: *mut lua_State) -> c_int {
        let s = State::from_raw_state(l);
        let u = LuaUserData::try_from(s.val(1)).ok();

        // take ownship, and remove it from cache table
        if let Some((this, mt)) = u.and_then(|u| {
            let mt = u.metatable().ok().flatten();
            u.take::<Self>().zip(mt)
        }) {
            let ptr = this.read().deref().key_to_cache();
            if !ptr.is_null() {
                mt.metatable()
                    .ok()
                    .flatten()
                    .and_then(|cache| cache.setp(ptr, ()).ok());
            }
            drop(this);
            return 0;
        }
        s.error_string("__close failed")
    }
}

unsafe extern "C" fn __len(l: *mut lua_State) -> c_int {
    lua_pushinteger(l, lua_rawlen(l, 1) as _);
    1
}

fn init_userdata<T: UserData>(s: &State) -> Result<()> {
    use crate::luaapi::UnsafeLuaApi;

    let ud = s.val(s.abs_index(-1));
    let ud: LuaUserData = ud.try_into()?;
    T::init_userdata(ud.userdata_ref::<T>().expect("convert userdata"), s, &ud)
}

impl<T: UserData> ToLua for T {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> = Some(|this, s| s.push_udatauv(this, 0));
}

// TODO: scoped userdata
impl<T: UserData<Trans = MaybePointer<T>>> ToLua for MaybePtrRef<'_, T> {
    const __PUSH: Option<fn(Self, &State) -> Result<()>> = Some(|this, s| {
        let key = this.key_to_cache();
        if !key.is_null() && get_cahced::<T>(s, key)? {
            return Ok(());
        }

        unsafe {
            use crate::luaapi::UnsafeLuaApi;

            let p = s
                .new_userdatauv(
                    core::mem::size_of::<MaybePointer<T>>(),
                    this.uservalue_count(s),
                )
                .cast::<MaybePointer<T>>()
                .as_mut()
                .expect("new MaybePointer");
            p.0 = this.0;
            core::ptr::write(&mut p.1, None);
        }
        s.set_or_init_metatable(T::metatable_key())?;

        if T::INDEX_USERVALUE {
            s.balance_with(init_userdata::<T>)?;
        }

        if !key.is_null() {
            cache_userdata::<T>(s, key)
        }
        // debug_assert_eq!(Type::Userdata, s.type_of(-1));

        Ok(())
    });
}

impl<'a, T: UserData<Trans = T>> FromLua<'a> for &'a T {
    const TYPE_NAME: &'static str = T::TYPE_NAME;

    fn from_lua(lua: &'a State, val: ValRef<'a>) -> Result<&'a T> {
        let u = LuaUserData::try_from(val)?;
        u.check_safe_index()?;
        u.userdata_ref::<T>()
            .ok_or("userdata not match")
            .lua_result()
            // Safety: check_safe_index
            .map(|x| unsafe { core::mem::transmute(x) })
    }
}

impl State {
    /// Register a metatable of UserData into the C registry and return it
    #[inline(always)]
    pub fn register_usertype<U: UserData>(&self) -> Result<Table> {
        self.get_or_init_metatable(U::metatable_key())?;
        Ok(self.top_val().try_into().unwrap())
    }

    /// Create userdata
    #[inline(always)]
    pub fn new_userdata<T: UserData>(&self, data: T) -> Result<LuaUserData> {
        self.new_userdata_with_values::<T, (), 0>(data, [])
    }

    #[inline(always)]
    pub fn new_userdata_with_values<T: UserData, R: ToLua, const N: usize>(
        &self,
        data: T,
        refs: [R; N],
    ) -> Result<LuaUserData> {
        let mut n = data.uservalue_count(self);
        self.push_udatauv(data, N as _)?;
        let ud = LuaUserData::try_from(self.top_val())?;
        for r in refs.into_iter() {
            n += 1;
            ud.set_iuservalue(n, r)?;
        }
        Ok(ud)
    }

    /// Create userdata with custom size
    #[inline(always)]
    pub fn new_userdata_untyped(&self, size: usize, uv_count: usize) -> Result<LuaUserData> {
        use crate::luaapi::UnsafeLuaApi;

        self.new_userdatauv(size, uv_count as _);
        self.top_val().try_into()
    }

    pub(crate) fn push_udatauv<T: UserData>(&self, data: T, extra: i32) -> Result<()> {
        use crate::luaapi::UnsafeLuaApi;

        let key = data.key_to_cache();
        if !key.is_null() && get_cahced::<T>(self, key)? {
            return Ok(());
        }

        unsafe {
            let p = self
                .new_userdatauv(
                    core::mem::size_of::<T::Trans>(),
                    data.uservalue_count(self) + extra,
                )
                .cast::<T::Trans>()
                .as_mut()
                .unwrap();
            core::ptr::write(p, <T::Trans as UserDataTrans<T>>::trans(data));
            if let Some(init_userdata) = <T::Trans as UserDataTrans<T>>::INIT_USERDATA {
                init_userdata(self, p);
            }
            self.set_or_init_metatable(T::metatable_key())?;
        }

        if T::INDEX_USERVALUE {
            self.balance_with(init_userdata::<T>)?;
        }

        if !key.is_null() {
            cache_userdata::<T>(self, key)
        }
        // debug_assert_eq!(Type::Userdata, s.type_of(-1));

        Ok(())
    }

    #[inline(always)]
    pub(crate) fn push_userdatauv<T>(&self, data: T, n: i32) -> Result<&mut T> {
        use crate::luaapi::UnsafeLuaApi;

        unsafe {
            let result = self
                .new_userdatauv(mem::size_of::<T>(), n)
                .cast::<T>()
                .as_mut()
                .ok_or_else(|| Error::runtime("allocate userdata"))?;
            core::ptr::write(result, data);
            Ok(result)
        }
    }

    /// Convenience function that calls `to_userdata` and performs a cast.
    pub(crate) unsafe fn to_userdata_typed<'a, T>(&'a self, index: Index) -> Option<&'a mut T> {
        use crate::luaapi::UnsafeLuaApi;

        mem::transmute(self.to_userdata(index))
    }

    unsafe fn check_userdata_typed<'a, T>(&'a self, index: Index) -> &'a mut T {
        use crate::luaapi::UnsafeLuaApi;

        luaL_checktype(self.state, index, LUA_TUSERDATA);
        mem::transmute(self.to_userdata(index))
    }

    #[inline(always)]
    fn get_userdata_by_size<'a, T>(&'a self, index: Index) -> Option<&'a mut T> {
        use crate::luaapi::UnsafeLuaApi;

        unsafe {
            if self.type_of(index) == Type::Userdata
                && self.raw_len(index) as usize == mem::size_of::<T>()
            {
                Some(mem::transmute(self.to_userdata(index)))
            } else {
                None
            }
        }
    }
}

#[derive(Debug)]
pub struct MethodRegistry<'a, U: 'a, R, W>(pub &'a Table<'a>, PhantomData<(U, R, W)>);

impl<'a, U: 'a, R, W> Deref for MethodRegistry<'a, U, R, W> {
    type Target = Table<'a>;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

#[allow(type_alias_bounds)]
pub type UserdataRegistry<'a, U: UserData + 'a> = MethodRegistry<
    'a,
    U,
    <U::Trans as UserDataTrans<U>>::Read<'a>,
    <U::Trans as UserDataTrans<U>>::Write<'a>,
>;

impl<'a, U: 'a + ?Sized, R: 'a, W> MethodRegistry<'a, &U, R, W> {
    #[inline(always)]
    pub fn add_deref<K, V, ARGS: 'a, RET: 'a>(&self, k: K, v: V) -> Result<&Self>
    where
        K: ToLua,
        V: LuaMethod<'a, (R, &'a U, &'a U), ARGS, RET>,
        U: 'a,
        R: Deref<Target = U>,
        &'a R: FromLua<'a>,
    {
        self.0
            .raw_set(k, self.state.bind_closure(|s| v.call_method(s), 0)?)?;
        Ok(self)
    }

    #[inline(always)]
    pub fn add_deref_mut<K, V, ARGS: 'a, RET: 'a>(&self, k: K, v: V) -> Result<&Self>
    where
        K: ToLua,
        V: LuaMethod<'a, (W, &'a U, &'a U), ARGS, RET>,
        U: 'a,
        W: DerefMut<Target = U>,
        &'a W: FromLua<'a> + 'a,
    {
        self.0
            .raw_set(k, self.state.bind_closure(|s| v.call_method(s), 0)?)?;
        Ok(self)
    }
}

impl<'a, U: 'a, R: 'a, W> MethodRegistry<'a, U, R, W> {
    pub fn new(mt: &'a Table<'a>) -> Self {
        Self(mt, PhantomData)
    }

    #[inline(always)]
    pub fn add_field_get<M, RET>(&self, k: &str, field: M) -> Result<&Self>
    where
        RET: ToLuaMulti + 'a,
        M: Fn(&'a State, &'a U) -> RET,
        R: Deref<Target = U> + FromLua<'a> + 'a,
    {
        self.0.raw_set(
            k,
            self.state.bind_closure(
                |lua| unsafe {
                    let this = check_from_lua::<R>(lua, 1)?;
                    lua.pushed(field(lua, core::mem::transmute(this.deref())))
                },
                0,
            )?,
        )?;
        Ok(self)
    }

    #[inline(always)]
    pub fn add_field_set<M, A, RET>(&self, k: &str, field: M) -> Result<&Self>
    where
        A: FromLua<'a> + 'a,
        RET: ToLuaMulti + 'a,
        M: Fn(&'a State, &'a mut U, A) -> RET,
        W: DerefMut<Target = U> + FromLua<'a> + 'a,
    {
        self.0.raw_set(
            k,
            self.state.bind_closure(
                |lua| unsafe {
                    let mut this = check_from_lua::<W>(lua, 1)?;
                    lua.pushed(field(
                        lua,
                        core::mem::transmute(this.deref_mut()),
                        check_from_lua(lua, 2)?,
                    ))
                },
                0,
            )?,
        )?;
        Ok(self)
    }

    #[inline(always)]
    pub fn add_method<M, ARGS, RET>(&self, k: &str, method: M) -> Result<&Self>
    where
        ARGS: FromLuaMulti<'a> + 'a,
        RET: ToLuaMulti + 'a,
        M: Fn(&'a State, &'a U, ARGS) -> RET,
        R: Deref<Target = U> + FromLua<'a> + 'a,
    {
        self.0.raw_set(
            k,
            self.state.bind_closure(
                |lua| unsafe {
                    let this = check_from_lua::<R>(lua, 1)?;
                    lua.pushed(method(
                        lua,
                        core::mem::transmute(this.deref()),
                        ARGS::from_lua_multi(lua, 2)?,
                    ))
                },
                0,
            )?,
        )?;
        Ok(self)
    }

    #[inline(always)]
    pub fn add_method_mut<M, ARGS, RET>(&self, k: &str, method: M) -> Result<&Self>
    where
        ARGS: FromLuaMulti<'a> + 'a,
        RET: ToLuaMulti + 'a,
        M: Fn(&'a State, &'a mut U, ARGS) -> RET,
        W: DerefMut<Target = U> + FromLua<'a> + 'a,
    {
        self.0.raw_set(
            k,
            self.state.bind_closure(
                |lua| unsafe {
                    let mut this = check_from_lua::<W>(lua, 1)?;
                    lua.pushed(method(
                        lua,
                        core::mem::transmute(this.deref_mut()),
                        ARGS::from_lua_multi(lua, 2)?,
                    ))
                },
                0,
            )?,
        )?;
        Ok(self)
    }

    #[cfg(feature = "async")]
    #[inline(always)]
    pub fn add_async_method<M, ARGS, RET, FUT>(&self, k: &str, method: M) -> Result<&Self>
    where
        ARGS: FromLuaMulti<'a> + Send + 'a,
        RET: ToLuaMulti + 'a,
        M: Fn(&'a State, &'a U, ARGS) -> FUT + Sync + Send + 'static,
        R: Deref<Target = U> + FromLua<'a> + Send + 'a,
        FUT: core::future::Future<Output = RET> + Send + 'a,
    {
        self.0.raw_set(
            k,
            self.state.bind_async_closure(move |lua, base| {
                let this = check_from_lua::<R>(lua, base)?;
                Ok(method(
                    lua,
                    unsafe { core::mem::transmute(this.deref()) },
                    ARGS::from_lua_multi(lua, base + 1)?,
                ))
            })?,
        )?;
        Ok(self)
    }

    #[cfg(feature = "async")]
    #[inline(always)]
    pub fn add_async_method_mut<M, ARGS, RET, FUT>(&self, k: &str, method: M) -> Result<&Self>
    where
        ARGS: FromLuaMulti<'a> + Send + 'a,
        RET: ToLuaMulti + 'a,
        M: Fn(&'a State, &'a mut U, ARGS) -> FUT + Sync + Send + 'static,
        W: DerefMut<Target = U> + FromLua<'a> + Send + 'a,
        FUT: core::future::Future<Output = RET> + Send + 'a,
    {
        self.0.raw_set(
            k,
            self.state.bind_async_closure(move |lua, base| {
                let mut this = check_from_lua::<W>(lua, base)?;
                Ok(method(
                    lua,
                    unsafe { core::mem::transmute(this.deref_mut()) },
                    ARGS::from_lua_multi(lua, base + 1)?,
                ))
            })?,
        )?;
        Ok(self)
    }

    #[inline(always)]
    pub fn add<K, V, ARGS: 'a, RET: 'a>(&self, k: K, v: V) -> Result<&Self>
    where
        K: ToLua,
        V: LuaMethod<'a, (R, &'a U), ARGS, RET>,
        R: Deref<Target = U> + FromLua<'a>,
    {
        self.0
            .raw_set(k, self.state.bind_closure(|s| v.call_method(s), 0)?)?;
        Ok(self)
    }

    #[inline(always)]
    pub fn add_mut<K, V, ARGS: 'a, RET: 'a>(&self, k: K, v: V) -> Result<&Self>
    where
        K: ToLua,
        V: LuaMethod<'a, (W, &'a mut U), ARGS, RET>,
        W: DerefMut<Target = U> + FromLua<'a> + 'a,
    {
        self.0
            .raw_set(k, self.state.bind_closure(|s| v.call_method(s), 0)?)?;
        Ok(self)
    }

    #[inline(always)]
    pub fn as_deref<A: ?Sized>(&self) -> MethodRegistry<'a, &'a A, U, W>
    where
        U: Deref<Target = A>,
    {
        MethodRegistry::new(self.0)
    }

    #[inline(always)]
    pub fn as_deref_mut<A: ?Sized>(&self) -> MethodRegistry<'a, &'a A, R, U>
    where
        U: DerefMut<Target = A>,
    {
        MethodRegistry::new(self.0)
    }
}
