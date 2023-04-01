use core::ptr::null_mut;

use crate::{
    convert::*,
    error::{Result, ToLuaResult},
    ffi::*,
    lua::ArcLuaInner,
    luaapi::{ThreadStatus, Type, UnsafeLuaApi},
    state::State,
    value::ValRef,
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

    pub fn resume<'a, A: ToLuaMulti, R: FromLuaMulti<'a>>(&'a mut self, args: A) -> Result<R> {
        self.pop(self.nres);
        match self
            .state
            .resume(null_mut(), self.push_multi(args)? as _, &mut self.nres)
        {
            ThreadStatus::Ok | ThreadStatus::Yield => {
                let fidx = self.get_top() - self.nres;
                self.set_top(fidx + R::COUNT as i32);
                self.nres = R::COUNT as i32;
                R::from_lua(self, self.abs_index(-(R::COUNT as i32)))
            }
            err => Err(self.status_to_error(err).unwrap_err()),
        }
    }
}

impl FromLua<'_> for Coroutine {
    fn from_lua(s: &State, val: ValRef) -> Option<Self> {
        Self::new(val).ok()
    }
}
