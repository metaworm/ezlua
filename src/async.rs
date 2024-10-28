use crate::{
    convert::*,
    error::{Error, Result},
    ffi::{self, lua_State, lua_resetthread},
    luaapi::*,
    marker::OwnedUserdata,
    state::{StackGuard, State},
    userdata::UserData,
    value::*,
};

use alloc::boxed::Box;
use core::{ffi::c_int, future::Future};

pub type TaskOutput<'a> = Box<dyn Future<Output = Result<usize>> + Send + 'a>;

struct TaskWrapper<'a> {
    verify: usize,
    task: Option<Box<dyn FnOnce(&'a State, i32) -> TaskOutput<'a> + Send + 'a>>,
    error: Option<Error>,
}

impl<'a> Function<'a> {
    #[inline(always)]
    pub async fn call_async_void<T: ToLuaMulti>(&self, args: T) -> Result<()> {
        self.call_async(args).await
    }

    // TODO: doc: once failed, refs to this state is invalid, and should to be dropped
    #[inline(always)]
    pub async fn call_async<T: ToLuaMulti, R: FromLuaMulti<'a>>(&self, args: T) -> Result<R> {
        self.call_async_from(args, None).await
    }

    #[inline(always)]
    pub async fn call_async_from<T: ToLuaMulti, R: FromLuaMulti<'a>>(
        &self,
        args: T,
        state: Option<&State>,
    ) -> Result<R> {
        let guard = self.state.stack_guard();

        self.state
            .check_stack(args.value_count().unwrap_or(10) as i32 + 2)?;
        self.state.push_value(self.index);
        let count = R::COUNT as i32;
        let guard = self
            .state
            .raw_call_async(state, guard, self.state.push_multi(args)? as _, count)
            .await?;
        let result_base = guard.top() + 1;
        self.state.to_multi_balance(guard, result_base)
    }
}

impl Table<'_> {
    #[inline(always)]
    pub fn set_async_closure<
        'l,
        K: ToLua,
        A: 'l,
        R: ToLuaMulti + 'l,
        F: LuaAsyncMethod<'l, (), A, R> + 'static,
    >(
        &'l self,
        key: K,
        fun: F,
    ) -> Result<&Self> {
        self.set(key, self.state.async_closure(fun)?)?;
        Ok(self)
    }

    #[inline(always)]
    pub fn set_async_function<
        'l,
        K: ToLua,
        A: FromLuaMulti<'l> + 'l,
        R: ToLuaMulti + 'l,
        FUT: Future<Output = R> + Send + 'l,
        F: Fn(&'l State, A) -> FUT + Send + 'static,
    >(
        &'l self,
        key: K,
        fun: F,
    ) -> Result<&Self> {
        self.set(key, self.state.async_function(fun)?)?;
        Ok(self)
    }
}

unsafe extern "C-unwind" fn continue_func(
    l: *mut lua_State,
    status: c_int,
    ctx: ffi::lua_KContext,
) -> c_int {
    let mut wrap = Box::from_raw(ctx as *mut TaskWrapper);
    if let Some(err) = wrap.error.take() {
        drop(wrap);
        State::from_raw_state(l).raise_error(err);
    }
    ffi::lua_gettop(l)
}

impl State {
    #[inline(always)]
    pub(crate) fn yield_task<'a, F: FnOnce(&'a State, i32) -> TaskOutput<'a> + Send + 'a>(
        self,
        callback: F,
    ) -> ! {
        let pudata = Box::into_raw(
            TaskWrapper {
                verify: continue_func as usize,
                task: Some(Box::new(callback)),
                error: None,
            }
            .into(),
        );
        self.push_light_userdata(pudata);

        unsafe {
            let l = self.as_ptr();
            let top = self.get_top();
            drop(self);
            ffi::lua_yieldk(l, top, pudata as _, Some(continue_func));
        }
        unreachable!("co_yieldk called in non-coroutine context; check is_yieldable first")
    }

    // /// Maps to `lua_pcallk`.
    // pub(crate) fn pcallk<F>(
    //     &self,
    //     nargs: c_int,
    //     nresults: c_int,
    //     msgh: c_int,
    //     continuation: F,
    // ) -> c_int
    // where
    //     F: FnOnce(&State, ThreadStatus) -> c_int,
    // {
    //     let func = continue_func::<F>;
    //     let ctx = Box::into_raw(continuation.into()) as _;
    //     unsafe {
    //         // lua_pcallk only returns if no yield occurs, so call the continuation
    //         func(
    //             self.as_ptr(),
    //             ffi::lua_pcallk(self.as_ptr(), nargs, nresults, msgh, ctx, Some(func)),
    //             ctx,
    //         )
    //     }
    // }

    // /// Maps to `lua_yield`.
    // pub(crate) fn r#yield(&self, nresults: c_int) -> ! {
    //     unsafe { ffi::lua_yield(self.as_ptr(), nresults) };
    //     panic!("co_yieldk called in non-coroutine context; check is_yieldable first")
    // }

    // /// Maps to `lua_yieldk`.
    // pub(crate) fn yieldk<F>(&self, nresults: c_int, continuation: F) -> !
    // where
    //     F: FnOnce(&State, ThreadStatus) -> c_int,
    // {
    //     unsafe extern "C-unwind" fn continue_func<F>(
    //         l: *mut lua_State,
    //         status: c_int,
    //         ctx: ffi::lua_KContext,
    //     ) -> c_int
    //     where
    //         F: FnOnce(&State, ThreadStatus) -> c_int,
    //     {
    //         core::mem::transmute::<_, Box<F>>(ctx)(
    //             &State::from_raw_state(l),
    //             ThreadStatus::from_c_int(status),
    //         )
    //     }

    //     let ctx = Box::into_raw(continuation.into()) as _;
    //     unsafe { ffi::lua_yieldk(self.as_ptr(), nresults, ctx, Some(continue_func::<F>)) };
    //     panic!("co_yieldk called in non-coroutine context; check is_yieldable first")
    // }

    /// Bind a rust async function(closure)
    #[inline(always)]
    pub fn async_function<
        'l,
        A: FromLuaMulti<'l> + 'l,
        R: ToLuaMulti + 'l,
        FUT: Future<Output = R> + Send + 'l,
        F: Fn(&'l State, A) -> FUT + Send + 'static,
    >(
        &self,
        fun: F,
    ) -> Result<Function> {
        self.bind_async_closure(move |lua, base| {
            A::from_lua_multi(lua, base).map(|args| fun(lua, args))
        })
    }

    /// Bind a rust async function(closure) with flexible argument types
    #[inline(always)]
    pub fn async_closure<
        'l,
        A: 'l,
        R: ToLuaMulti + 'l,
        F: LuaAsyncMethod<'l, (), A, R> + 'static,
    >(
        &self,
        fun: F,
    ) -> Result<Function> {
        self.bind_async_closure(move |lua, base| fun.call_method(lua, base))
    }

    #[inline(always)]
    pub fn bind_async<'l, R: ToLuaMulti + 'l, F: Future<Output = R> + Send + 'static>(
        &self,
        fun: F,
    ) -> Result<LuaUserData> {
        struct AsyncWrapper<F>(F);

        impl<R: ToLuaMulti, F: Future<Output = R> + Send + 'static> UserData for AsyncWrapper<F> {
            const TYPE_NAME: &'static str = "ezlua::AsyncWrapper";

            fn metatable(mt: crate::userdata::UserdataRegistry<Self>) -> Result<()> {
                mt.set_async_closure("__call", |OwnedUserdata::<Self>(this)| this.0)?;
                Ok(())
            }
        }

        self.new_userdata(AsyncWrapper(fun))
    }

    #[doc(hidden)]
    #[inline(always)]
    pub fn bind_async_closure<
        'l,
        R: ToLuaMulti + 'l,
        FUT: Future<Output = R> + Send + 'l,
        F: Fn(&'l State, i32) -> Result<FUT> + Send + 'static,
    >(
        &self,
        f: F,
    ) -> Result<Function> {
        if core::mem::size_of::<F>() == 0 {
            self.check_stack(1)?;
            self.push_cclosure(Some(async_closure_wrapper::<R, FUT, F>), 0);
        } else {
            self.check_stack(2)?;
            self.push_userdatauv(f, 0)?;
            self.push_binding(async_closure_wrapper::<R, FUT, F>, __gc::<F>, 0)?;
        }
        self.top_val().try_into()
    }

    /// not stack-balance
    pub(crate) async fn raw_call_async<'a>(
        &'a self,
        state: Option<&State>,
        guard: StackGuard<'a>,
        mut nargs: i32,
        nresult: i32,
    ) -> Result<StackGuard<'a>> {
        assert!(nargs >= 0 && nresult >= 0);

        loop {
            let mut nres = nresult;
            let status = {
                let from = state.map(State::as_ptr).unwrap_or(core::ptr::null_mut());
                self.resume(from, nargs, &mut nres)
            };
            match status {
                ThreadStatus::Yield => {
                    debug_assert!(nres > 0);

                    let taskwrap = unsafe {
                        self.to_userdata_typed::<TaskWrapper>(-1)
                            .ok_or("coroutine task expect a TaskWrapper")
                            .map_err(Error::runtime)?
                    };
                    if taskwrap.verify != continue_func as *const () as usize {
                        return Err(Error::runtime("expect a rust task in async coroutine"));
                    }

                    // yield(..., TaskWrapper)
                    let base = self.get_top() - nres + 1;
                    // pop the TaskWrapper
                    self.pop(1);

                    // execute the task
                    let task = taskwrap
                        .task
                        .take()
                        .ok_or("task is already moved")
                        .map_err(Error::runtime)?;
                    let state = unsafe { Self::from_raw_state(self.state) };
                    nargs = Box::into_pin(task(&state, base))
                        .await
                        .unwrap_or_else(|err| {
                            taskwrap.error.replace(err);
                            0
                        }) as _;
                    drop(state);

                    // keep the last nargs elements in stack
                    let top = self.get_top();
                    if top > base {
                        for i in 0..nargs {
                            self.copy(top + 1 - nargs + i, base + i);
                        }
                        self.set_top(base + nargs - 1);
                    } else {
                        debug_assert_eq!(top, base);
                    }
                }
                ThreadStatus::Ok => {
                    // at the end, function in coroutine was also poped
                    return Ok(guard);
                }
                err => {
                    core::mem::forget(guard);
                    let err = self
                        .statuscode_to_error_with_traceback(err as _, true)
                        .unwrap_err();
                    // TODO: reset thread graceful
                    unsafe {
                        lua_resetthread(self.state);
                    }
                    self.drop_slots_greater(self.get_top());
                    return Err(err);
                }
            }
        }
    }
}

pub unsafe extern "C-unwind" fn async_closure_wrapper<
    'l,
    R: ToLuaMulti + 'l,
    FUT: Future<Output = R> + Send + 'l,
    // Safety to remove Sync: The executor of the Future is expected to be on the current thread
    F: Fn(&'l State, i32) -> Result<FUT> + Send + 'static,
>(
    l: *mut lua_State,
) -> i32 {
    let state = State::from_raw_state(l);
    let fptr = if core::mem::size_of::<F>() == 0 {
        1
    } else {
        state.to_userdata(ffi::lua_upvalueindex(1)) as usize
    };
    state.yield_task(move |lua: &'l State, base| {
        let callback: &'l F = core::mem::transmute(fptr);
        let fut = callback(lua, base);
        Box::new(async move { fut?.await.push_multi(lua) })
    })
}

pub trait LuaAsyncMethod<'a, THIS: 'a, ARGS: 'a, RET: 'a>: Send {
    type Output: Future<Output = RET> + Send;

    fn call_method(&self, lua: &'a State, begin: Index) -> Result<Self::Output>;
}

macro_rules! impl_method {
    ($(($x:ident, $i:tt))*) => (
        // For normal function
        #[allow(unused_parens)]
        impl<'a,
            FN: Fn($($x),*) -> FUT + Send,
            FUT: Future<Output = RET> + Send + 'a,
            RET: ToLuaMulti + 'a,
            $($x: FromLua<'a> + 'a,)*
        > LuaAsyncMethod<'a, (), ($($x,)*), RET> for FN {
            type Output = FUT;

            #[inline(always)]
            fn call_method(&self, lua: &'a State, begin: Index) -> Result<Self::Output> {
                Ok(self($(check_from_lua::<$x>(lua, begin + $i)?),*))
            }
        }

        // For normal function with &LuaState
        #[allow(unused_parens)]
        impl<'a,
            FN: Fn(&'a State, $($x),*) -> FUT + Send,
            FUT: Future<Output = RET> + Send + 'a,
            RET: ToLuaMulti + 'a,
            $($x: FromLua<'a> + 'a,)*
        > LuaAsyncMethod<'a, (), (&'a State, $($x,)*), RET> for FN {
            type Output = FUT;

            #[inline(always)]
            fn call_method(&self, lua: &'a State, begin: Index) -> Result<Self::Output> {
                Ok(self(lua, $(check_from_lua::<$x>(lua, begin + $i)?),*))
            }
        }
    );
}

impl_method!();
impl_method!((A, 0));
impl_method!((A, 0)(B, 1));
impl_method!((A, 0)(B, 1)(C, 2));
impl_method!((A, 0)(B, 1)(C, 2)(D, 3));
impl_method!((A, 0)(B, 1)(C, 2)(D, 3)(E, 4));
impl_method!((A, 0)(B, 1)(C, 2)(D, 3)(E, 4)(F, 5));
impl_method!((A, 0)(B, 1)(C, 2)(D, 3)(E, 4)(F, 5)(G, 6));
impl_method!((A, 0)(B, 1)(C, 2)(D, 3)(E, 4)(F, 5)(G, 6)(H, 7));
impl_method!((A, 0)(B, 1)(C, 2)(D, 3)(E, 4)(F, 5)(G, 6)(H, 7)(I, 8));
impl_method!((A, 0)(B, 1)(C, 2)(D, 3)(E, 4)(F, 5)(G, 6)(H, 7)(I, 8)(J, 9));
impl_method!((A, 0)(B, 1)(C, 2)(D, 3)(E, 4)(F, 5)(G, 6)(H, 7)(I, 8)(J, 9)(K, 10));
impl_method!((A, 0)(B, 1)(C, 2)(D, 3)(E, 4)(F, 5)(G, 6)(H, 7)(I, 8)(J, 9)(K, 10)(L, 11));
