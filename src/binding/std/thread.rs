use crate::coroutine::CoroutineWithRef;

use super::*;

use core::cell::RefCell;
#[cfg(not(target_os = "windows"))]
use std::os::unix::thread::{JoinHandleExt, RawPthread as RawHandle};
#[cfg(target_os = "windows")]
use std::os::windows::io::{AsRawHandle, RawHandle};
use std::sync::{mpsc::*, *};
use std::thread::{self, JoinHandle};
use std::time::Duration;

struct LuaThread {
    handle: RawHandle,
    join: JoinHandle<LuaResult<CoroutineWithRef>>,
}

impl UserData for LuaThread {
    const TYPE_NAME: &'static str = "LLuaThread";

    fn getter(fields: UserdataRegistry<Self>) -> Result<()> {
        fields.set_closure("handle", |this: &Self| this.handle as usize)?;
        fields.add_method("name", |s, this, ()| this.join.thread().name())?;
        fields.set_closure("id", |this: &Self| this.join.thread().id().as_u64().get())?;

        Ok(())
    }

    fn methods(mt: UserdataRegistry<Self>) -> Result<()> {
        mt.set(
            "join",
            mt.state()
                .new_closure1(|lua, OwnedUserdata::<Self>(this)| {
                    this.join.join().lua_result()??.take(lua)
                })?,
        )?;
        mt.set_closure("unpark", |this: &Self| this.join.thread().unpark())?;

        Ok(())
    }
}

#[cfg(target_os = "windows")]
const RAW_NULL: RawHandle = std::ptr::null_mut();
#[cfg(not(target_os = "windows"))]
const RAW_NULL: RawHandle = 0;

#[derive(Default, Deref, AsRef)]
struct LuaMutex(Mutex<()>);
struct LuaMutexGaurd<'a>(MutexGuard<'a, ()>);

impl<'a> UserData for LuaMutexGaurd<'a> {
    const TYPE_NAME: &'static str = "LuaMutexGaurd";

    fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
        methods.set_closure("unlock", ValRef::close)?;
        Ok(())
    }
}

impl UserData for LuaMutex {
    const TYPE_NAME: &'static str = "LuaMutex";

    fn methods(mt: UserdataRegistry<Self>) -> Result<()> {
        mt.as_deref()
            .add_deref("is_poisoned", Mutex::<()>::is_poisoned)?;
        mt.add_method("lock", |s, this, ()| {
            s.new_userdata_with_values(LuaMutexGaurd(this.0.lock().lua_result()?), [ArgRef(1)])
        })?;
        mt.add_method("try_lock", |s, this, ()| {
            NilError(
                this.0
                    .try_lock()
                    .lua_result()
                    .map(LuaMutexGaurd)
                    .map(|guard| s.new_userdata_with_values(guard, [ArgRef(1)])),
            )
        })?;

        Ok(())
    }
}

#[derive(Debug)]
struct LuaCondVar {
    lock: Mutex<Reference>,
    cvar: Condvar,
}

impl Default for LuaCondVar {
    fn default() -> Self {
        Self {
            lock: Reference(0).into(),
            cvar: Default::default(),
        }
    }
}

impl UserData for LuaCondVar {
    const TYPE_NAME: &'static str = "LuaCondVar";

    fn methods(mt: UserdataRegistry<Self>) -> Result<()> {
        mt.add_method("wait", |s, this, tm: Option<u64>| this.wait(s, tm))?;
        mt.add_method("notify_one", |s, this, val: ValRef| {
            this.push_some(s, val).map(|_| this.cvar.notify_one())
        })?;
        mt.add_method("notify_all", |s, this, val: ValRef| {
            this.push_some(s, val).map(|_| this.cvar.notify_all())
        })?;

        Ok(())
    }
}

impl LuaCondVar {
    fn push_some(&self, s: &LuaState, val: ValRef) -> Result<()> {
        let mut i = self.lock.lock().lua_result()?;
        let creg = s.registry();
        creg.unreference((*i).into());
        *i = creg.reference(val)?;

        Ok(())
    }

    fn wait<'a>(&self, s: &'a LuaState, timeout: Option<u64>) -> Result<ValRef<'a>> {
        let lock = &self.lock;
        let cvar = &self.cvar;
        if let Some(tm) = timeout {
            let (i, r) = cvar
                .wait_timeout(lock.lock().lua_result()?, Duration::from_millis(tm))
                .map_err(LuaError::runtime_debug)?;
            if r.timed_out() {
                return s.new_val(());
            }
            s.registry().take_reference(*i)
        } else {
            let i = cvar
                .wait(lock.lock().lua_result()?)
                .map_err(LuaError::runtime_debug)?;
            s.registry().take_reference(*i)
        }
    }
}

impl UserData for Sender<Reference> {
    const TYPE_NAME: &'static str = "Sender";

    fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
        methods.set_closure("send", |lua: &LuaState, this: &Self, val: ValRef| {
            this.send(lua.registry().reference(val)?).lua_result()
        })?;

        Ok(())
    }
}

impl UserData for Receiver<Reference> {
    const TYPE_NAME: &'static str = "Receiver";
    type Trans = RefCell<Self>;

    fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
        methods.add_method_mut("recv", |lua: &LuaState, this, ()| {
            this.recv()
                .lua_result()
                .and_then(|r| lua.registry().take_reference(r))
        })?;
        methods.add_method_mut("try_recv", |lua: &LuaState, this, ()| {
            match this.try_recv() {
                Err(TryRecvError::Empty) => lua.new_val(()),
                err => err
                    .lua_result()
                    .and_then(|r| lua.registry().take_reference(r)),
            }
        })?;
        methods.add_method_mut("recv_timeout", |lua: &LuaState, this, tm| {
            this.recv_timeout(tm)
                .lua_result()
                .and_then(|r| lua.registry().take_reference(r))
        })?;

        Ok(())
    }

    fn metatable(mt: UserdataRegistry<Self>) -> LuaResult<()> {
        mt.set("__call", mt.get("__method")?.get("recv")?)?;

        Ok(())
    }
}

pub fn init(lua: &LuaState) -> Result<LuaTable> {
    let t = lua.new_table_with_size(0, 4)?;
    t.set_closure("spawn", |routine: Coroutine, name: Option<&str>| {
        thread::Builder::new()
            .name(name.unwrap_or("<lua>").into())
            .spawn(move || {
                let result = routine
                    .val(1)
                    .pcall::<_, ValRef>(())
                    .and_then(|res| routine.registry().reference(res));
                result.map(|refer| CoroutineWithRef(routine, refer))
            })
            .map(|join| {
                #[cfg(target_os = "windows")]
                let handle = join.as_raw_handle();
                #[cfg(not(target_os = "windows"))]
                let handle = join.as_pthread_t();

                LuaThread { join, handle }
            })
    })?;
    t.set_closure("sleep", |time: u64| {
        thread::sleep(Duration::from_millis(time))
    })?;
    t.set_closure("park", thread::park)?;
    t.set_closure("id", || thread::current().id().as_u64().get())?;
    t.set_closure("yield_now", thread::yield_now)?;
    t.set_closure("mutex", LuaMutex::default)?;
    t.set_closure("condvar", LuaCondVar::default)?;
    t.set(
        "name",
        lua.new_closure0(|s| s.new_val(thread::current().name()))?,
    )?;

    let sync = lua.new_table()?;
    sync.set_closure("channel", channel::<Reference>)?;
    t.set("sync", sync)?;

    Ok(t)
}

pub fn wrap_sender<'l, T: FromLuaMulti<'l> + Send + 'static>(
    lua: &'l LuaState,
    sender: Sender<T>,
) -> LuaResult<LuaFunction> {
    lua.new_function(move |_, val: T| sender.send(val).is_ok())
}

pub fn wrap_receiver<'l, T: ToLuaMulti + Send + 'l + 'static>(
    lua: &'l LuaState,
    recver: Receiver<T>,
) -> LuaResult<LuaFunction> {
    let recver = RwLock::new(recver);
    lua.new_function(move |l, ()| {
        if let Ok(x) = recver.write().unwrap().recv() {
            l.pushed(x)
        } else {
            l.pushed(())
        }
    })
}
