use crate::{
    convert::*,
    coroutine::Coroutine,
    error::{Error, Result},
    ffi::{self, lua_State, lua_gettop, CFunction},
    luaapi::*,
    state::State,
    value::*,
};

use alloc::boxed::Box;
use core::{ffi::c_int, future::Future, marker::Tuple};

struct TaskWrapper<'a>(
    Option<
        Box<
            dyn FnOnce(&'a State, i32) -> Box<dyn Future<Output = Result<usize>> + Send + 'a>
                + Send
                + 'a,
        >,
    >,
);

impl ValRef<'_> {
    #[inline(always)]
    pub async fn call_async_void<'a, T: ToLuaMulti>(&'a self, args: T) -> Result<()> {
        self.call_async(args).await
    }

    #[inline(always)]
    pub async fn call_async<'a, T: ToLuaMulti, R: FromLuaMulti<'a>>(
        &'a self,
        args: T,
    ) -> Result<R> {
        self.call_async_from(args, None).await
    }

    #[inline(always)]
    pub async fn call_async_from<'a, T: ToLuaMulti, R: FromLuaMulti<'a>>(
        &'a self,
        args: T,
        state: Option<&State>,
    ) -> Result<R> {
        let guard = self.state.stack_guard();

        self.check_type(Type::Function)?;
        self.state
            .check_stack(args.value_count().unwrap_or(10) as i32 + 2)?;
        self.state.push_value(self.index);
        let count = R::COUNT as i32;
        self.state
            .raw_call_async(state, guard.top(), self.state.push_multi(args)? as _, count)
            .await?;
        let result_base = guard.top() + 1;
        self.state.to_multi_balance(guard, result_base)
    }

    #[inline(always)]
    pub fn register_async<
        'l,
        K: ToLua,
        A: FromLuaMulti<'l> + Tuple,
        R: ToLuaMulti + 'l,
        FUT: Future<Output = R> + Send + 'l,
        F: Fn<A, Output = FUT> + Sync + Send + 'static,
    >(
        &'l self,
        key: K,
        fun: F,
    ) -> Result<&Self> {
        self.set(key, self.state.async_closure(fun)?)?;
        Ok(self)
    }
}

impl Coroutine {
    #[inline(always)]
    pub async fn call_async<'a, T: ToLuaMulti, R: FromLuaMulti<'a>>(
        &'a self,
        args: T,
    ) -> Result<R> {
        self.call_async_from(args, None).await
    }

    #[inline(always)]
    pub async fn call_async_from<'a, T: ToLuaMulti, R: FromLuaMulti<'a>>(
        &'a self,
        args: T,
        state: Option<&State>,
    ) -> Result<R> {
        let guard = self.stack_guard();

        self.check_type(1, Type::Function)?;
        self.state
            .check_stack(args.value_count().unwrap_or(10) as i32 + 2)?;

        self.push_value(1);
        let count = R::COUNT as i32;
        self.raw_call_async(state, guard.top(), self.push_multi(args)? as _, count)
            .await?;
        let result_base = guard.top() + 1;
        self.to_multi_balance(guard, result_base)
    }
}

impl State {
    #[inline(always)]
    pub(crate) fn yield_task<
        'a,
        RET: ToLuaMulti + 'a,
        FUT: Future<Output = RET> + Send + 'a,
        F: FnOnce(&'a State, i32) -> crate::error::Result<FUT> + Send + 'a,
    >(
        self,
        callback: F,
    ) -> ! {
        self.push_userdatauv(
            TaskWrapper(Some(Box::new(|s: &'a State, base| {
                let fut = callback(s, base);
                Box::new(async move { fut?.await.push_multi(s) })
            }))),
            0,
        )
        .expect("push task wrapper");

        unsafe extern "C" fn continue_func(
            l: *mut lua_State,
            status: c_int,
            ctx: ffi::lua_KContext,
        ) -> c_int {
            lua_gettop(l)
        }

        unsafe {
            let l = self.as_ptr();
            let top = self.get_top();
            drop(self);
            ffi::lua_yieldk(l, top, 0, Some(continue_func));
        }
        panic!("co_yieldk called in non-coroutine context; check is_yieldable first")
    }

    /// Maps to `lua_pcallk`.
    pub fn pcallk<F>(&self, nargs: c_int, nresults: c_int, msgh: c_int, continuation: F) -> c_int
    where
        F: FnOnce(&State, ThreadStatus) -> c_int,
    {
        let func = continue_func::<F>;
        let ctx = Box::into_raw(continuation.into()) as _;
        unsafe {
            // lua_pcallk only returns if no yield occurs, so call the continuation
            func(
                self.as_ptr(),
                ffi::lua_pcallk(self.as_ptr(), nargs, nresults, msgh, ctx, Some(func)),
                ctx,
            )
        }
    }

    /// Maps to `lua_yield`.
    pub fn r#yield(&self, nresults: c_int) -> ! {
        unsafe { ffi::lua_yield(self.as_ptr(), nresults) };
        panic!("co_yieldk called in non-coroutine context; check is_yieldable first")
    }

    /// Maps to `lua_yieldk`.
    pub fn yieldk<F>(&self, nresults: c_int, continuation: F) -> !
    where
        F: FnOnce(&State, ThreadStatus) -> c_int,
    {
        let ctx = Box::into_raw(continuation.into()) as _;
        unsafe { ffi::lua_yieldk(self.as_ptr(), nresults, ctx, Some(continue_func::<F>)) };
        panic!("co_yieldk called in non-coroutine context; check is_yieldable first")
    }

    #[inline(always)]
    pub fn async_closure<
        'l,
        A: FromLuaMulti<'l> + Tuple,
        R: ToLuaMulti + 'l,
        FUT: Future<Output = R> + Send + 'l,
        F: Fn<A, Output = FUT> + Sync + Send + 'static,
    >(
        &'l self,
        fun: F,
    ) -> Result<ValRef> {
        self.to_async_closure_wrapper(move |s, base| Result::Ok(fun.call(A::from_lua(s, base)?)))
    }

    #[inline(always)]
    fn to_async_closure_wrapper<
        'l,
        R: ToLuaMulti + 'l,
        FUT: Future<Output = R> + Send + 'l,
        F: Fn(&'l State, i32) -> Result<FUT> + Sync + Send + 'l,
    >(
        &self,
        f: F,
    ) -> Result<ValRef> {
        if core::mem::size_of::<F>() == 0 {
            self.push_cclosure(Some(async_closure_wrapper::<R, FUT, F>), 0);
        } else {
            self.push_userdatauv(f, 0)?;
            let mt = self.create_table(0, 1)?;
            mt.set("__gc", __gc::<F> as CFunction)?;
            mt.0.ensure_top();
            self.set_metatable(-2);
            self.push_cclosure(Some(async_closure_wrapper::<R, FUT, F>), 1);
        }
        Ok(self.top_val())
    }

    /// not stack-balance
    pub(crate) async fn raw_call_async(
        &self,
        state: Option<&State>,
        origin_top: i32,
        mut nargs: i32,
        nresult: i32,
    ) -> Result<i32> {
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
                    // std::println!("nres: {nres}");

                    let task = unsafe {
                        self.to_userdata_typed::<TaskWrapper>(-1)
                            .ok_or("coroutine task expect a TaskWrapper")
                            .map_err(Error::runtime)?
                            .0
                            .take()
                            .ok_or("task is already moved")
                            .map_err(Error::runtime)?
                    };

                    // yield(..., TaskWrapper)
                    let base = self.get_top() - nres + 1;
                    // pop the TaskWrapper
                    self.pop(1);

                    // execute the task
                    nargs = Box::into_pin(task(self, base)).await? as _;

                    // keep the last nargs elements in stack
                    let top = self.get_top();
                    if top > base {
                        for i in 0..nargs {
                            self.copy(top + 1 - nargs + i, base + i);
                        }
                        self.set_top(base + nargs - 1);
                    } else {
                        debug_assert!(top == base);
                    }
                    // std::println!("nargs: {nargs}");
                }
                ThreadStatus::Ok => {
                    // at the end, function in coroutine was also poped
                    return Ok(nresult);
                }
                err => {
                    self.statuscode_to_error(err as _)?;
                }
            }
        }
    }
}

unsafe extern "C" fn continue_func<F>(
    l: *mut lua_State,
    status: c_int,
    ctx: ffi::lua_KContext,
) -> c_int
where
    F: FnOnce(&State, ThreadStatus) -> c_int,
{
    core::mem::transmute::<_, Box<F>>(ctx)(
        &State::from_raw_state(l),
        ThreadStatus::from_c_int(status),
    )
}

pub unsafe extern "C" fn async_closure_wrapper<
    'l,
    R: ToLuaMulti + 'l,
    FUT: Future<Output = R> + Send + 'l,
    F: Fn(&'l State, i32) -> Result<FUT> + Sync + Send + 'l,
>(
    l: *mut lua_State,
) -> i32 {
    let state = State::from_raw_state(l);
    #[allow(unused_assignments)]
    let mut pfn = core::mem::transmute(1usize);
    let f: &'l F = if core::mem::size_of::<F>() == 0 {
        core::mem::transmute(pfn)
    } else {
        pfn = state.to_userdata(ffi::lua_upvalueindex(1));
        core::mem::transmute(pfn)
    };

    state.yield_task(f)
}
