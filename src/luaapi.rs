use crate::{
    convert::{Index, ToLua},
    error::Error,
    ffi::*,
    state::State,
    str::*,
};

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;
use core::ffi::{c_char, c_int, c_void};
use core::{mem, ptr, slice, str};

#[allow(non_camel_case_types)]
type size_t = usize;

#[extend::ext(pub, name = UnsafeLuaApi)]
impl State {
    /// Initializes a new Lua state. This function does not open any libraries
    /// by default. Calls `lua_newstate` internally.
    fn new() -> *mut lua_State {
        unsafe { luaL_newstate() }
    }

    /// Returns an unsafe pointer to the wrapped `lua_State`.
    #[inline(always)]
    fn as_ptr(&self) -> *mut lua_State {
        self.state
    }

    /// Maps to `luaL_openlibs`.
    fn open_libs(&self) {
        unsafe {
            luaL_openlibs(self.state);
        }
    }

    /// Maps to `luaopen_base`.
    #[inline(always)]
    fn open_base(&self) -> c_int {
        unsafe { luaopen_base(self.state) }
    }

    /// Maps to `luaopen_coroutine`.
    #[inline(always)]
    fn open_coroutine(&self) -> c_int {
        unsafe { luaopen_coroutine(self.state) }
    }

    /// Maps to `luaopen_table`.
    #[inline(always)]
    fn open_table(&self) -> c_int {
        unsafe { luaopen_table(self.state) }
    }

    /// Maps to `luaopen_io`.
    #[inline(always)]
    fn open_io(&self) -> c_int {
        unsafe { luaopen_io(self.state) }
    }

    /// Maps to `luaopen_os`.
    #[inline(always)]
    fn open_os(&self) -> c_int {
        unsafe { luaopen_os(self.state) }
    }

    /// Maps to `luaopen_string`.
    #[inline(always)]
    fn open_string(&self) -> c_int {
        unsafe { luaopen_string(self.state) }
    }

    /// Maps to `luaopen_utf8`.
    #[inline(always)]
    fn open_utf8(&self) -> c_int {
        unsafe { luaopen_utf8(self.state) }
    }

    /// Maps to `luaopen_math`.
    #[inline(always)]
    fn open_math(&self) -> c_int {
        unsafe { luaopen_math(self.state) }
    }

    /// Maps to `luaopen_debug`.
    #[inline(always)]
    fn open_debug(&self) -> c_int {
        unsafe { luaopen_debug(self.state) }
    }

    /// Maps to `luaopen_package`.
    #[inline(always)]
    fn open_package(&self) -> c_int {
        unsafe { luaopen_package(self.state) }
    }

    /// Maps to `luaL_dofile`.
    fn do_file(&self, filename: &str) -> i32 {
        let c_str = CString::new(filename).unwrap();
        unsafe { luaL_dofile(self.state, c_str.as_ptr()) }
    }

    /// Maps to `luaL_dostring`.
    fn do_string(&self, s: &str) -> i32 {
        let c_str = CString::new(s).unwrap();
        unsafe { luaL_dostring(self.state, c_str.as_ptr()) }
    }

    //===========================================================================
    // State manipulation
    //===========================================================================
    /// Maps to `lua_close`.
    #[inline(always)]
    fn close(&self) {
        unsafe {
            lua_close(self.state);
        }
    }

    /// [-0, +1, m] Maps to `lua_newthread`.
    #[inline(always)]
    fn new_thread(&self) -> *mut lua_State {
        unsafe { lua_newthread(self.state) }
    }

    /// Maps to `lua_atpanic`.
    #[inline(always)]
    fn at_panic(&self, panicf: lua_CFunction) -> lua_CFunction {
        unsafe { lua_atpanic(self.state, panicf) }
    }

    /// Maps to `lua_version`.
    fn version(state: *mut lua_State) -> lua_Number {
        unsafe { lua_version(state) }
    }

    //===========================================================================
    // Basic stack manipulation
    //===========================================================================
    /// Maps to `lua_absindex`.
    #[inline(always)]
    fn abs_index(&self, idx: Index) -> Index {
        unsafe { lua_absindex(self.state, idx) }
    }

    /// Maps to `lua_gettop`.
    #[inline(always)]
    fn get_top(&self) -> Index {
        unsafe { lua_gettop(self.state) }
    }

    /// Maps to `lua_settop`.
    #[inline(always)]
    fn set_top(&self, index: Index) {
        unsafe { lua_settop(self.state, index) }
    }

    /// Maps to `lua_rotate`.
    #[inline(always)]
    fn rotate(&self, idx: Index, n: c_int) {
        unsafe { lua_rotate(self.state, idx, n) }
    }

    /// Maps to `lua_copy`.
    #[inline(always)]
    fn copy(&self, from_idx: Index, to_idx: Index) {
        unsafe { lua_copy(self.state, from_idx, to_idx) }
    }

    /// Maps to `lua_checkstack`.
    #[inline(always)]
    fn check_stack(&self, extra: c_int) -> bool {
        let result = unsafe { lua_checkstack(self.state, extra) };
        result != 0
    }

    /// Maps to `lua_xmove`.
    #[inline(always)]
    fn xmove(&self, to: *mut lua_State, n: c_int) {
        unsafe { lua_xmove(self.state, to, n) }
    }

    //===========================================================================
    // Access functions (stack -> C)
    //===========================================================================
    /// Maps to `lua_isnumber`.
    #[inline(always)]
    fn is_number(&self, index: Index) -> bool {
        unsafe { lua_isnumber(self.state, index) == 1 }
    }

    /// Maps to `lua_isstring`.
    #[inline(always)]
    fn is_string(&self, index: Index) -> bool {
        unsafe { lua_isstring(self.state, index) == 1 }
    }

    /// Maps to `lua_iscfunction`.
    #[inline(always)]
    fn is_native_fn(&self, index: Index) -> bool {
        unsafe { lua_iscfunction(self.state, index) == 1 }
    }

    /// Maps to `lua_isinteger`.
    #[inline(always)]
    fn is_integer(&self, index: Index) -> bool {
        unsafe { lua_isinteger(self.state, index) == 1 }
    }

    /// Maps to `lua_isuserdata`.
    #[inline(always)]
    fn is_userdata(&self, index: Index) -> bool {
        unsafe { lua_isuserdata(self.state, index) == 1 }
    }

    /// Maps to `lua_type`.
    #[inline(always)]
    fn type_of(&self, index: Index) -> Type {
        let result = unsafe { lua_type(self.state, index) };
        Type::from_c_int(result)
    }

    /// Maps to `lua_typename`.
    #[inline(always)]
    fn typename_of(&self, tp: Type) -> Cow<str> {
        unsafe {
            let ptr = lua_typename(self.state, tp as c_int);
            let slice = CStr::from_ptr(ptr).to_bytes();
            String::from_utf8_lossy(slice)
        }
    }

    /// Maps to `lua_toboolean`.
    #[inline(always)]
    fn to_bool(&self, index: Index) -> bool {
        let result = unsafe { lua_toboolean(self.state, index) };
        result != 0
    }

    // omitted: lua_tolstring

    /// Maps to `lua_rawlen`.
    #[inline(always)]
    fn raw_len(&self, index: Index) -> size_t {
        unsafe { lua_rawlen(self.state, index) }
    }

    /// Maps to `lua_tocfunction`.
    #[inline(always)]
    fn to_native_fn(&self, index: Index) -> lua_CFunction {
        let result = unsafe { lua_tocfunction(self.state, index) };
        result
    }

    /// Maps to `lua_touserdata`.
    #[inline(always)]
    fn to_userdata(&self, index: Index) -> *mut c_void {
        unsafe { lua_touserdata(self.state, index) }
    }

    /// Maps to `lua_tothread`.
    #[inline]
    fn to_thread(&self, index: Index) -> Option<&mut lua_State> {
        unsafe { lua_tothread(self.state, index).as_mut() }
    }

    /// Maps to `lua_topointer`.
    #[inline(always)]
    fn to_pointer(&self, index: Index) -> *const c_void {
        unsafe { lua_topointer(self.state, index) }
    }

    //===========================================================================
    // Comparison and arithmetic functions
    //===========================================================================
    /// Maps to `lua_arith`.
    #[inline(always)]
    fn arith(&self, op: Arithmetic) {
        unsafe { lua_arith(self.state, op as c_int) }
    }

    /// Maps to `lua_rawequal`.
    #[inline(always)]
    fn raw_equal(&self, idx1: Index, idx2: Index) -> bool {
        let result = unsafe { lua_rawequal(self.state, idx1, idx2) };
        result != 0
    }

    /// Maps to `lua_compare`.
    #[inline(always)]
    fn compare(&self, idx1: Index, idx2: Index, op: Comparison) -> bool {
        let result = unsafe { lua_compare(self.state, idx1, idx2, op as c_int) };
        result != 0
    }

    //===========================================================================
    // Push functions (C -> stack)
    //===========================================================================
    /// Maps to `lua_pushnil`.
    #[inline(always)]
    fn push_nil(&self) {
        unsafe { lua_pushnil(self.state) }
    }

    /// Maps to `lua_pushnumber`.
    #[inline(always)]
    fn push_number(&self, n: lua_Number) {
        unsafe { lua_pushnumber(self.state, n) }
    }

    /// Maps to `lua_pushinteger`.
    #[inline(always)]
    fn push_integer(&self, i: lua_Integer) {
        unsafe { lua_pushinteger(self.state, i) }
    }

    // omitted: lua_pushstring

    /// Maps to `lua_pushlstring`.
    #[inline(always)]
    fn push_string(&self, s: &str) {
        unsafe { lua_pushlstring(self.state, s.as_ptr() as *const _, s.len() as size_t) };
    }

    /// Maps to `lua_pushlstring`.
    #[inline(always)]
    fn push_bytes(&self, s: &[u8]) {
        unsafe { lua_pushlstring(self.state, s.as_ptr() as *const _, s.len() as size_t) };
    }

    // omitted: lua_pushvfstring
    // omitted: lua_pushfstring

    /// Maps to `lua_pushcclosure`.
    #[inline(always)]
    fn push_cclosure(&self, f: lua_CFunction, n: c_int) {
        unsafe { lua_pushcclosure(self.state, f, n) }
    }

    /// Maps to `lua_pushboolean`.
    #[inline(always)]
    fn push_bool(&self, b: bool) {
        unsafe { lua_pushboolean(self.state, b as c_int) }
    }

    /// Maps to `lua_pushlightuserdata`. The Lua state will receive a pointer to
    /// the given value. The caller is responsible for cleaning up the data. Any
    /// code that manipulates the userdata is free to modify its contents, so
    /// memory safety is not guaranteed.
    #[inline(always)]
    fn push_light_userdata<T>(&self, ud: *mut T) {
        unsafe { lua_pushlightuserdata(self.state, mem::transmute(ud)) }
    }

    /// Maps to `lua_pushthread`.
    #[inline(always)]
    fn push_thread(&self) -> bool {
        let result = unsafe { lua_pushthread(self.state) };
        result != 1
    }

    /// Maps to `lua_pushvalue`.
    #[inline(always)]
    fn push_value(&self, index: Index) {
        unsafe { lua_pushvalue(self.state, index) }
    }

    //===========================================================================
    // Get functions (Lua -> stack)
    //===========================================================================
    /// [-0, +1, -] `lua_getglobal`.
    #[inline(always)]
    fn get_global(&self, name: &CStr) -> Type {
        Type::from_c_int(unsafe { lua_getglobal(self.state, name.as_ptr()) })
    }

    /// Maps to `lua_gettable`.
    #[inline(always)]
    fn get_table(&self, index: Index) -> Type {
        let ty = unsafe { lua_gettable(self.state, index) };
        Type::from_c_int(ty)
    }

    /// Maps to `lua_getfield`.
    #[inline(always)]
    fn get_field(&self, index: Index, k: &CStr) -> Type {
        Type::from_c_int(unsafe { lua_getfield(self.state, index, k.as_ptr()) })
    }

    /// Maps to `lua_geti`.
    #[inline(always)]
    fn geti(&self, index: Index, i: lua_Integer) -> Type {
        let ty = unsafe { lua_geti(self.state, index, i) };
        Type::from_c_int(ty)
    }

    /// [-1, +1, -] `lua_rawget`.
    #[inline(always)]
    fn raw_get(&self, index: Index) -> Type {
        let ty = unsafe { lua_rawget(self.state, index) };
        Type::from_c_int(ty)
    }

    /// Maps to `lua_rawgeti`.
    #[inline(always)]
    fn raw_geti(&self, index: Index, n: lua_Integer) -> Type {
        let ty = unsafe { lua_rawgeti(self.state, index, n) };
        Type::from_c_int(ty)
    }

    /// [0, +1, -] `lua_rawgetp`.
    #[inline(always)]
    fn raw_getp<T>(&self, index: Index, p: *const T) -> Type {
        let ty = unsafe { lua_rawgetp(self.state, index, mem::transmute(p)) };
        Type::from_c_int(ty)
    }

    /// Maps to `lua_createtable`.
    #[inline(always)]
    fn create_table(&self, narr: c_int, nrec: c_int) {
        unsafe { lua_createtable(self.state, narr, nrec) }
    }

    /// Maps to `lua_newuserdata`. The pointer returned is owned by the Lua state
    /// and it will be garbage collected when it is no longer in use or the state
    /// is closed. To specify custom cleanup behavior, use a `__gc` metamethod.
    #[inline(always)]
    fn new_userdata(&self, sz: size_t) -> *mut c_void {
        unsafe { lua_newuserdata(self.state, sz) }
    }

    #[inline(always)]
    fn new_userdatauv(&self, sz: size_t, n: i32) -> *mut c_void {
        unsafe { lua_newuserdatauv(self.state, sz, n) }
    }

    /// [-0, +(0|1), –] `lua_getmetatable`.
    #[inline(always)]
    fn get_metatable(&self, objindex: Index) -> bool {
        let result = unsafe { lua_getmetatable(self.state, objindex) };
        result != 0
    }

    /// Maps to `lua_getuservalue`.
    #[inline(always)]
    fn get_uservalue(&self, idx: Index) -> Type {
        let result = unsafe { lua_getuservalue(self.state, idx) };
        Type::from_c_int(result)
    }

    /// [-0, +1, –] Maps to `lua_getiuservalue`.
    #[inline(always)]
    fn get_iuservalue(&self, idx: Index, n: i32) -> Type {
        let result = unsafe { lua_getiuservalue(self.state, idx, n) };
        Type::from_c_int(result)
    }

    //===========================================================================
    // Set functions (stack -> Lua)
    //===========================================================================
    /// Maps to `lua_setglobal`.
    #[inline(always)]
    fn set_global(&self, var: &CStr) {
        unsafe { lua_setglobal(self.state, var.as_ptr()) }
    }

    /// Maps to `lua_settable`.
    #[inline(always)]
    fn set_table(&self, idx: Index) {
        unsafe { lua_settable(self.state, idx) }
    }

    /// Maps to `lua_setfield`.
    #[inline(always)]
    fn set_field(&self, idx: Index, k: &CStr) {
        unsafe { lua_setfield(self.state, idx, k.as_ptr()) }
    }

    /// Maps to `lua_seti`.
    #[inline(always)]
    fn seti(&self, idx: Index, n: lua_Integer) {
        unsafe { lua_seti(self.state, idx, n) }
    }

    /// [-2, +0, m] `lua_rawset`.
    #[inline(always)]
    fn raw_set(&self, idx: Index) {
        unsafe { lua_rawset(self.state, idx) }
    }

    /// Maps to `lua_rawseti`.
    #[inline(always)]
    fn raw_seti(&self, idx: Index, n: lua_Integer) {
        unsafe { lua_rawseti(self.state, idx, n) }
    }

    /// Maps to `lua_rawsetp`.
    #[inline(always)]
    fn raw_setp<T>(&self, idx: Index, p: *const T) {
        unsafe { lua_rawsetp(self.state, idx, mem::transmute(p)) }
    }

    /// [-1, +0, -] `lua_setmetatable`.
    #[inline(always)]
    fn set_metatable(&self, objindex: Index) {
        unsafe { lua_setmetatable(self.state, objindex) };
    }

    /// [-1, +0, -] `lua_setuservalue`.
    #[inline(always)]
    fn set_uservalue(&self, idx: Index) {
        unsafe {
            lua_setuservalue(self.state, idx);
        }
    }

    /// [-1, +0, -] Maps to `lua_setiuservalue`.
    #[inline(always)]
    fn set_iuservalue(&self, idx: Index, n: i32) {
        unsafe {
            lua_setiuservalue(self.state, idx, n);
        }
    }

    /// Maps to `luaL_callmeta`.
    #[inline(always)]
    fn call(&self, n: c_int, r: c_int) {
        unsafe { lua_call(self.state, n, r) }
    }

    fn tailcall(self, n: c_int, r: c_int) {
        let state = self.state;
        drop(self);
        unsafe { lua_call(state, n, r) }
    }

    //===========================================================================
    // Coroutine functions
    //===========================================================================
    /// Maps to `lua_resume`.
    fn resume(&self, from: *mut lua_State, nargs: c_int, nresults: &mut c_int) -> ThreadStatus {
        let result = unsafe { lua_resume(self.state, from, nargs, nresults) };
        ThreadStatus::from_c_int(result)
    }

    /// Maps to `lua_status`.
    #[inline(always)]
    fn status(&self) -> ThreadStatus {
        let result = unsafe { lua_status(self.state) };
        ThreadStatus::from_c_int(result)
    }

    /// Maps to `lua_isyieldable`.
    #[inline(always)]
    fn is_yieldable(&self) -> bool {
        let result = unsafe { lua_isyieldable(self.state) };
        result != 0
    }

    //===========================================================================
    // Garbage-collection function
    //===========================================================================
    // TODO: return typing?
    /// Maps to `lua_gc`.
    #[inline(always)]
    fn gc(&self, what: GcOption, data: c_int) -> c_int {
        unsafe { lua_gc(self.state, what as c_int, data) }
    }

    //===========================================================================
    // Miscellaneous functions
    //===========================================================================
    /// Maps to `lua_error`.
    #[inline(always)]
    fn error(self) -> ! {
        let state = self.state;
        drop(self);
        unsafe { lua_error(state) };
    }

    /// Maps to `lua_next`.
    #[inline(always)]
    fn next(&self, idx: Index) -> bool {
        let result = unsafe { lua_next(self.state, idx) };
        result != 0
    }

    /// Maps to `lua_concat`.
    #[inline(always)]
    fn concat(&self, n: c_int) {
        unsafe { lua_concat(self.state, n) }
    }

    /// Maps to `lua_len`.
    #[inline(always)]
    fn len(&self, idx: Index) {
        unsafe { lua_len(self.state, idx) }
    }

    /// Maps to `lua_stringtonumber`.
    fn string_to_number(&self, s: &str) -> size_t {
        let c_str = CString::new(s).unwrap();
        unsafe { lua_stringtonumber(self.state, c_str.as_ptr()) }
    }

    /// Maps to `lua_getallocf`.
    #[inline(always)]
    fn get_alloc_fn(&self) -> (lua_Alloc, *mut c_void) {
        let mut slot = ptr::null_mut();
        (unsafe { lua_getallocf(self.state, &mut slot) }, slot)
    }

    /// Maps to `lua_setallocf`.
    #[inline(always)]
    fn set_alloc_fn(&self, f: lua_Alloc, ud: *mut c_void) {
        unsafe { lua_setallocf(self.state, f, ud) }
    }

    /// Maps to `lua_tonumber`.
    #[inline(always)]
    fn to_number(&self, index: Index) -> lua_Number {
        unsafe { lua_tonumber(self.state, index) }
    }

    /// Maps to `lua_tonumberx`.
    #[inline(always)]
    fn to_numberx(&self, index: Index) -> Option<lua_Number> {
        let mut suc = 0i32;
        let r = unsafe { lua_tonumberx(self.state, index, &mut suc) };
        if suc > 0 {
            Some(r)
        } else {
            None
        }
    }

    /// Maps to `lua_tointeger`.
    #[inline(always)]
    fn to_integer(&self, index: Index) -> lua_Integer {
        unsafe { lua_tointeger(self.state, index) }
    }

    /// Maps to `lua_tointegerx`.
    #[inline(always)]
    fn to_integerx(&self, index: Index) -> Option<lua_Integer> {
        let mut isnum: c_int = 0;
        let r = unsafe { lua_tointegerx(self.state, index, &mut isnum) };
        if isnum == 0 {
            None
        } else {
            Some(r)
        }
    }

    /// Maps to `lua_pop`.
    #[inline(always)]
    fn pop(&self, n: c_int) {
        unsafe { lua_pop(self.state, n) }
    }

    /// Maps to `lua_newtable`.
    #[inline(always)]
    fn new_table(&self) {
        unsafe { lua_newtable(self.state) }
    }

    /// Maps to `lua_register`.
    #[inline(always)]
    fn register(&self, n: &str, f: CFunction) {
        let c_str = CString::new(n).unwrap();
        unsafe { lua_register(self.state, c_str.as_ptr(), Some(f)) }
    }

    /// Maps to `lua_pushcfunction`.
    #[inline(always)]
    fn push_fn(&self, f: lua_CFunction) {
        unsafe { lua_pushcfunction(self.state, f) }
    }

    /// Maps to `lua_isfunction`.
    #[inline(always)]
    fn is_function(&self, index: Index) -> bool {
        unsafe { lua_isfunction(self.state, index) == 1 }
    }

    /// Maps to `lua_istable`.
    #[inline(always)]
    fn is_table(&self, index: Index) -> bool {
        unsafe { lua_istable(self.state, index) == 1 }
    }

    /// Maps to `lua_islightuserdata`.
    #[inline(always)]
    fn is_light_userdata(&self, index: Index) -> bool {
        unsafe { lua_islightuserdata(self.state, index) == 1 }
    }

    /// Maps to `lua_isnil`.
    #[inline(always)]
    fn is_nil(&self, index: Index) -> bool {
        unsafe { lua_isnil(self.state, index) == 1 }
    }

    /// Maps to `lua_isboolean`.
    #[inline(always)]
    fn is_bool(&self, index: Index) -> bool {
        unsafe { lua_isboolean(self.state, index) == 1 }
    }

    /// Maps to `lua_isthread`.
    #[inline(always)]
    fn is_thread(&self, index: Index) -> bool {
        unsafe { lua_isthread(self.state, index) == 1 }
    }

    /// Maps to `lua_isnone`.
    #[inline(always)]
    fn is_none(&self, index: Index) -> bool {
        unsafe { lua_isnone(self.state, index) == 1 }
    }

    /// Maps to `lua_isnoneornil`.
    #[inline(always)]
    fn is_none_or_nil(&self, index: Index) -> bool {
        unsafe { lua_isnoneornil(self.state, index) == 1 }
    }

    // omitted: lua_pushliteral

    /// Maps to `lua_pushglobaltable`.
    #[inline(always)]
    fn push_global_table(&self) {
        unsafe { lua_pushglobaltable(self.state) };
    }

    /// Maps to `lua_insert`.
    #[inline(always)]
    fn insert(&self, idx: Index) {
        unsafe { lua_insert(self.state, idx) }
    }

    /// Maps to `lua_remove`.
    #[inline(always)]
    fn remove(&self, idx: Index) {
        unsafe { lua_remove(self.state, idx) }
    }

    /// Maps to `lua_replace`.
    #[inline(always)]
    fn replace(&self, idx: Index) {
        unsafe { lua_replace(self.state, idx) }
    }

    //===========================================================================
    // Debug API
    //===========================================================================
    /// Maps to `lua_getstack`.
    fn get_stack(&self, level: c_int) -> Option<lua_Debug> {
        let mut ar: lua_Debug = unsafe { core::mem::zeroed() };
        let result = unsafe { lua_getstack(self.state, level, &mut ar) };
        if result == 1 {
            Some(ar)
        } else {
            None
        }
    }

    /// Maps to `lua_getinfo`.
    fn get_info(&self, what: &CStr, ar: &mut lua_Debug) -> i32 {
        unsafe { lua_getinfo(self.state, what.as_ptr(), ar) }
    }

    /// Maps to `lua_getlocal`.
    fn get_local(&self, ar: &lua_Debug, n: c_int) -> Option<&str> {
        let ptr = unsafe { lua_getlocal(self.state, ar, n) };
        if ptr.is_null() {
            None
        } else {
            let slice = unsafe { CStr::from_ptr(ptr).to_bytes() };
            str::from_utf8(slice).ok()
        }
    }

    /// Maps to `lua_setlocal`.
    fn set_local(&self, ar: &lua_Debug, n: c_int) -> Option<&str> {
        let ptr = unsafe { lua_setlocal(self.state, ar, n) };
        if ptr.is_null() {
            None
        } else {
            let slice = unsafe { CStr::from_ptr(ptr).to_bytes() };
            str::from_utf8(slice).ok()
        }
    }

    /// Maps to `lua_getupvalue`.
    fn get_upvalue(&self, funcindex: Index, n: c_int) -> Option<&str> {
        let ptr = unsafe { lua_getupvalue(self.state, funcindex, n) };
        if ptr.is_null() {
            None
        } else {
            let slice = unsafe { CStr::from_ptr(ptr).to_bytes() };
            str::from_utf8(slice).ok()
        }
    }

    /// Maps to `lua_setupvalue`.
    fn set_upvalue(&self, funcindex: Index, n: c_int) -> Option<&str> {
        let ptr = unsafe { lua_setupvalue(self.state, funcindex, n) };
        if ptr.is_null() {
            None
        } else {
            let slice = unsafe { CStr::from_ptr(ptr).to_bytes() };
            str::from_utf8(slice).ok()
        }
    }

    /// Maps to `lua_upvalueid`.
    fn upvalue_id(&self, funcindex: Index, n: c_int) -> *mut c_void {
        unsafe { lua_upvalueid(self.state, funcindex, n) }
    }

    /// Maps to `lua_upvaluejoin`.
    fn upvalue_join(&self, fidx1: Index, n1: c_int, fidx2: Index, n2: c_int) {
        unsafe { lua_upvaluejoin(self.state, fidx1, n1, fidx2, n2) }
    }

    #[cfg(features = "std")]
    /// Maps to `lua_sethook`.
    fn set_hook(&self, func: lua_Hook, mask: HookMask, count: c_int) {
        unsafe { lua_sethook(self.state, func, mask.bits(), count) }
    }

    /// Maps to `lua_gethook`.
    fn get_hook(&self) -> Option<lua_Hook> {
        unsafe { lua_gethook(self.state) }
    }

    #[cfg(features = "std")]
    /// Maps to `lua_gethookmask`.
    fn get_hook_mask(&self) -> HookMask {
        let result = unsafe { lua_gethookmask(self.state) };
        HookMask::from_bits_truncate(result)
    }

    /// Maps to `lua_gethookcount`.
    fn get_hook_count(&self) -> c_int {
        unsafe { lua_gethookcount(self.state) }
    }

    /// Maps to `luaL_getmetafield`.
    #[inline(always)]
    fn get_metafield(&self, obj: Index, e: &CStr) -> bool {
        let result = unsafe { luaL_getmetafield(self.state, obj, e.as_ptr()) };
        result != 0
    }

    /// Maps to `luaL_callmeta`.
    #[inline(always)]
    fn call_meta(&self, obj: Index, e: &CStr) -> bool {
        let result = unsafe { luaL_callmeta(self.state, obj, e.as_ptr()) };
        result != 0
    }

    /// [-0, +0, -]
    #[inline(always)]
    fn to_string(&self, index: Index) -> *const c_char {
        unsafe { lua_tolstring(self.state, index, ptr::null_mut()) }
    }

    /// [-0, +0, -]
    #[inline(always)]
    fn tolstring(&self, index: Index, size: &mut usize) -> *const c_char {
        unsafe { lua_tolstring(self.state, index, size as *mut usize) }
    }

    /// [-0, +0, -]
    #[inline(always)]
    fn to_cfunction(&self, index: Index) -> lua_CFunction {
        unsafe { lua_tocfunction(self.state, index) }
    }

    /// Maps to `luaL_tolstring`.
    /// [-0, +1, -]
    #[inline(always)]
    fn cast_string(&self, index: Index) -> Option<&[u8]> {
        let mut len = 0;
        let ptr = unsafe { luaL_tolstring(self.state, index, &mut len) };
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { slice::from_raw_parts(ptr as *const u8, len as usize) })
        }
    }

    #[inline(always)]
    fn to_str<'a>(&'a self, index: Index) -> Option<&'a str> {
        self.to_bytes(index).and_then(|r| str::from_utf8(r).ok())
    }

    #[inline(always)]
    fn to_string_lossy<'a>(&'a self, index: Index) -> Option<Cow<'a, str>> {
        self.to_bytes(index).map(|r| String::from_utf8_lossy(r))
    }

    /// Maps to `lua_tolstring`, but allows arbitrary bytes.
    /// This function returns a reference to the string at the given index,
    /// on which `to_owned` may be called.
    fn to_bytes(&self, index: Index) -> Option<&[u8]> {
        let mut len = 0;
        let ptr = unsafe { lua_tolstring(self.state, index, &mut len) };
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { slice::from_raw_parts(ptr as *const u8, len as usize) })
        }
    }

    /// Maps to `luaL_argerror`.
    fn arg_error(&self, arg: Index, extramsg: &CStr) -> ! {
        unsafe { luaL_argerror(self.state, arg, extramsg.as_ptr()) };
        unreachable!()
    }

    /// Maps to `luaL_checknumber`.
    #[inline(always)]
    fn check_number(&self, arg: Index) -> lua_Number {
        unsafe { luaL_checknumber(self.state, arg) }
    }

    /// Maps to `luaL_optnumber`.
    #[inline(always)]
    fn opt_number(&self, arg: Index, def: lua_Number) -> lua_Number {
        unsafe { luaL_optnumber(self.state, arg, def) }
    }

    /// Maps to `luaL_checkinteger`.
    #[inline(always)]
    fn check_integer(&self, arg: Index) -> lua_Integer {
        unsafe { luaL_checkinteger(self.state, arg) }
    }

    /// Maps to `luaL_optinteger`.
    #[inline(always)]
    fn opt_integer(&self, arg: Index, def: lua_Integer) -> lua_Integer {
        unsafe { luaL_optinteger(self.state, arg, def) }
    }

    /// Maps to `luaL_checkstack`.
    fn check_stack_msg(&self, sz: c_int, msg: &str) {
        let c_str = CString::new(msg).unwrap();
        unsafe { luaL_checkstack(self.state, sz, c_str.as_ptr()) }
    }

    /// Maps to `luaL_checktype`.
    #[inline(always)]
    fn check_type(&self, arg: Index, t: Type) {
        unsafe { luaL_checktype(self.state, arg, t as c_int) }
    }

    /// Maps to `luaL_checkany`.
    #[inline(always)]
    fn check_any(&self, arg: Index) {
        unsafe { luaL_checkany(self.state, arg) }
    }

    /// Maps to `luaL_newmetatable`.
    #[inline(always)]
    fn new_metatable(&self, tname: &CStr) -> bool {
        unsafe { luaL_newmetatable(self.state, tname.as_ptr()) != 0 }
    }

    /// Maps to `luaL_setmetatable`.
    #[inline(always)]
    fn set_metatable_from_registry(&self, tname: &CStr) {
        unsafe { luaL_setmetatable(self.state, tname.as_ptr()) }
    }

    /// Maps to `luaL_testudata`.
    #[inline(always)]
    fn test_userdata(&self, arg: Index, tname: &CStr) -> *mut c_void {
        unsafe { luaL_testudata(self.state, arg, tname.as_ptr()) }
    }

    /// Convenience function that calls `test_userdata` and performs a cast.
    //#[unstable(reason="this is an experimental function")]
    #[inline(always)]
    unsafe fn test_userdata_typed<'a, T>(
        &'a mut self,
        arg: Index,
        tname: &CStr,
    ) -> Option<&'a mut T> {
        mem::transmute(self.test_userdata(arg, tname))
    }

    /// Maps to `luaL_checkudata`.
    #[inline(always)]
    fn checkudata<'a, T>(&'a self, arg: Index, tname: &CStr) -> &'a mut T {
        unsafe { mem::transmute(luaL_checkudata(self.state, arg, tname.as_ptr())) }
    }

    /// Maps to `luaL_where`. `where` is a reserved keyword.
    #[inline(always)]
    fn location(&self, lvl: c_int) {
        unsafe { luaL_where(self.state, lvl) }
    }

    // omitted: luaL_error

    /// Maps to `luaL_checkoption`.
    fn check_option(&self, arg: Index, def: Option<&str>, lst: &[&str]) -> usize {
        let mut vec: Vec<*const c_char> = Vec::with_capacity(lst.len() + 1);
        let cstrs: Vec<CString> = lst.iter().map(|ent| CString::new(*ent).unwrap()).collect();
        for ent in cstrs.iter() {
            vec.push(ent.as_ptr());
        }
        vec.push(ptr::null());
        let result = match def {
            Some(def) => unsafe {
                let c_str = CString::new(def).unwrap();
                luaL_checkoption(self.state, arg, c_str.as_ptr(), vec.as_ptr())
            },
            None => unsafe { luaL_checkoption(self.state, arg, ptr::null(), vec.as_ptr()) },
        };
        result as usize
    }

    /// luaL_ref [-1, +0, m]
    #[inline(always)]
    fn reference(&self, t: Index) -> Reference {
        let result = unsafe { luaL_ref(self.state, t) };
        Reference(result)
    }

    /// Maps to `luaL_unref`.
    #[inline(always)]
    fn unreference(&self, t: Index, reference: Reference) {
        unsafe { luaL_unref(self.state, t, reference.value()) }
    }

    /// Maps to `luaL_loadfilex`.
    fn load_filex(&self, filename: &str, mode: &str) -> i32 {
        unsafe {
            let filename_c_str = CString::new(filename).unwrap();
            let mode_c_str = CString::new(mode).unwrap();
            luaL_loadfilex(self.state, filename_c_str.as_ptr(), mode_c_str.as_ptr())
        }
    }

    /// Maps to `luaL_loadfile`.
    fn load_file(&self, filename: &str) -> i32 {
        let c_str = CString::new(filename).unwrap();
        unsafe { luaL_loadfile(self.state, c_str.as_ptr()) }
    }

    /// [-0, +1, -]
    fn load_buffer<F: AsRef<[u8]>>(&self, source: F, chunk_name: Option<&str>) -> i32 {
        let buffer = source.as_ref();
        let chunk = chunk_name.and_then(|name| CString::new(name).ok());
        unsafe {
            luaL_loadbuffer(
                self.state,
                buffer.as_ptr() as *const c_char,
                buffer.len(),
                chunk.as_ref().map(|s| s.as_ptr()).unwrap_or(ptr::null()),
            )
        }
    }

    /// Maps to `luaL_loadbufferx`.
    fn load_bufferx(&self, buff: &[u8], name: &str, mode: &str) -> i32 {
        let name_c_str = CString::new(name).unwrap();
        let mode_c_str = CString::new(mode).unwrap();
        unsafe {
            luaL_loadbufferx(
                self.state,
                buff.as_ptr() as *const _,
                buff.len() as size_t,
                name_c_str.as_ptr(),
                mode_c_str.as_ptr(),
            )
        }
    }

    /// Maps to `luaL_loadstring`.
    fn load_string(&self, source: &str) -> i32 {
        let c_str = CString::new(source).unwrap();
        unsafe { luaL_loadstring(self.state, c_str.as_ptr()) }
    }

    /// Maps to `lua_dump`.
    #[inline]
    fn dump(&self, mut writer: impl FnMut(&[u8]), strip: bool) -> c_int {
        use core::mem::transmute;
        unsafe extern "C" fn dump_wrapper(
            l: *mut lua_State,
            p: *const c_void,
            sz: usize,
            ud: *mut c_void,
        ) -> c_int {
            let callback = transmute::<_, &mut &mut dyn FnMut(&[u8])>(ud);
            callback(core::slice::from_raw_parts(p as *const u8, sz));
            0
        }
        let writer: &mut dyn FnMut(&[u8]) = &mut writer;
        unsafe { lua_dump(self.state, dump_wrapper, transmute(&writer), strip as c_int) }
    }

    /// Maps to `luaL_len`.
    fn len_direct(&self, index: Index) -> lua_Integer {
        unsafe { luaL_len(self.state, index) }
    }

    /// Maps to `luaL_gsub`.
    fn gsub(&self, s: &str, p: &str, r: &str) -> &str {
        let s_c_str = CString::new(s).unwrap();
        let p_c_str = CString::new(p).unwrap();
        let r_c_str = CString::new(r).unwrap();
        let ptr = unsafe {
            luaL_gsub(
                self.state,
                s_c_str.as_ptr(),
                p_c_str.as_ptr(),
                r_c_str.as_ptr(),
            )
        };
        let slice = unsafe { CStr::from_ptr(ptr).to_bytes() };
        str::from_utf8(slice).unwrap()
    }

    /// Maps to `luaL_setfuncs`.
    fn set_fns(&self, l: &[(&str, lua_CFunction)], nup: c_int) {
        let mut reg: Vec<luaL_Reg> = Vec::with_capacity(l.len() + 1);
        let ents: Vec<(CString, lua_CFunction)> = l
            .iter()
            .map(|&(s, f)| (CString::new(s).unwrap(), f))
            .collect();
        for &(ref s, f) in ents.iter() {
            reg.push(luaL_Reg {
                name: s.as_ptr(),
                func: f,
            });
        }
        reg.push(luaL_Reg {
            name: ptr::null(),
            func: None,
        });
        unsafe { luaL_setfuncs(self.state, reg.as_ptr(), nup) }
    }

    /// Maps to `luaL_getsubtable`.
    #[inline(always)]
    fn get_subtable(&self, idx: Index, fname: &CStr) -> bool {
        unsafe { luaL_getsubtable(self.state, idx, fname.as_ptr()) != 0 }
    }

    /// Maps to `luaL_traceback`.
    #[inline(always)]
    fn traceback(&self, state: *mut lua_State, msg: &CStr, level: c_int) {
        unsafe { luaL_traceback(self.state, state, msg.as_ptr(), level) }
    }

    /// Maps to `luaL_requiref`.
    #[inline(always)]
    fn requiref(&self, modname: &CStr, openf: CFunction, glb: bool) {
        unsafe { luaL_requiref(self.state, modname.as_ptr(), Some(openf), glb as c_int) }
    }

    /// Maps to `luaL_argcheck`.
    #[inline(always)]
    fn arg_check(&self, cond: bool, arg: Index, extramsg: &str) {
        let c_str = CString::new(extramsg).unwrap();
        unsafe { luaL_argcheck(self.state, cond as c_int, arg, c_str.as_ptr()) }
    }

    /// Maps to `luaL_checklstring`.
    fn check_string(&self, n: Index) -> &str {
        let mut size = 0;
        let ptr = unsafe { luaL_checklstring(self.state, n, &mut size) };
        let slice = unsafe { slice::from_raw_parts(ptr as *const u8, size as usize) };
        str::from_utf8(slice).unwrap()
    }

    /// Maps to `luaL_optlstring`.
    fn opt_string<'a>(&'a mut self, n: Index, default: &'a str) -> &'a str {
        let mut size = 0;
        let c_str = CString::new(default).unwrap();
        let ptr = unsafe { luaL_optlstring(self.state, n, c_str.as_ptr(), &mut size) };
        if ptr == c_str.as_ptr() {
            default
        } else {
            let slice = unsafe { slice::from_raw_parts(ptr as *const u8, size as usize) };
            str::from_utf8(slice).unwrap()
        }
    }

    /// Maps to `luaL_getmetatable`.
    #[inline(always)]
    fn get_metatable_from_registry(&self, tname: &str) {
        let c_str = CString::new(tname).unwrap();
        unsafe { luaL_getmetatable(self.state, c_str.as_ptr()) }
    }

    /// Before call push, stack should be checked by .check_stack()
    #[inline(always)]
    fn push<T: ToLua>(&self, value: T) -> Result<(), Error> {
        match T::__PUSH {
            Some(push) => {
                #[cfg(debug_assertions)]
                let top = self.get_top();
                push(value, self)?;
                #[cfg(debug_assertions)]
                assert_eq!(top + 1, self.get_top(), "{}", core::any::type_name::<T>());
            }
            None => {
                let top = self.get_top();
                self.push_value(value.to_lua(self)?.index);
                if !self.clear_with_keep_top_one(top) {
                    panic!(
                        "stack should be increased, top: {top} after: {}",
                        self.get_top()
                    );
                }
            }
        }
        Ok(())
    }
}

/// Arithmetic operations for `lua_arith`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arithmetic {
    Add = LUA_OPADD as isize,
    Sub = LUA_OPSUB as isize,
    Mul = LUA_OPMUL as isize,
    Mod = LUA_OPMOD as isize,
    Pow = LUA_OPPOW as isize,
    Div = LUA_OPDIV as isize,
    IDiv = LUA_OPIDIV as isize,
    BAnd = LUA_OPBAND as isize,
    BOr = LUA_OPBOR as isize,
    BXor = LUA_OPBXOR as isize,
    Shl = LUA_OPSHL as isize,
    Shr = LUA_OPSHR as isize,
    Unm = LUA_OPUNM as isize,
    BNot = LUA_OPBNOT as isize,
}

/// Comparison operations for `lua_compare`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comparison {
    Eq = LUA_OPEQ as isize,
    Lt = LUA_OPLT as isize,
    Le = LUA_OPLE as isize,
}

/// Represents all possible Lua data types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    None = LUA_TNONE as isize,
    Nil = LUA_TNIL as isize,
    Boolean = LUA_TBOOLEAN as isize,
    LightUserdata = LUA_TLIGHTUSERDATA as isize,
    Number = LUA_TNUMBER as isize,
    String = LUA_TSTRING as isize,
    Table = LUA_TTABLE as isize,
    Function = LUA_TFUNCTION as isize,
    Userdata = LUA_TUSERDATA as isize,
    Thread = LUA_TTHREAD as isize,
    Invalid,
}

impl core::fmt::Display for Type {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Type::None => write!(f, "none"),
            Type::Nil => write!(f, "nil"),
            Type::Boolean => write!(f, "boolean"),
            Type::LightUserdata => write!(f, "lightuserdata"),
            Type::Number => write!(f, "number"),
            Type::String => write!(f, "string"),
            Type::Table => write!(f, "table"),
            Type::Function => write!(f, "function"),
            Type::Userdata => write!(f, "userdata"),
            Type::Thread => write!(f, "thread"),
            Type::Invalid => write!(f, "invalid"),
        }
    }
}

impl Type {
    fn from_c_int(i: c_int) -> Type {
        match i {
            LUA_TNIL => Type::Nil,
            LUA_TBOOLEAN => Type::Boolean,
            LUA_TLIGHTUSERDATA => Type::LightUserdata,
            LUA_TNUMBER => Type::Number,
            LUA_TSTRING => Type::String,
            LUA_TTABLE => Type::Table,
            LUA_TFUNCTION => Type::Function,
            LUA_TUSERDATA => Type::Userdata,
            LUA_TTHREAD => Type::Thread,
            _ => Type::Invalid,
        }
    }

    pub fn is_none_or_nil(&self) -> bool {
        matches!(*self, Type::None | Type::Nil)
    }
}

/// Options for the Lua garbage collector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GcOption {
    Stop = LUA_GCSTOP as isize,
    Restart = LUA_GCRESTART as isize,
    Collect = LUA_GCCOLLECT as isize,
    Count = LUA_GCCOUNT as isize,
    CountBytes = LUA_GCCOUNTB as isize,
    Step = LUA_GCSTEP as isize,
    SetPause = LUA_GCSETPAUSE as isize,
    SetStepMul = LUA_GCSETSTEPMUL as isize,
    IsRunning = LUA_GCISRUNNING as isize,
}

/// Mode of the Lua garbage collector (GC)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GCMode {
    Incremental,
    Generational,
}

/// Status of a Lua state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreadStatus {
    Ok = LUA_OK as isize,
    Yield = LUA_YIELD as isize,
    RuntimeError = LUA_ERRRUN as isize,
    SyntaxError = LUA_ERRSYNTAX as isize,
    MemoryError = LUA_ERRMEM as isize,
    // GcError = LUA_ERRGCMM as isize,
    MessageHandlerError = LUA_ERRERR as isize,
    FileError = LUA_ERRFILE as isize,
}

impl ThreadStatus {
    pub(crate) fn from_c_int(i: c_int) -> ThreadStatus {
        match i {
            LUA_OK => ThreadStatus::Ok,
            LUA_YIELD => ThreadStatus::Yield,
            LUA_ERRRUN => ThreadStatus::RuntimeError,
            LUA_ERRSYNTAX => ThreadStatus::SyntaxError,
            LUA_ERRMEM => ThreadStatus::MemoryError,
            // LUA_ERRGCMM => ThreadStatus::GcError,
            LUA_ERRERR => ThreadStatus::MessageHandlerError,
            LUA_ERRFILE => ThreadStatus::FileError,
            _ => panic!("Unknown Lua error code: {}", i),
        }
    }

    pub fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }

    /// Returns `true` for error statuses and `false` for `Ok` and `Yield`.
    pub fn is_err(self) -> bool {
        match self {
            ThreadStatus::RuntimeError
            | ThreadStatus::SyntaxError
            | ThreadStatus::MemoryError
            // | ThreadStatus::GcError
            | ThreadStatus::MessageHandlerError
            | ThreadStatus::FileError => true,
            ThreadStatus::Ok | ThreadStatus::Yield => false,
        }
    }
}

/// Type of Lua references generated through `reference` and `unreference`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Reference(pub c_int);

impl Default for Reference {
    fn default() -> Self {
        NOREF
    }
}

/// The result of `reference` for `nil` values.
pub const REFNIL: Reference = Reference(LUA_REFNIL);

/// A value that will never be returned by `reference`.
pub const NOREF: Reference = Reference(LUA_REFNIL);

impl Reference {
    /// Returns `true` if this reference is equal to `REFNIL`.
    pub fn is_nil_ref(self) -> bool {
        self == REFNIL
    }

    /// Returns `true` if this reference is equal to `NOREF`.
    pub fn is_no_ref(self) -> bool {
        self == NOREF
    }

    /// Convenience function that returns the value of this reference.
    pub fn value(self) -> c_int {
        let Reference(value) = self;
        value
    }
}

impl From<c_int> for Reference {
    fn from(i: c_int) -> Self {
        Self(i)
    }
}

#[cfg(features = "std")]
bitflags::bitflags! {
    #[doc="Hook point masks for `lua_sethook`."]
    flags HookMask: c_int {
        #[doc="Called when the interpreter calls a function."]
        const MASKCALL  = LUA_MASKCALL,
        #[doc="Called when the interpreter returns from a function."]
        const MASKRET   = LUA_MASKRET,
        #[doc="Called when the interpreter is about to start the execution of a new line of code."]
        const MASKLINE  = LUA_MASKLINE,
        #[doc="Called after the interpreter executes every `count` instructions."]
        const MASKCOUNT = LUA_MASKCOUNT
    }
}

impl lua_Debug {
    pub fn source(&self) -> Option<Cow<str>> {
        if self.source.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(self.source).to_string_lossy() })
        }
    }

    pub fn short_src(&self) -> Cow<str> {
        unsafe { CStr::from_ptr(self.short_src.as_ptr()).to_string_lossy() }
    }

    pub fn name(&self) -> Option<Cow<str>> {
        if self.name.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(self.name).to_string_lossy() })
        }
    }

    pub fn what(&self) -> Option<Cow<str>> {
        if self.what.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(self.what).to_string_lossy() })
        }
    }

    pub fn namewhat(&self) -> Option<Cow<str>> {
        if self.namewhat.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(self.namewhat).to_string_lossy() })
        }
    }
}
