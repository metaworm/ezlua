use crate::{
    convert::*,
    error::{Error, Result},
    ffi::*,
    luaapi::{ThreadStatus, Type},
    marker::RegVal,
    str::*,
    value::{ValRef, Value},
};

use alloc::{borrow::Cow, collections::BinaryHeap as Slots, format};
use core::{cell::RefCell, ffi::c_char, ffi::c_int, str};

/// Safe wrapper for operation to lua_State
#[derive(Debug)]
pub struct State {
    pub base: Index,
    pub(crate) state: *mut lua_State,
    pub(crate) free: RefCell<Slots<i32>>,
}

#[cfg(feature = "unsafe_send_sync")]
unsafe impl Send for State {}
#[cfg(feature = "unsafe_send_sync")]
unsafe impl Sync for State {}

impl State {
    /// Load lua script and execute it
    #[inline]
    pub fn do_string<S: AsRef<[u8]>>(&self, script: S, name: Option<&str>) -> Result<()> {
        self.load(script, name)?.pcall_void(())
    }

    #[inline(always)]
    pub fn registry_value<V: ToLua>(&self, val: V) -> Result<RegVal> {
        self.registry().reference(val).map(|r| RegVal {
            reference: r,
            inner: self.lua_inner(),
        })
    }

    #[inline(always)]
    pub fn safe_index(&self, i: Index) -> bool {
        i <= self.base
    }

    #[inline(always)]
    pub(crate) fn stack_guard(&self) -> StackGuard {
        StackGuard::from(self)
    }

    #[track_caller]
    pub(crate) fn drop_valref<'a>(&'a self, val: &ValRef<'a>) {
        if val.index > self.base {
            self.give_back_slot(val.index);
        }
    }

    #[inline(always)]
    #[track_caller]
    pub(crate) fn slot_exists(&self, i: Index) -> bool {
        self.free.borrow().iter().find(|&n| *n == i).is_some()
    }

    #[inline(always)]
    #[track_caller]
    pub(crate) fn give_back_slot(&self, i: Index) {
        #[cfg(feature = "std")]
        if debug_ezlua() {
            let loc = core::panic::Location::caller();
            assert!(
                !self.slot_exists(i),
                "[give back]: {i} from: {}:{}",
                loc.file(),
                loc.line()
            );
            std::println!("[give back]: {i} from: {}:{}", loc.file(), loc.line());
        }
        self.free.borrow_mut().push(i);
    }

    pub fn free_slots(&self) -> core::cell::Ref<Slots<i32>> {
        self.free.borrow()
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

pub(crate) const fn debug_ezlua() -> bool {
    option_env!("DEBUG_EZLUA").is_some()
}

pub mod unsafe_impl {
    #[cfg(feature = "std")]
    use std::path::Path;

    use alloc::string::String;

    use super::*;
    use crate::{
        luaapi::{GcOption, UnsafeLuaApi},
        value::{Function, LuaString, Table},
    };

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
            self.try_replace_top().unwrap_or_else(|| {
                let top = self.get_top();
                ValRef {
                    state: self,
                    index: top,
                }
            })
        }

        pub(crate) fn try_replace_top(&self) -> Option<ValRef> {
            let top = self.get_top();
            while let Some(slot) = self.free.borrow_mut().pop() {
                if slot < top {
                    #[cfg(feature = "std")]
                    if debug_ezlua() {
                        std::println!("[borrow slot] {slot} top: {top}");
                    }
                    self.replace(slot);
                    return Some(ValRef {
                        state: self,
                        index: slot,
                    });
                } else {
                    #[cfg(feature = "std")]
                    if debug_ezlua() {
                        std::println!("[drop slot] {slot}");
                    }
                }
            }
            None
        }

        pub(crate) fn val(&self, i: Index) -> ValRef {
            debug_assert!(i > 0);
            if i <= self.base {
                self.val_without_push(i)
            } else {
                self.check_stack(1).expect("stack");
                self.push_value(i);
                self.top_val()
            }
        }

        #[inline(always)]
        pub fn arg_val(&self, i: Index) -> Option<ValRef> {
            self.safe_index(i).then(|| ValRef {
                state: self,
                index: self.abs_index(i),
            })
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

        /// Get the C registry table
        #[inline(always)]
        pub fn registry(&self) -> Table {
            Table(ValRef {
                state: self,
                index: LUA_REGISTRYINDEX,
            })
        }

        /// Create a new lua value
        #[inline(always)]
        pub fn new_val<V: ToLua>(&self, val: V) -> Result<ValRef> {
            self.check_stack(1)?;
            self.push(val)?;
            Ok(self.top_val())
        }

        /// Create a new lua value, return as [`Value`] rather than [`ValRef`]
        #[inline(always)]
        pub fn new_value<V: ToLua>(&self, val: V) -> Result<Value> {
            self.new_val(val).map(ValRef::into_value)
        }

        /// Create a lua table and specify the size
        #[inline(always)]
        pub fn new_table_with_size(&self, narr: c_int, nrec: c_int) -> Result<Table> {
            self.check_stack(1)?;
            self.create_table(narr, nrec);
            Ok(self.top_val().try_into().expect("table"))
        }

        /// Create a lua table
        #[inline(always)]
        pub fn new_table(&self) -> Result<Table> {
            self.new_table_with_size(0, 0)
        }

        /// Create a lua string
        #[inline]
        pub fn new_string<S: AsRef<[u8]>>(&self, s: S) -> Result<LuaString> {
            self.check_stack(1)?;
            self.push_bytes(s.as_ref());
            Ok(self.top_val().try_into().expect("string"))
        }

        /// Load script string or bytecode
        pub fn load<S: AsRef<[u8]>>(&self, s: S, name: Option<&str>) -> Result<Function> {
            self.check_stack(1)?;
            let guard = self.stack_guard();
            self.statuscode_to_error(self.load_buffer(s, name))?;
            core::mem::forget(guard);
            Ok(self.top_val().try_into().expect("function"))
        }

        /// Create function from script file
        #[cfg(feature = "std")]
        #[inline]
        pub fn load_file<P: AsRef<Path>>(&self, path: P) -> Result<Function> {
            let path = path.as_ref();
            self.load(
                std::fs::read(path).map_err(Error::from_debug)?,
                Some(format!("@{}", path.to_string_lossy()).as_str()),
            )
        }

        /// Register your own lua module, which can be load by `require` function in lua
        #[inline(always)]
        pub fn register_module<'a, F: Fn(&'a State) -> Result<Table<'a>>>(
            &self,
            name: &str,
            init: F,
            global: bool,
        ) -> Result<()> {
            assert!(core::mem::size_of::<F>() == 0);
            self.requiref(
                &CString::new(name).map_err(Error::runtime_debug)?,
                crate::convert::function_wrapper(init),
                global,
            );
            Ok(())
        }

        /// Get the lua global table
        #[inline(always)]
        pub fn global(&self) -> Table {
            self.raw_geti(LUA_REGISTRYINDEX, LUA_RIDX_GLOBALS);
            self.top_val().try_into().expect("table")
        }

        /// Do a full GC for lua
        pub fn gc_collect(&self) -> Result<()> {
            self.gc(GcOption::Collect, 0);

            Ok(())
        }

        /// Stack backtrace info
        pub fn backtrace(&self, co: Option<&State>, msg: &str, level: i32) -> String {
            self.traceback(
                co.map(|s| s.as_ptr()).unwrap_or(core::ptr::null_mut()),
                CString::new(msg).unwrap().as_c_str(),
                level,
            );
            let result = self.to_string_lossy(-1).unwrap_or_default().into_owned();
            self.pop(1);
            result
        }

        /// [-0, +1, -]
        pub(crate) fn get_or_init_metatable(&self, callback: MetatableKey) -> Result<()> {
            let top = self.get_top();
            let reg = self.registry();
            let p = callback as *const usize;
            let metatable = self.raw_getp(LUA_REGISTRYINDEX, p);
            if metatable.is_none_or_nil() {
                let mt = self.new_table_with_size(0, 4)?;
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
                mt.0.ensure_top();
                self.replace(-2);
            }
            debug_assert_eq!(self.get_top(), top + 1);

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

        #[inline(always)]
        pub unsafe fn test_userdata_meta<T>(&self, i: Index, meta: MetatableKey) -> Option<&mut T> {
            let _guard = self.stack_guard();

            let p = if self.get_metatable(i) && {
                self.raw_getp(LUA_REGISTRYINDEX, meta as *const ());
                self.raw_equal(-1, -2)
            } {
                self.to_userdata(i) as _
            } else {
                core::ptr::null_mut()
            };
            (p as *mut T).as_mut()
        }

        /// [-1, +1, -]
        pub fn trace_error(&self, s: Option<&Self>) -> Cow<'_, str> {
            let err = self.to_str(-1).unwrap_or("");
            self.pop(1);
            unsafe {
                let thread = s.unwrap_or(self);
                luaL_traceback(self.state, thread.state, err.as_ptr() as *const c_char, 0);
            }
            self.to_string_lossy(-1).unwrap_or_default()
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

        pub fn stack(&self, n: i32) -> Option<lua_Debug> {
            self.get_stack(n)
        }

        #[inline(always)]
        pub(crate) fn raise_with<T, F: FnOnce(&State) -> Result<T>>(self, fun: F) -> T {
            match fun(&self) {
                Ok(result) => result,
                Err(err) => unsafe {
                    self.raise_error(err);
                },
            }
        }

        #[inline(always)]
        pub(crate) unsafe fn return_result<T: ToLuaMulti>(self, t: T) -> usize {
            match t.push_multi(&self) {
                Ok(result) => result,
                Err(err) => self.raise_error(err),
            }
        }

        #[inline(always)]
        pub(crate) fn protect_call<'a, T: ToLuaMulti, R: FromLuaMulti<'a>>(
            &'a self,
            args: T,
            callback: CFunction,
        ) -> Result<R> {
            self.pcall_trace(callback, args)
        }

        // tracebacked pcall
        #[inline(always)]
        pub(crate) fn pcall_trace<'a, F: ToLua, T: ToLuaMulti, R: FromLuaMulti<'a>>(
            &'a self,
            func: F,
            args: T,
        ) -> Result<R> {
            let guard = self.stack_guard();

            self.check_stack(args.value_count().unwrap_or(10) as i32 + 2)?;
            self.push_fn(Some(Self::traceback_c));
            self.push(func)?;
            self.statuscode_to_error(unsafe {
                lua_pcall(self.state, self.push_multi(args)? as _, -1, guard.top() + 1)
            })?;

            let result_base = guard.top() + 2;
            self.to_multi_balance(guard, result_base)
        }

        #[inline(always)]
        pub(crate) fn to_multi_balance<'a, R: FromLuaMulti<'a>>(
            &'a self,
            guard: StackGuard<'a>,
            result_base: i32,
        ) -> Result<R> {
            let top = self.get_top();
            let res = R::from_lua_multi(self, result_base);
            self.check_multi_balance(guard, top);
            res
        }

        fn check_multi_balance<'a>(&'a self, guard: StackGuard<'a>, top: i32) {
            if self.get_top() > top {
                // reuse slots between old_top and top
                for i in guard.top() + 1..=top {
                    self.give_back_slot(i);
                }
                core::mem::forget(guard);
            } else {
                // there are no new valref
                drop(guard);
            }
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
                    let err = self.to_string_lossy(-1).unwrap_or_default().into_owned();
                    match ts {
                        ThreadStatus::RuntimeError | ThreadStatus::MessageHandlerError => {
                            Err(Error::runtime(err))
                        }
                        // ThreadStatus::GcError => Err(Error::Gc(err)),
                        ThreadStatus::SyntaxError => Err(Error::Syntax(err)),
                        ThreadStatus::MemoryError => Err(Error::Memory(err)),
                        ThreadStatus::FileError => Err(Error::runtime(err)),
                        _ => unreachable!(),
                    }
                }
            }
        }

        pub(crate) fn statuscode_to_error_and_pop(&self, ts: i32) -> Result<()> {
            let result = self.statuscode_to_error(ts);
            if result.is_err() {
                self.pop(1)
            };
            result
        }

        pub(crate) fn statuscode_to_error(&self, ts: i32) -> Result<()> {
            match ts {
                LUA_OK => Ok(()),
                LUA_YIELD => Err(Error::Yield),
                _ => {
                    let err = self.to_string_lossy(-1).unwrap_or_default().into_owned();
                    match ts {
                        LUA_ERRRUN | LUA_ERRERR => Err(Error::runtime(err)),
                        // LUA_ERRGCMM => Err(Error::Gc(err)),
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
                Value::String(r) => self.pushval(r.0),
                Value::Table(r) => self.pushval(r.0),
                Value::Function(r) => self.pushval(r.0),
                Value::UserData(r) => self.pushval(r.0),
                Value::Thread(r) => self.pushval(r.0),
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
        pub(crate) fn clear_with_keep_top_one(&self, base: Index) -> bool {
            let top = self.get_top();
            if top == base + 1 {
                return true;
            }
            if top > base + 1 {
                self.drop_slots_greater(base);
                self.replace(base + 1);
                self.set_top(base + 1);
                return true;
            }

            false
        }

        #[track_caller]
        pub(crate) fn dump_stack(&self) -> String {
            let loc = core::panic::Location::caller();
            let mut info = format!("dump_stack from {}:{}\n", loc.file(), loc.line());
            for i in (1..=self.get_top()).rev().take(6) {
                let val = self.val_without_push(i);
                info += format!("  [{i}] {val:?}\n").as_str();
                core::mem::forget(val);
            }
            info
        }

        /// drop the slot > i
        pub(crate) fn drop_slots_greater(&self, i: Index) {
            let mut free = self.free.borrow_mut();
            while free.peek().filter(|&&s| s > i).is_some() {
                free.pop();
            }
        }
    }
}
