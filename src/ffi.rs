#![allow(non_snake_case, non_upper_case_globals, non_camel_case_types)]

use core::ffi::{c_char, c_int, c_long, c_uchar, c_void};
use core::mem::size_of;
use core::ptr;

type size_t = usize;

pub use crate::luaconf::{LUAI_MAXSTACK, LUA_IDSIZE};

pub const LUA_VERSION_NUM: i32 = 504;
pub const LUA_EXTRASPACE: i32 = size_of::<usize>() as i32;
pub type LUA_NUMBER = f64;
pub type LUA_INTEGER = i64;
pub type LUA_UNSIGNED = u32;
pub type LUA_KCONTEXT = isize;
pub const LUA_REGISTRYINDEX: i32 = -(LUAI_MAXSTACK as i32) - 1000;
pub const LUAL_NUMSIZES: usize = size_of::<LUA_INTEGER>() * 16 + size_of::<LUA_NUMBER>();
pub const LUAL_BUFFERSIZE: usize = 80 * size_of::<usize>() * size_of::<LUA_INTEGER>();

pub const LUA_SIGNATURE: &'static [u8] = b"\x1bLua";

// option for multiple returns in 'lua_pcall' and 'lua_call'
pub const LUA_MULTRET: c_int = -1;

#[inline(always)]
pub const fn lua_upvalueindex(i: c_int) -> c_int {
    LUA_REGISTRYINDEX - i
}

// thread status
pub const LUA_OK: c_int = 0;
pub const LUA_YIELD: c_int = 1;
pub const LUA_ERRRUN: c_int = 2;
pub const LUA_ERRSYNTAX: c_int = 3;
pub const LUA_ERRMEM: c_int = 4;
pub const LUA_ERRGCMM: c_int = 5;
pub const LUA_ERRERR: c_int = 6;

pub type lua_State = c_void;

// basic types
pub const LUA_TNONE: c_int = -1;

pub const LUA_TNIL: c_int = 0;
pub const LUA_TBOOLEAN: c_int = 1;
pub const LUA_TLIGHTUSERDATA: c_int = 2;
pub const LUA_TNUMBER: c_int = 3;
pub const LUA_TSTRING: c_int = 4;
pub const LUA_TTABLE: c_int = 5;
pub const LUA_TFUNCTION: c_int = 6;
pub const LUA_TUSERDATA: c_int = 7;
pub const LUA_TTHREAD: c_int = 8;

pub const LUA_NUMTAGS: c_int = 9;

// minimum stack available to a C function
pub const LUA_MINSTACK: c_int = 20;

// predefined values in the registry
pub const LUA_RIDX_MAINTHREAD: lua_Integer = 1;
pub const LUA_RIDX_GLOBALS: lua_Integer = 2;
pub const LUA_RIDX_LAST: lua_Integer = LUA_RIDX_GLOBALS;

/// A Lua number, usually equivalent to `f64`.
pub type lua_Number = LUA_NUMBER;

/// A Lua integer, usually equivalent to `i64`.
pub type lua_Integer = LUA_INTEGER;

// unsigned integer type
pub type lua_Unsigned = LUA_UNSIGNED;

// type for continuation-function contexts
pub type lua_KContext = LUA_KCONTEXT;

/// Type for native functions that can be passed to Lua.
pub type CFunction = unsafe extern "C" fn(L: *mut lua_State) -> c_int;
pub type lua_CFunction = Option<CFunction>;

// Type for continuation functions
pub type lua_KFunction =
    Option<unsafe extern "C" fn(L: *mut lua_State, status: c_int, ctx: lua_KContext) -> c_int>;

// Type for functions that read/write blocks when loading/dumping Lua chunks
pub type lua_Reader = Option<
    unsafe extern "C" fn(L: *mut lua_State, ud: *mut c_void, sz: *mut size_t) -> *const c_char,
>;
pub type lua_Writer = Option<
    unsafe extern "C" fn(L: *mut lua_State, p: *const c_void, sz: size_t, ud: *mut c_void) -> c_int,
>;

/// Type for memory-allocation functions.
pub type lua_Alloc = Option<
    unsafe extern "C" fn(
        ud: *mut c_void,
        ptr: *mut c_void,
        osize: size_t,
        nsize: size_t,
    ) -> *mut c_void,
>;

extern "C" {
    // state manipulation
    pub fn lua_newstate(f: lua_Alloc, ud: *mut c_void) -> *mut lua_State;
    pub fn lua_close(L: *mut lua_State);
    pub fn lua_newthread(L: *mut lua_State) -> *mut lua_State;

    pub fn lua_atpanic(L: *mut lua_State, panicf: lua_CFunction) -> lua_CFunction;

    pub fn lua_version(L: *mut lua_State) -> *const lua_Number;

    // basic stack manipulation
    pub fn lua_absindex(L: *mut lua_State, idx: c_int) -> c_int;
    pub fn lua_gettop(L: *mut lua_State) -> c_int;
    pub fn lua_settop(L: *mut lua_State, idx: c_int);
    pub fn lua_pushvalue(L: *mut lua_State, idx: c_int);
    pub fn lua_rotate(L: *mut lua_State, idx: c_int, n: c_int);
    pub fn lua_copy(L: *mut lua_State, fromidx: c_int, toidx: c_int);
    pub fn lua_checkstack(L: *mut lua_State, sz: c_int) -> c_int;

    pub fn lua_xmove(from: *mut lua_State, to: *mut lua_State, n: c_int);

    // access functions (stack -> C)
    pub fn lua_isnumber(L: *mut lua_State, idx: c_int) -> c_int;
    pub fn lua_isstring(L: *mut lua_State, idx: c_int) -> c_int;
    pub fn lua_iscfunction(L: *mut lua_State, idx: c_int) -> c_int;
    pub fn lua_isinteger(L: *mut lua_State, idx: c_int) -> c_int;
    pub fn lua_isuserdata(L: *mut lua_State, idx: c_int) -> c_int;
    pub fn lua_type(L: *mut lua_State, idx: c_int) -> c_int;
    pub fn lua_typename(L: *mut lua_State, tp: c_int) -> *const c_char;

    pub fn lua_tonumberx(L: *mut lua_State, idx: c_int, isnum: *mut c_int) -> lua_Number;
    pub fn lua_tointegerx(L: *mut lua_State, idx: c_int, isnum: *mut c_int) -> lua_Integer;
    pub fn lua_toboolean(L: *mut lua_State, idx: c_int) -> c_int;
    pub fn lua_tolstring(L: *mut lua_State, idx: c_int, len: *mut size_t) -> *const c_char;
    pub fn lua_rawlen(L: *mut lua_State, idx: c_int) -> size_t;
    pub fn lua_tocfunction(L: *mut lua_State, idx: c_int) -> lua_CFunction;
    pub fn lua_touserdata(L: *mut lua_State, idx: c_int) -> *mut c_void;
    pub fn lua_tothread(L: *mut lua_State, idx: c_int) -> *mut lua_State;
    pub fn lua_topointer(L: *mut lua_State, idx: c_int) -> *const c_void;
}

// Comparison and arithmetic functions
pub const LUA_OPADD: c_int = 0;
pub const LUA_OPSUB: c_int = 1;
pub const LUA_OPMUL: c_int = 2;
pub const LUA_OPMOD: c_int = 3;
pub const LUA_OPPOW: c_int = 4;
pub const LUA_OPDIV: c_int = 5;
pub const LUA_OPIDIV: c_int = 6;
pub const LUA_OPBAND: c_int = 7;
pub const LUA_OPBOR: c_int = 8;
pub const LUA_OPBXOR: c_int = 9;
pub const LUA_OPSHL: c_int = 10;
pub const LUA_OPSHR: c_int = 11;
pub const LUA_OPUNM: c_int = 12;
pub const LUA_OPBNOT: c_int = 13;

extern "C" {
    pub fn lua_arith(L: *mut lua_State, op: c_int);
}

pub const LUA_OPEQ: c_int = 0;
pub const LUA_OPLT: c_int = 1;
pub const LUA_OPLE: c_int = 2;

extern "C" {
    pub fn lua_rawequal(L: *mut lua_State, idx1: c_int, idx2: c_int) -> c_int;
    pub fn lua_compare(L: *mut lua_State, idx1: c_int, idx2: c_int, op: c_int) -> c_int;
}

// push functions (C -> stack)
extern "C" {
    pub fn lua_pushnil(L: *mut lua_State);
    pub fn lua_pushnumber(L: *mut lua_State, n: lua_Number);
    pub fn lua_pushinteger(L: *mut lua_State, n: lua_Integer);
    pub fn lua_pushlstring(L: *mut lua_State, s: *const c_char, l: size_t) -> *const c_char;
    pub fn lua_pushstring(L: *mut lua_State, s: *const c_char) -> *const c_char;
    // TODO: omitted:
    // lua_pushvfstring
    pub fn lua_pushfstring(L: *mut lua_State, fmt: *const c_char, ...) -> *const c_char;
    pub fn lua_pushcclosure(L: *mut lua_State, f: lua_CFunction, n: c_int);
    pub fn lua_pushboolean(L: *mut lua_State, b: c_int);
    pub fn lua_pushlightuserdata(L: *mut lua_State, p: *mut c_void);
    pub fn lua_pushthread(L: *mut lua_State) -> c_int;
}

// get functions (Lua -> stack)
extern "C" {
    pub fn lua_getglobal(L: *mut lua_State, var: *const c_char) -> c_int;
    pub fn lua_gettable(L: *mut lua_State, idx: c_int) -> c_int;
    pub fn lua_getfield(L: *mut lua_State, idx: c_int, k: *const c_char) -> c_int;
    pub fn lua_geti(L: *mut lua_State, idx: c_int, n: lua_Integer) -> c_int;
    pub fn lua_rawget(L: *mut lua_State, idx: c_int) -> c_int;
    pub fn lua_rawgeti(L: *mut lua_State, idx: c_int, n: lua_Integer) -> c_int;
    pub fn lua_rawgetp(L: *mut lua_State, idx: c_int, p: *const c_void) -> c_int;

    pub fn lua_createtable(L: *mut lua_State, narr: c_int, nrec: c_int);
    pub fn lua_newuserdatauv(L: *mut lua_State, sz: size_t, n: i32) -> *mut c_void;
    pub fn lua_getmetatable(L: *mut lua_State, objindex: c_int) -> c_int;
    pub fn lua_getiuservalue(L: *mut lua_State, idx: c_int, n: i32) -> c_int;
}

// set functions (stack -> Lua)
extern "C" {
    pub fn lua_setglobal(L: *mut lua_State, var: *const c_char);
    pub fn lua_settable(L: *mut lua_State, idx: c_int);
    pub fn lua_setfield(L: *mut lua_State, idx: c_int, k: *const c_char);
    pub fn lua_seti(L: *mut lua_State, idx: c_int, n: lua_Integer);
    pub fn lua_rawset(L: *mut lua_State, idx: c_int);
    pub fn lua_rawseti(L: *mut lua_State, idx: c_int, n: lua_Integer);
    pub fn lua_rawsetp(L: *mut lua_State, idx: c_int, p: *const c_void);
    pub fn lua_setmetatable(L: *mut lua_State, objindex: c_int) -> c_int;
    pub fn lua_setiuservalue(L: *mut lua_State, idx: c_int, n: i32) -> c_int;
}

// 'load' and 'call' functions (load and run Lua code)
extern "C" {
    pub fn lua_callk(
        L: *mut lua_State,
        nargs: c_int,
        nresults: c_int,
        ctx: lua_KContext,
        k: lua_KFunction,
    );
    pub fn lua_pcallk(
        L: *mut lua_State,
        nargs: c_int,
        nresults: c_int,
        errfunc: c_int,
        ctx: lua_KContext,
        k: lua_KFunction,
    ) -> c_int;
    pub fn lua_load(
        L: *mut lua_State,
        reader: lua_Reader,
        dt: *mut c_void,
        chunkname: *const c_char,
        mode: *const c_char,
    ) -> c_int;
    pub fn lua_dump(
        L: *mut lua_State,
        writer: lua_Writer,
        data: *mut c_void,
        strip: c_int,
    ) -> c_int;
}

#[inline(always)]
pub unsafe fn lua_call(L: *mut lua_State, n: c_int, r: c_int) {
    lua_callk(L, n, r, 0, None)
}

#[inline(always)]
pub unsafe fn lua_pcall(L: *mut lua_State, n: c_int, r: c_int, f: c_int) -> c_int {
    lua_pcallk(L, n, r, f, 0, None)
}

// coroutine functions
extern "C" {
    pub fn lua_yieldk(
        L: *mut lua_State,
        nresults: c_int,
        ctx: lua_KContext,
        k: lua_KFunction,
    ) -> c_int;
    pub fn lua_resume(
        L: *mut lua_State,
        from: *mut lua_State,
        narg: c_int,
        nresults: *mut c_int,
    ) -> c_int;
    pub fn lua_status(L: *mut lua_State) -> c_int;
    pub fn lua_isyieldable(L: *mut lua_State) -> c_int;
}

#[inline(always)]
pub unsafe fn lua_yield(L: *mut lua_State, n: c_int) -> c_int {
    lua_yieldk(L, n, 0, None)
}

// garbage-collection function and options
pub const LUA_GCSTOP: c_int = 0;
pub const LUA_GCRESTART: c_int = 1;
pub const LUA_GCCOLLECT: c_int = 2;
pub const LUA_GCCOUNT: c_int = 3;
pub const LUA_GCCOUNTB: c_int = 4;
pub const LUA_GCSTEP: c_int = 5;
pub const LUA_GCSETPAUSE: c_int = 6;
pub const LUA_GCSETSTEPMUL: c_int = 7;
pub const LUA_GCISRUNNING: c_int = 9;

extern "C" {
    pub fn lua_gc(L: *mut lua_State, what: c_int, data: c_int) -> c_int;
}

// miscellaneous functions
extern "C" {
    pub fn lua_error(L: *mut lua_State) -> c_int;
    pub fn lua_next(L: *mut lua_State, idx: c_int) -> c_int;
    pub fn lua_concat(L: *mut lua_State, n: c_int);
    pub fn lua_len(L: *mut lua_State, idx: c_int);
    pub fn lua_stringtonumber(L: *mut lua_State, s: *const c_char) -> size_t;
    pub fn lua_getallocf(L: *mut lua_State, ud: *mut *mut c_void) -> lua_Alloc;
    pub fn lua_setallocf(L: *mut lua_State, f: lua_Alloc, ud: *mut c_void);
}

#[inline(always)]
pub unsafe fn lua_newuserdata(L: *mut lua_State, size: usize) -> *mut c_void {
    lua_newuserdatauv(L, size, 1)
}

#[inline(always)]
pub unsafe fn lua_getuservalue(L: *mut lua_State, i: c_int) -> i32 {
    lua_getiuservalue(L, i, 1)
}

#[inline(always)]
pub unsafe fn lua_setuservalue(L: *mut lua_State, i: c_int) -> i32 {
    lua_setiuservalue(L, i, 1)
}

// some useful macros
// here, implemented as Rust functions
#[inline(always)]
pub unsafe fn lua_getextraspace(L: *mut lua_State) -> *mut c_void {
    core::mem::transmute(L as usize - LUA_EXTRASPACE as usize)
}

#[inline(always)]
pub unsafe fn lua_tonumber(L: *mut lua_State, i: c_int) -> lua_Number {
    lua_tonumberx(L, i, ptr::null_mut())
}

#[inline(always)]
pub unsafe fn lua_tointeger(L: *mut lua_State, i: c_int) -> lua_Integer {
    lua_tointegerx(L, i, ptr::null_mut())
}

#[inline(always)]
pub unsafe fn lua_pop(L: *mut lua_State, n: c_int) {
    lua_settop(L, -n - 1)
}

#[inline(always)]
pub unsafe fn lua_newtable(L: *mut lua_State) {
    lua_createtable(L, 0, 0)
}

#[inline(always)]
pub unsafe fn lua_register(L: *mut lua_State, n: *const c_char, f: lua_CFunction) {
    lua_pushcfunction(L, f);
    lua_setglobal(L, n)
}

#[inline(always)]
pub unsafe fn lua_pushcfunction(L: *mut lua_State, f: lua_CFunction) {
    lua_pushcclosure(L, f, 0)
}

#[inline(always)]
pub unsafe fn lua_isfunction(L: *mut lua_State, n: c_int) -> c_int {
    (lua_type(L, n) == LUA_TFUNCTION) as c_int
}

#[inline(always)]
pub unsafe fn lua_istable(L: *mut lua_State, n: c_int) -> c_int {
    (lua_type(L, n) == LUA_TTABLE) as c_int
}

#[inline(always)]
pub unsafe fn lua_islightuserdata(L: *mut lua_State, n: c_int) -> c_int {
    (lua_type(L, n) == LUA_TLIGHTUSERDATA) as c_int
}

#[inline(always)]
pub unsafe fn lua_isnil(L: *mut lua_State, n: c_int) -> c_int {
    (lua_type(L, n) == LUA_TNIL) as c_int
}

#[inline(always)]
pub unsafe fn lua_isboolean(L: *mut lua_State, n: c_int) -> c_int {
    (lua_type(L, n) == LUA_TBOOLEAN) as c_int
}

#[inline(always)]
pub unsafe fn lua_isthread(L: *mut lua_State, n: c_int) -> c_int {
    (lua_type(L, n) == LUA_TTHREAD) as c_int
}

#[inline(always)]
pub unsafe fn lua_isnone(L: *mut lua_State, n: c_int) -> c_int {
    (lua_type(L, n) == LUA_TNONE) as c_int
}

#[inline(always)]
pub unsafe fn lua_isnoneornil(L: *mut lua_State, n: c_int) -> c_int {
    (lua_type(L, n) <= 0) as c_int
}

// TODO: Test
#[inline(always)]
pub unsafe fn lua_pushliteral(L: *mut lua_State, s: &'static str) -> *const c_char {
    let c_str = alloc::ffi::CString::new(s).unwrap();
    lua_pushlstring(L, c_str.as_ptr(), c_str.as_bytes().len() as size_t)
}

#[inline(always)]
pub unsafe fn lua_pushglobaltable(L: *mut lua_State) -> c_int {
    lua_rawgeti(L, LUA_REGISTRYINDEX, LUA_RIDX_GLOBALS)
}

#[inline(always)]
pub unsafe fn lua_tostring(L: *mut lua_State, i: c_int) -> *const c_char {
    lua_tolstring(L, i, ptr::null_mut())
}

#[inline(always)]
pub unsafe fn lua_insert(L: *mut lua_State, idx: c_int) {
    lua_rotate(L, idx, 1)
}

#[inline(always)]
pub unsafe fn lua_remove(L: *mut lua_State, idx: c_int) {
    lua_rotate(L, idx, -1);
    lua_pop(L, 1)
}

#[inline(always)]
pub unsafe fn lua_replace(L: *mut lua_State, idx: c_int) {
    lua_copy(L, -1, idx);
    lua_pop(L, 1)
}

// Debug API
// Event codes
pub const LUA_HOOKCALL: c_int = 0;
pub const LUA_HOOKRET: c_int = 1;
pub const LUA_HOOKLINE: c_int = 2;
pub const LUA_HOOKCOUNT: c_int = 3;
pub const LUA_HOOKTAILCALL: c_int = 4;

// Event masks
pub const LUA_MASKCALL: c_int = 1 << (LUA_HOOKCALL as usize);
pub const LUA_MASKRET: c_int = 1 << (LUA_HOOKRET as usize);
pub const LUA_MASKLINE: c_int = 1 << (LUA_HOOKLINE as usize);
pub const LUA_MASKCOUNT: c_int = 1 << (LUA_HOOKCOUNT as usize);

/// Type for functions to be called on debug events.
pub type lua_Hook = Option<extern "C" fn(L: *mut lua_State, ar: *mut lua_Debug)>;

extern "C" {
    pub fn lua_getstack(L: *mut lua_State, level: c_int, ar: *mut lua_Debug) -> c_int;
    pub fn lua_getinfo(L: *mut lua_State, what: *const c_char, ar: *mut lua_Debug) -> c_int;
    pub fn lua_getlocal(L: *mut lua_State, ar: *const lua_Debug, n: c_int) -> *const c_char;
    pub fn lua_setlocal(L: *mut lua_State, ar: *const lua_Debug, n: c_int) -> *const c_char;
    pub fn lua_getupvalue(L: *mut lua_State, funcindex: c_int, n: c_int) -> *const c_char;
    pub fn lua_setupvalue(L: *mut lua_State, funcindex: c_int, n: c_int) -> *const c_char;

    pub fn lua_upvalueid(L: *mut lua_State, fidx: c_int, n: c_int) -> *mut c_void;
    pub fn lua_upvaluejoin(L: *mut lua_State, fidx1: c_int, n1: c_int, fidx2: c_int, n2: c_int);

    pub fn lua_sethook(L: *mut lua_State, func: lua_Hook, mask: c_int, count: c_int);
    pub fn lua_gethook(L: *mut lua_State) -> lua_Hook;
    pub fn lua_gethookmask(L: *mut lua_State) -> c_int;
    pub fn lua_gethookcount(L: *mut lua_State) -> c_int;
}

#[repr(C)]
pub struct lua_Debug {
    pub event: c_int,
    pub name: *const c_char,
    pub namewhat: *const c_char,
    pub what: *const c_char,
    pub source: *const c_char,
    pub srclen: usize,
    pub currentline: c_int,
    pub linedefined: c_int,
    pub lastlinedefined: c_int,
    pub nups: c_uchar,
    pub nparams: c_uchar,
    pub isvararg: c_char,
    pub istailcall: c_char,
    pub ftransfer: u16, /* (r) index of first value transferred */
    pub ntransfer: u16,
    pub short_src: [c_char; LUA_IDSIZE as usize],
    // private part
    i_ci: *mut c_void,
}

extern "C" {
    pub fn luaopen_base(L: *mut lua_State) -> c_int;
    pub fn luaopen_coroutine(L: *mut lua_State) -> c_int;
    pub fn luaopen_table(L: *mut lua_State) -> c_int;
    pub fn luaopen_io(L: *mut lua_State) -> c_int;
    pub fn luaopen_os(L: *mut lua_State) -> c_int;
    pub fn luaopen_string(L: *mut lua_State) -> c_int;
    pub fn luaopen_utf8(L: *mut lua_State) -> c_int;
    pub fn luaopen_math(L: *mut lua_State) -> c_int;
    pub fn luaopen_debug(L: *mut lua_State) -> c_int;
    pub fn luaopen_package(L: *mut lua_State) -> c_int;

    pub fn luaL_openlibs(L: *mut lua_State);
}

// extra error code for 'luaL_load'
pub const LUA_ERRFILE: c_int = LUA_ERRERR + 1;

#[repr(C)]
pub struct luaL_Reg {
    pub name: *const c_char,
    pub func: lua_CFunction,
}

#[inline(always)]
pub unsafe fn luaL_checkversion(L: *mut lua_State) {
    luaL_checkversion_(L, LUA_VERSION_NUM as lua_Number, LUAL_NUMSIZES as size_t)
}

extern "C" {
    pub fn luaL_checkversion_(L: *mut lua_State, ver: lua_Number, sz: size_t);

    pub fn luaL_getmetafield(L: *mut lua_State, obj: c_int, e: *const c_char) -> c_int;
    pub fn luaL_callmeta(L: *mut lua_State, obj: c_int, e: *const c_char) -> c_int;
    pub fn luaL_tolstring(L: *mut lua_State, idx: c_int, len: *mut size_t) -> *const c_char;
    pub fn luaL_argerror(L: *mut lua_State, arg: c_int, l: *const c_char) -> c_int;
    pub fn luaL_typeerror(L: *mut lua_State, arg: c_int, l: *const c_char) -> c_int;
    pub fn luaL_checklstring(L: *mut lua_State, arg: c_int, l: *mut size_t) -> *const c_char;
    pub fn luaL_optlstring(
        L: *mut lua_State,
        arg: c_int,
        def: *const c_char,
        l: *mut size_t,
    ) -> *const c_char;
    pub fn luaL_checknumber(L: *mut lua_State, arg: c_int) -> lua_Number;
    pub fn luaL_optnumber(L: *mut lua_State, arg: c_int, def: lua_Number) -> lua_Number;
    pub fn luaL_checkinteger(L: *mut lua_State, arg: c_int) -> lua_Integer;
    pub fn luaL_optinteger(L: *mut lua_State, arg: c_int, def: lua_Integer) -> lua_Integer;

    pub fn luaL_checkstack(L: *mut lua_State, sz: c_int, msg: *const c_char);
    pub fn luaL_checktype(L: *mut lua_State, arg: c_int, t: c_int);
    pub fn luaL_checkany(L: *mut lua_State, arg: c_int);

    pub fn luaL_newmetatable(L: *mut lua_State, tname: *const c_char) -> c_int;
    pub fn luaL_setmetatable(L: *mut lua_State, tname: *const c_char);
    pub fn luaL_testudata(L: *mut lua_State, ud: c_int, tname: *const c_char) -> *mut c_void;
    pub fn luaL_checkudata(L: *mut lua_State, ud: c_int, tname: *const c_char) -> *mut c_void;

    pub fn luaL_where(L: *mut lua_State, lvl: c_int);
    pub fn luaL_error(L: *mut lua_State, fmt: *const c_char, ...) -> c_int;

    // TODO: test this
    pub fn luaL_checkoption(
        L: *mut lua_State,
        arg: c_int,
        def: *const c_char,
        lst: *const *const c_char,
    ) -> c_int;

    pub fn luaL_fileresult(L: *mut lua_State, stat: c_int, fname: *const c_char) -> c_int;
    pub fn luaL_execresult(L: *mut lua_State, stat: c_int) -> c_int;
}

// pre-defined references
pub const LUA_NOREF: c_int = -2;
pub const LUA_REFNIL: c_int = -1;

extern "C" {
    pub fn luaL_ref(L: *mut lua_State, t: c_int) -> c_int;
    pub fn luaL_unref(L: *mut lua_State, t: c_int, r: c_int);

    pub fn luaL_loadfilex(L: *mut lua_State, filename: *const c_char, mode: *const c_char)
        -> c_int;
}

#[inline(always)]
pub unsafe fn luaL_loadfile(L: *mut lua_State, f: *const c_char) -> c_int {
    luaL_loadfilex(L, f, ptr::null())
}

extern "C" {
    pub fn luaL_loadbufferx(
        L: *mut lua_State,
        buff: *const c_char,
        sz: size_t,
        name: *const c_char,
        mode: *const c_char,
    ) -> c_int;
    pub fn luaL_loadstring(L: *mut lua_State, s: *const c_char) -> c_int;

    pub fn luaL_newstate() -> *mut lua_State;

    pub fn luaL_len(L: *mut lua_State, idx: c_int) -> lua_Integer;

    pub fn luaL_gsub(
        L: *mut lua_State,
        s: *const c_char,
        p: *const c_char,
        r: *const c_char,
    ) -> *const c_char;

    pub fn luaL_setfuncs(L: *mut lua_State, l: *const luaL_Reg, nup: c_int);

    pub fn luaL_getsubtable(L: *mut lua_State, idx: c_int, fname: *const c_char) -> c_int;

    pub fn luaL_traceback(L: *mut lua_State, L1: *mut lua_State, msg: *const c_char, level: c_int);

    pub fn luaL_requiref(
        L: *mut lua_State,
        modname: *const c_char,
        openf: lua_CFunction,
        glb: c_int,
    );
}

#[inline(always)]
#[allow(unused_variables)]
pub unsafe fn luaL_newlibtable(L: *mut lua_State, l: *const luaL_Reg) {
    // TODO: figure out how to pass an appropriate hint for the second param
    // this involves correcting the second parameter's type; in C this is
    // sizeof(l)/sizeof(l[0])
    lua_createtable(L, 0, 0)
}

#[inline(always)]
pub unsafe fn luaL_newlib(L: *mut lua_State, l: *const luaL_Reg) {
    luaL_checkversion(L);
    luaL_newlibtable(L, l);
    luaL_setfuncs(L, l, 0)
}

#[inline(always)]
pub unsafe fn luaL_argcheck(L: *mut lua_State, cond: c_int, arg: c_int, extramsg: *const c_char) {
    if cond == 0 {
        luaL_argerror(L, arg, extramsg);
    }
}

#[inline(always)]
pub unsafe fn luaL_checkstring(L: *mut lua_State, n: c_int) -> *const c_char {
    luaL_checklstring(L, n, ptr::null_mut())
}

#[inline(always)]
pub unsafe fn luaL_optstring(L: *mut lua_State, n: c_int, d: *const c_char) -> *const c_char {
    luaL_optlstring(L, n, d, ptr::null_mut())
}

// From 5.3 user manual:
// Macros to project non-default integer types (luaL_checkint, luaL_optint,
// luaL_checklong, luaL_optlong) were deprecated. Use their equivalent over
// lua_Integer with a type cast (or, when possible, use lua_Integer in your
// code).
#[inline(always)]
//#[deprecated]
pub unsafe fn luaL_checkint(L: *mut lua_State, n: c_int) -> c_int {
    luaL_checkinteger(L, n) as c_int
}

#[inline(always)]
//#[deprecated]
pub unsafe fn luaL_optint(L: *mut lua_State, n: c_int, d: c_int) -> c_int {
    luaL_optinteger(L, n, d as lua_Integer) as c_int
}

#[inline(always)]
//#[deprecated]
pub unsafe fn luaL_checklong(L: *mut lua_State, n: c_int) -> c_long {
    luaL_checkinteger(L, n) as c_long
}

#[inline(always)]
//#[deprecated]
pub unsafe fn luaL_optlong(L: *mut lua_State, n: c_int, d: c_long) -> c_long {
    luaL_optinteger(L, n, d as lua_Integer) as c_long
}

#[inline(always)]
pub unsafe fn luaL_typename(L: *mut lua_State, i: c_int) -> *const c_char {
    lua_typename(L, lua_type(L, i))
}

#[inline(always)]
pub unsafe fn luaL_dofile(L: *mut lua_State, filename: *const c_char) -> c_int {
    let status = luaL_loadfile(L, filename);
    if status == 0 {
        lua_pcall(L, 0, LUA_MULTRET, 0)
    } else {
        status
    }
}

#[inline(always)]
pub unsafe fn luaL_dostring(L: *mut lua_State, s: *const c_char) -> c_int {
    let status = luaL_loadstring(L, s);
    if status == 0 {
        lua_pcall(L, 0, LUA_MULTRET, 0)
    } else {
        status
    }
}

#[inline(always)]
pub unsafe fn luaL_getmetatable(L: *mut lua_State, n: *const c_char) {
    lua_getfield(L, LUA_REGISTRYINDEX, n);
}

// luaL_opt would be implemented here but it is undocumented, so it's omitted

#[inline(always)]
pub unsafe fn luaL_loadbuffer(
    L: *mut lua_State,
    s: *const c_char,
    sz: size_t,
    n: *const c_char,
) -> c_int {
    luaL_loadbufferx(L, s, sz, n, ptr::null())
}

#[repr(C)]
pub struct luaL_Buffer {
    pub b: *mut c_char,
    pub size: size_t,
    pub n: size_t,
    pub L: *mut lua_State,
    pub initb: [c_char; LUAL_BUFFERSIZE as usize],
}

// TODO: Test this thoroughly
#[inline(always)]
pub unsafe fn luaL_addchar(B: *mut luaL_Buffer, c: c_char) {
    // (B)->n < (B) -> size || luaL_prepbuffsize((B), 1)
    if (*B).n < (*B).size {
        luaL_prepbuffsize(B, 1);
    }
    // (B)->b[(B)->n++] = (c)
    let offset = (*B).b.offset((*B).n as isize);
    ptr::write(offset, c);
    (*B).n += 1;
}

#[inline(always)]
pub unsafe fn luaL_addsize(B: *mut luaL_Buffer, s: size_t) {
    (*B).n += s;
}

extern "C" {
    pub fn luaL_buffinit(L: *mut lua_State, B: *mut luaL_Buffer);
    pub fn luaL_prepbuffsize(B: *mut luaL_Buffer, sz: size_t) -> *mut c_char;
    pub fn luaL_addlstring(B: *mut luaL_Buffer, s: *const c_char, l: size_t);
    pub fn luaL_addstring(B: *mut luaL_Buffer, s: *const c_char);
    pub fn luaL_addvalue(B: *mut luaL_Buffer);
    pub fn luaL_pushresult(B: *mut luaL_Buffer);
    pub fn luaL_pushresultsize(B: *mut luaL_Buffer, sz: size_t);
    pub fn luaL_buffinitsize(L: *mut lua_State, B: *mut luaL_Buffer, sz: size_t) -> *mut c_char;
}

pub unsafe fn luaL_prepbuffer(B: *mut luaL_Buffer) -> *mut c_char {
    luaL_prepbuffsize(B, LUAL_BUFFERSIZE as size_t)
}

// #[repr(C)]
// pub struct luaL_Stream {
//     pub f: *mut ::libc::FILE,
//     pub closef: lua_CFunction,
// }

// omitted: old module system compatibility
