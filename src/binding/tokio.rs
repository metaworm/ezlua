use ::tokio::runtime::Handle;
use tokio::{sync::oneshot, task::JoinHandle};

use crate::{prelude::*, userdata::UserDataTrans};

pub struct TokioTask {
    join: JoinHandle<LuaResult<Reference>>,
}

impl TokioTask {
    pub async fn join<'a>(lua: &'a LuaState, this: LuaUserData<'a>) -> LuaResult<ValRef<'a>> {
        let this = this.take::<Self>().ok_or("task was taken").lua_result()?;
        lua.registry()
            .take_reference(this.join.await.lua_result()??)
    }
}

impl UserData for TokioTask {
    fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
        methods.set_closure("abort", |this: &Self| this.join.abort())?;

        methods.set_async_closure("join", TokioTask::join)?;

        Ok(())
    }
}

impl UserData for Handle {
    fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
        methods.set_closure("spawn", |this: &Self, routine: Coroutine| TokioTask {
            join: this.spawn(async move {
                routine
                    .call_async::<_, ValRef>(())
                    .await
                    .and_then(|v| routine.registry().reference(v))
            }),
        })?;

        methods.set(
            "block_on",
            methods
                .state()
                .new_closure2(|lua, this: &Self, routine: Coroutine| {
                    this.block_on(async move {
                        routine
                            .call_async::<_, ValRef>(())
                            .await
                            .and_then(|v| routine.registry().reference(v))
                    })
                    .and_then(|v| lua.registry().take_reference(v))
                })?,
        )?;

        methods.set_closure("spawn_blocking", |this: &Self, routine: Coroutine| {
            TokioTask {
                join: this.spawn_blocking(move || {
                    routine
                        .val(1)
                        .pcall::<_, ValRef>(())
                        .and_then(|v| routine.registry().reference(v))
                }),
            }
        })?;

        Ok(())
    }
}

pub fn open(lua: &LuaState) -> LuaResult<LuaTable> {
    let m = lua.new_table()?;

    m.set_closure("spawn", |routine: Coroutine| TokioTask {
        join: ::tokio::spawn(async move {
            routine
                .call_async::<_, ValRef>(())
                .await
                .and_then(|v| routine.registry().reference(v))
        }),
    })?;

    m.set_closure("spawn_blocking", |routine: Coroutine| TokioTask {
        join: ::tokio::task::spawn_blocking(move || {
            routine
                .val(1)
                .pcall::<_, ValRef>(())
                .and_then(|v| routine.registry().reference(v))
        }),
    })?;

    m.set_async_closure("sleep", ::tokio::time::sleep)?;

    m.set_closure("current_handle", || Handle::try_current().ok())?;

    {
        use ::tokio::sync::{mpsc::*, RwLock, RwLockReadGuard, RwLockWriteGuard};

        impl<T: UserData> UserDataTrans<T> for RwLock<T> {
            type Read<'a> = RwLockReadGuard<'a, T> where T: 'a;
            type Write<'a> = RwLockWriteGuard<'a, T> where T: 'a;

            fn trans(udata: T) -> Self {
                RwLock::new(udata)
            }

            fn read(&self) -> Self::Read<'_> {
                self.try_read().expect("")
            }
        }

        impl<'a, T: UserData<Trans = RwLock<T>>> FromLua<'a> for RwLockReadGuard<'a, T> {
            fn from_lua(lua: &'a LuaState, val: ValRef<'a>) -> LuaResult<Self> {
                val.check_safe_index()?;
                val.as_userdata()
                    .and_then(|u| u.userdata_ref::<T>())
                    .ok_or("userdata not match")
                    .lua_result()?
                    .try_read()
                    .lua_result()
                    // Safety: check_safe_index
                    .map(|x| unsafe { core::mem::transmute(x) })
            }
        }

        impl<'a, T: UserData<Trans = RwLock<T>>> FromLua<'a> for RwLockWriteGuard<'a, T> {
            fn from_lua(lua: &'a LuaState, val: ValRef<'a>) -> LuaResult<Self> {
                val.check_safe_index()?;
                val.as_userdata()
                    .and_then(|u| u.userdata_ref::<T>())
                    .ok_or("userdata not match")
                    .lua_result()?
                    .try_write()
                    .lua_result()
                    // Safety: check_safe_index
                    .map(|x| unsafe { core::mem::transmute(x) })
            }
        }

        impl UserData for Sender<Reference> {
            const TYPE_NAME: &'static str = "TokioSender";

            fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
                methods.set_async_closure(
                    "send",
                    async move |lua: &LuaState, this: &Self, val: ValRef| {
                        this.send(lua.registry().reference(val)?).await.lua_result()
                    },
                )?;
                methods.set_closure("try_send", |lua: &LuaState, this: &Self, val: ValRef| {
                    LuaResult::Ok(this.try_send(lua.registry().reference(val)?).is_ok())
                })?;
                methods.set_async_closure(
                    "send_timeout",
                    async move |lua: &LuaState, this: &Self, val: ValRef, tm| {
                        this.send_timeout(lua.registry().reference(val)?, tm)
                            .await
                            .lua_result()
                    },
                )?;

                Ok(())
            }
        }

        impl UserData for Receiver<Reference> {
            const TYPE_NAME: &'static str = "TokioReceiver";
            type Trans = RwLock<Self>;

            fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
                methods.add_method_mut("close", |_, this, ()| this.close())?;
                methods.add_method_mut("try_recv", |lua, this, ()| {
                    this.try_recv()
                        .lua_result()
                        .and_then(|r| lua.registry().take_reference(r))
                        .ok()
                })?;
                methods.add_method_mut("blocking_recv", |lua, this, ()| {
                    this.blocking_recv()
                        .map(|r| lua.registry().take_reference(r))
                        .transpose()
                })?;
                methods.set_async_closure(
                    "recv",
                    async move |lua: &LuaState, mut this: RwLockWriteGuard<Self>| {
                        this.recv()
                            .await
                            .map(|r| lua.registry().take_reference(r))
                            .transpose()
                    },
                )?;

                Ok(())
            }

            fn metatable(mt: UserdataRegistry<Self>) -> LuaResult<()> {
                mt.set("__call", mt.get("__method")?.get("recv")?)?;

                Ok(())
            }
        }

        impl UserData for UnboundedSender<Reference> {
            const TYPE_NAME: &'static str = "TokioUnboundedSender";

            fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
                methods.set_closure("send", |lua: &LuaState, this: &Self, val: ValRef| {
                    this.send(lua.registry().reference(val)?).lua_result()
                })?;
                methods.set_closure("same_channel", Self::same_channel)?;

                Ok(())
            }

            fn metatable(mt: UserdataRegistry<Self>) -> LuaResult<()> {
                mt.set("__eq", mt.get("__method")?.get("same_channel")?)?;

                Ok(())
            }
        }

        impl UserData for UnboundedReceiver<Reference> {
            const TYPE_NAME: &'static str = "TokioUnboundedReceiver";
            type Trans = RwLock<Self>;

            fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
                methods.add_method_mut("close", |_, this, ()| this.close())?;
                methods.add_method_mut("try_recv", |lua, this, ()| {
                    this.try_recv()
                        .lua_result()
                        .and_then(|r| lua.registry().take_reference(r))
                        .ok()
                })?;
                methods.add_method_mut("blocking_recv", |lua, this, ()| {
                    this.blocking_recv()
                        .map(|r| lua.registry().take_reference(r))
                        .transpose()
                })?;
                methods.set_async_closure(
                    "recv",
                    async move |lua: &LuaState, mut this: RwLockWriteGuard<Self>| {
                        this.recv()
                            .await
                            .map(|r| lua.registry().take_reference(r))
                            .transpose()
                    },
                )?;

                Ok(())
            }

            fn metatable(mt: UserdataRegistry<Self>) -> LuaResult<()> {
                mt.set("__call", mt.get("__method")?.get("recv")?)?;

                Ok(())
            }
        }

        impl UserData for oneshot::Sender<Reference> {
            const TYPE_NAME: &'static str = "TokioOneshotSender";

            fn getter(fields: UserdataRegistry<Self>) -> LuaResult<()> {
                fields.set_closure("is_closed", Self::is_closed)?;

                Ok(())
            }

            fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
                methods.set_closure(
                    "send",
                    |lua: &LuaState, OwnedUserdata(this), val: ValRef| {
                        Self::send(this, lua.registry().reference(val)?).lua_result()
                    },
                )?;

                Ok(())
            }
        }

        impl UserData for oneshot::Receiver<Reference> {
            const TYPE_NAME: &'static str = "TokioOneshotReceiver";

            fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
                methods.set(
                    "blocking_recv",
                    methods
                        .state()
                        .new_closure1(|lua, OwnedUserdata::<Self>(this)| {
                            this.blocking_recv()
                                .lua_result()
                                .and_then(|r| lua.registry().take_reference(r))
                        })?,
                )?;
                methods.set_async_closure(
                    "recv",
                    async move |lua: &LuaState, OwnedUserdata::<Self>(this)| {
                        this.await
                            .lua_result()
                            .and_then(|r| lua.registry().take_reference(r))
                    },
                )?;

                Ok(())
            }
        }

        let sync = lua.new_table()?;
        sync.set_closure("channel", channel::<Reference>)?;
        sync.set_closure("oneshot_channel", oneshot::channel::<Reference>)?;
        sync.set_closure("unbounded_channel", unbounded_channel::<Reference>)?;

        m.set("sync", sync)?;
    }

    Ok(m)
}