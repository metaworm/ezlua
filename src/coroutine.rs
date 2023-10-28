use core::ptr::null_mut;

use crate::{
    convert::*,
    error::{Result, ToLuaResult},
    ffi::*,
    lua::ArcLuaInner,
    luaapi::{ThreadStatus, Type, UnsafeLuaApi, NOREF},
    prelude::Reference,
    state::State,
    value::{Function, ValRef},
};

pub struct Coroutine {
    inner: ArcLuaInner,
    pub(crate) state: State,
    nres: i32,
}

unsafe impl Send for Coroutine {}

impl core::ops::Deref for Coroutine {
    type Target = State;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl Drop for Coroutine {
    fn drop(&mut self) {
        self.push_nil();
        self.raw_setp(LUA_REGISTRYINDEX, self.as_ptr());
    }
}

impl Coroutine {
    // [-0, +0]
    pub fn empty(s: &State) -> Self {
        let result = s.new_thread();
        let result = unsafe { Self::init(result) };
        s.pop(1);
        result
    }

    unsafe fn init(l: *mut lua_State) -> Self {
        let state = State::from_raw_state(l);
        // assert_eq!(state.type_of(1), Type::Function);

        state.push_thread();
        state.raw_setp(LUA_REGISTRYINDEX, l);

        let inner = state.lua_inner();
        Self {
            inner,
            state,
            nres: 0,
        }
    }

    pub fn new(fun: ValRef) -> Result<Self> {
        Ok(match fun.type_of() {
            Type::Function => unsafe {
                let s = fun.state;
                let l = s.new_thread();
                fun.ensure_top();
                s.xmove(l, 1);
                Self::init(l)
            },
            Type::Thread => unsafe {
                let l = fun
                    .state
                    .to_thread(fun.index)
                    .ok_or("to_thread")
                    .convert_error()?;
                Self::init(l)
            },
            _ => return Err("invalid coroutine type").convert_error(),
        })
    }

    /// Get the function associated to this coroutine
    #[inline(always)]
    pub fn func(&self) -> Result<Function> {
        self.val(1).try_into()
    }

    #[doc(hidden)]
    pub fn resume<'a, A: ToLuaMulti, R: FromLuaMulti<'a>>(&'a mut self, args: A) -> Result<R> {
        self.pop(self.nres);
        self.state
            .check_stack(args.value_count().unwrap_or(10) as i32)?;
        match self
            .state
            .resume(null_mut(), self.push_multi(args)? as _, &mut self.nres)
        {
            ThreadStatus::Ok | ThreadStatus::Yield => {
                let fidx = self.get_top() - self.nres;
                self.set_top(fidx + R::COUNT as i32);
                self.nres = R::COUNT as i32;
                R::from_lua_multi(self, self.abs_index(-(R::COUNT as i32)))
            }
            err => Err(self.status_to_error(err).unwrap_err()),
        }
    }
}

impl FromLua<'_> for Coroutine {
    fn from_lua(s: &State, val: ValRef) -> Result<Self> {
        Self::new(val)
    }
}

pub struct CoroutineWithRef(pub Coroutine, pub Reference);

impl CoroutineWithRef {
    pub fn take<'l>(&mut self, lua: &'l State) -> Result<ValRef<'l>> {
        let result = lua.registry().take_reference(self.1)?;
        self.1 = NOREF;
        Ok(result)
    }
}

impl Drop for CoroutineWithRef {
    fn drop(&mut self) {
        if !self.1.is_no_ref() {
            self.0.registry().unreference(self.1);
        }
    }
}
