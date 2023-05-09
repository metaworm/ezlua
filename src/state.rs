use crate::{
    convert::*,
    error::{Error, Result},
    ffi::*,
    luaapi::{ThreadStatus, Type},
    marker::RegVal,
    str::*,
    value::{ValRef, Value},
};

use alloc::borrow::Cow;
#[cfg(feature = "use_bset")]
use alloc::collections::BTreeSet as Slots;
#[cfg(not(feature = "use_bset"))]
use alloc::collections::BinaryHeap as Slots;
use alloc::format;
use core::cell::{Cell, RefCell};
use core::ffi::{c_char, c_int};
use core::{mem, str};

/// Safe wrapper for operation to lua_State
#[derive(Debug)]
pub struct State {
    pub base: Index,
    /// maximum index of valref which in use
    pub(crate) max_vi: Cell<i32>,
    pub(crate) state: *mut lua_State,
    pub(crate) free: RefCell<Slots<i32>>,
}

#[cfg(feature = "unsafe_send_sync")]
unsafe impl Send for State {}
#[cfg(feature = "unsafe_send_sync")]
unsafe impl Sync for State {}

impl State {
    #[inline]
    pub fn do_string<S: AsRef<[u8]>>(&self, script: S, name: Option<&str>) -> Result<()> {
        self.new_function(script, name)?.pcall_void(())
    }

    #[inline(always)]
    pub fn registry_value<V: ToLua>(&self, val: V) -> Result<RegVal> {
        self.registry().reference(val).map(|r| RegVal {
            reference: r,
            inner: self.lua_inner(),
        })
    }

    #[inline(always)]
    pub(crate) fn stack_guard(&self) -> StackGuard {
        StackGuard::from(self)
    }
}

#[derive(Debug)]
pub(crate) struct StackGuard<'a> {
    state: &'a State,
    top: Index,
}

impl<'a> StackGuard<'a> {
    #[inline(always)]
    pub fn top(&self) -> i32 {
        self.top
    }
}

impl<'a> From<&'a State> for StackGuard<'a> {
    fn from(state: &'a State) -> Self {
        let top = state.stack_top();
        Self { state, top }
    }
}

pub mod unsafe_impl {
    #[cfg(feature = "std")]
    use std::path::Path;

    use alloc::string::String;

    use super::*;
    use crate::luaapi::UnsafeLuaApi;

    impl<'a> Drop for StackGuard<'a> {
        fn drop(&mut self) {
            self.state.set_top(self.top);
        }
    }

    impl State {
        pub unsafe fn from_raw_state(state: *mut lua_State) -> Self {
            let base = lua_gettop(state);
            Self {
                base,
                state,
                free: Default::default(),
                max_vi: Default::default(),
            }
        }

        #[inline(always)]
        pub fn raw_state(&self) -> *mut lua_State {
            self.state
        }

        #[inline(always)]
        pub fn stack_top(&self) -> i32 {
            self.get_top()
        }

        #[inline]
        pub fn check_stack(&self, n: i32) -> Result<()> {
            if UnsafeLuaApi::check_stack(self, n) {
                Ok(())
            } else {
                Err(Error::runtime(format!("check stack {n}")))
            }
        }

        #[inline]
        pub fn check_type(&self, i: Index, ty: Type) -> Result<()> {
            let t = self.type_of(i);
            if t != ty {
                return Err(Error::TypeNotMatch(t));
            }
            Ok(())
        }

        #[inline(always)]
        pub fn up_value(&self, i: Index) -> ValRef {
            ValRef {
                state: self,
                index: lua_upvalueindex(i),
            }
        }

        pub(crate) fn top_val(&self) -> ValRef {
            let top = self.get_top();
            self.try_replace_top(top).unwrap_or_else(|| {
                self.set_max_valref_index(top);
                ValRef {
                    state: self,
                    index: top,
                }
            })
        }

        pub(crate) fn try_replace_top(&self, top: Index) -> Option<ValRef> {
            let top = self.get_top();
            while let Some(slot) = self.free.borrow_mut().pop() {
                if slot < top {
                    self.replace(slot);
                    return Some(ValRef {
                        state: self,
                        index: slot,
                    });
                }
            }
            None
        }

        pub(crate) fn val(&self, i: Index) -> ValRef {
            if i <= self.base {
                self.val_without_push(i)
            } else {
                self.push_value(i);
                self.top_val()
            }
        }

        pub fn to_safe_bytes(&self, i: Index) -> Option<&[u8]> {
            self.safe_index(i).then(|| self.to_bytes(i)).flatten()
        }

        #[inline(always)]
        pub(crate) fn val_without_push(&self, i: Index) -> ValRef {
            ValRef {
                state: self,
                index: self.abs_index(i),
            }
        }

        #[inline(always)]
        pub fn registry(&self) -> ValRef {
            ValRef {
                state: self,
                index: LUA_REGISTRYINDEX,
            }
        }

        #[inline(always)]
        pub fn new_val<V: ToLua>(&self, val: V) -> Result<ValRef> {
            self.check_stack(1)?;
            self.push(val)?;
            Ok(self.top_val())
        }

        #[inline(always)]
        pub fn new_value<V: ToLua>(&self, val: V) -> Result<Value> {
            self.new_val(val).map(ValRef::into_value)
        }

        #[inline(always)]
        pub fn create_table(&self, narr: c_int, nrec: c_int) -> Result<ValRef> {
            UnsafeLuaApi::create_table(self, narr, nrec);
            Ok(self.top_val())
        }

        /// Create a table
        #[inline(always)]
        pub fn new_table(&self) -> Result<ValRef> {
            self.create_table(0, 0)
        }

        /// Create a lua string
        #[inline]
        pub fn new_string<S: AsRef<[u8]>>(&self, s: S) -> Result<ValRef> {
            self.push_bytes(s.as_ref());
            Ok(self.top_val())
        }

        /// Create function from script string or bytecode
        #[inline(always)]
        pub fn new_function<S: AsRef<[u8]>>(&self, s: S, name: Option<&str>) -> Result<ValRef> {
            self.load(s, name)
        }

        /// Load script string or bytecode
        pub fn load<S: AsRef<[u8]>>(&self, s: S, name: Option<&str>) -> Result<ValRef> {
            let guard = self.stack_guard();
            self.statuscode_to_error(self.load_buffer(s, name))?;
            core::mem::forget(guard);
            Ok(self.top_val())
        }

        #[cfg(feature = "std")]
        /// Create function from script file
        #[inline]
        pub fn load_file<P: AsRef<Path>>(&self, path: P) -> Result<ValRef> {
            let path = path.as_ref();
            self.new_function(
                std::fs::read(path).map_err(Error::from_debug)?,
                Some(format!("@{}", path.to_string_lossy()).as_str()),
            )
        }

        #[inline(always)]
        pub fn register_module<'a, F: Fn(&'a State) -> Result<ValRef<'a>>>(
            &self,
            name: &str,
            init: F,
            global: bool,
        ) -> Result<()> {
            assert!(core::mem::size_of::<F>() == 0);
            self.requiref(
                &CString::new(name).map_err(Error::runtime_debug)?,
                crate::convert::closure_wrapper::<_, F>,
                global,
            );
            Ok(())
        }

        pub(crate) fn drop_valref<'a>(&'a self, val: &ValRef<'a>) {
            if val.index > self.base {
                self.free.borrow_mut().push(val.index);
            }
        }

        #[inline(always)]
        pub fn safe_index(&self, i: Index) -> bool {
            i <= self.base
        }

        #[inline(always)]
        pub fn arg_val(&self, i: Index) -> Option<ValRef> {
            self.safe_index(i).then_some(ValRef {
                state: self,
                index: self.abs_index(i),
            })
        }

        #[inline(always)]
        pub fn global(&self) -> ValRef {
            self.raw_geti(LUA_REGISTRYINDEX, LUA_RIDX_GLOBALS);
            self.top_val()
        }

        /// [-0, +1, -]
        pub(crate) fn get_or_init_metatable(&self, callback: MetatableKey) -> Result<()> {
            let top = self.get_top();
            let reg = self.registry();
            let p = callback as *const usize;
            let metatable = self.raw_getp(LUA_REGISTRYINDEX, p);
            if metatable.is_none_or_nil() {
                let mt = self.create_table(0, 4)?;
                self.balance_with(|_| callback(&mt))?;
                debug_assert_eq!(self.type_of(mt.index), Type::Table);

                if self.get_field(mt.index, crate::cstr!("__name")) == Type::String {
                    self.push_value(mt.index);
                    self.set_table(LUA_REGISTRYINDEX);
                } else {
                    self.pop(1);
                }

                self.push_value(mt.index);
                self.raw_setp(LUA_REGISTRYINDEX, p);
                mt.ensure_top();
                self.replace(-2);
            }
            assert_eq!(self.get_top(), top + 1);

            Ok(())
        }

        /// [-0, +0, -]
        #[inline]
        pub(crate) fn set_or_init_metatable(&self, callback: MetatableKey) -> Result<()> {
            let ty = self.type_of(-1);
            assert!(ty == Type::Userdata || ty == Type::Table);
            self.get_or_init_metatable(callback)?;
            self.set_metatable(-2);
            Ok(())
        }

        // ///////////////////////// Wrapper functions //////////////////////////////

        pub fn check_udata<T>(&self, i: Index, name: &CStr) -> &mut T {
            unsafe { mem::transmute(luaL_checkudata(self.state, i, name.as_ptr())) }
        }

        pub fn test_userdata_meta_(&self, i: Index, meta: MetatableKey) -> *mut () {
            if self.get_metatable(i) && {
                self.raw_getp(LUA_REGISTRYINDEX, meta as *const ());
                self.raw_equal(-1, -2)
            } {
                self.pop(2);
                self.to_userdata(i) as _
            } else {
                core::ptr::null_mut()
            }
        }

        #[inline(always)]
        pub unsafe fn test_userdata_meta<T>(&self, i: Index, meta: MetatableKey) -> Option<&mut T> {
            (self.test_userdata_meta_(i, meta) as *mut T).as_mut()
        }

        pub unsafe fn check_userdata<T>(&self, i: Index, meta: MetatableKey) -> &mut T {
            let p = self.test_userdata_meta::<T>(i, meta);
            match p {
                Some(p) => p,
                None => {
                    let tname = CString::new(core::any::type_name::<T>()).unwrap_or_default();
                    self.type_error(i, &tname);
                }
            }
        }

        /// [-1, +1, -]
        pub fn trace_error(&self, s: Option<&Self>) -> Cow<'_, str> {
            let err = self.to_str(-1).unwrap_or("");
            self.pop(1);
            unsafe {
                let thread = s.unwrap_or(self);
                luaL_traceback(self.state, thread.state, err.as_ptr() as *const c_char, 0);
            }
            self.to_str_lossy(-1).unwrap_or_default()
        }

        #[inline(always)]
        pub(crate) fn balance_with<'a, T: 'a, F: FnOnce(&'a State) -> T>(
            &'a self,
            callback: F,
        ) -> T {
            let top = self.get_top();
            let result = callback(self);
            self.set_top(top);
            self.drop_slots_greater(top);
            result
        }

        pub fn backtrace(&self, co: Option<&State>, msg: &str, level: i32) -> String {
            self.traceback(
                co.map(|s| s.as_ptr()).unwrap_or(core::ptr::null_mut()),
                CString::new(msg).unwrap().as_c_str(),
                level,
            );
            let result = self.to_str_lossy(-1).unwrap_or_default().into_owned();
            self.pop(1);
            result
        }

        pub fn stack(&self, n: i32) -> Option<lua_Debug> {
            self.get_stack(n)
        }

        #[inline(always)]
        pub unsafe fn error_string(self, e: impl AsRef<str>) -> ! {
            self.push_string(e.as_ref());
            core::mem::drop(e);
            self.error()
        }

        #[inline(always)]
        pub unsafe fn raise_error(self, e: impl core::fmt::Debug) -> ! {
            self.error_string(format!("{e:?}"))
        }

        pub unsafe extern "C" fn traceback_c(l: *mut lua_State) -> i32 {
            luaL_traceback(l, l, lua_tostring(l, 1), 1);
            1
        }

        pub(crate) fn status_to_error(&self, ts: ThreadStatus) -> Result<()> {
            match ts {
                ThreadStatus::Ok => Ok(()),
                ThreadStatus::Yield => Err(Error::Yield),
                _ => {
                    let err = self.to_str_lossy(-1).unwrap_or_default().into_owned();
                    match ts {
                        ThreadStatus::RuntimeError | ThreadStatus::MessageHandlerError => {
                            Err(Error::runtime(err))
                        }
                        ThreadStatus::GcError => Err(Error::Gc(err)),
                        ThreadStatus::SyntaxError => Err(Error::Syntax(err)),
                        ThreadStatus::MemoryError => Err(Error::Memory(err)),
                        ThreadStatus::FileError => Err(Error::runtime(err)),
                        _ => unreachable!(),
                    }
                }
            }
        }

        pub(crate) fn statuscode_to_error(&self, ts: i32) -> Result<()> {
            match ts {
                LUA_OK => Ok(()),
                LUA_YIELD => Err(Error::Yield),
                _ => {
                    let err = self.to_str_lossy(-1).unwrap_or_default().into_owned();
                    match ts {
                        LUA_ERRRUN | LUA_ERRERR => Err(Error::runtime(err)),
                        LUA_ERRGCMM => Err(Error::Gc(err)),
                        LUA_ERRSYNTAX => Err(Error::Syntax(err)),
                        LUA_ERRMEM => Err(Error::Memory(err)),
                        LUA_ERRFILE => Err(Error::runtime(err)),
                        _ => unreachable!(),
                    }
                }
            }
        }

        /// Pushes the given value onto the stack.
        pub(crate) fn pushv(&self, value: Value) {
            match value {
                Value::None | Value::Nil => self.push_nil(),
                Value::Bool(b) => self.push_bool(b),
                Value::Integer(i) => self.push_integer(i),
                Value::Number(n) => self.push_number(n),
                Value::LightUserdata(ud) => self.push_light_userdata(ud),
                Value::String(r)
                | Value::Table(r)
                | Value::Function(r)
                | Value::Userdata(r)
                | Value::Thread(r) => self.pushval(r),
            }
        }

        pub(crate) fn pushval(&self, val: ValRef) {
            self.pushvalref(&val)
        }

        pub(crate) fn pushvalref(&self, val: &ValRef) {
            let state = val.state.raw_state();
            val.state.push_value(val.index);
            if state != self.raw_state() {
                unsafe { crate::ffi::lua_xmove(state, self.raw_state(), 1) }
            }
        }

        /// clear the stack, but only retain the top value
        pub(crate) fn clear_with_keep_top_one(&self, base: Index) {
            let top = self.get_top();
            if top == base + 1 {
                return;
            }
            if top > (base + 1) {
                self.drop_slots_greater(base);
                self.replace(base + 1);
                self.set_top(base + 1);
            } else {
                panic!("stack should be increased but decreased")
            }
        }

        pub(crate) fn reset_max_valref_index(&self) {
            self.max_vi.set(0);
        }

        pub(crate) fn set_max_valref_index(&self, i: Index) {
            self.max_vi.set(self.max_vi.get().max(i));
        }

        /// drop the slot > i
        pub(crate) fn drop_slots_greater(&self, i: Index) {
            #[cfg(feature = "use_bset")]
            self.free.borrow_mut().split_off(&(i + 1));
            #[cfg(not(feature = "use_bset"))]
            {
                let mut free = self.free.borrow_mut();
                while free.peek().filter(|&&s| s > i).is_some() {
                    free.pop();
                }
            }
        }
    }
}
