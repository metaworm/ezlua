use crate::{luaapi::UnsafeLuaApi, state::State, value::Value};
use alloc::sync::Arc;

pub(crate) type ArcLuaInner = Arc<LuaInner>;

#[derive(Debug)]
pub struct Lua(ArcLuaInner);

impl core::ops::Deref for Lua {
    type Target = State;

    fn deref(&self) -> &Self::Target {
        &self.0 .0
    }
}

impl Lua {
    pub fn new() -> Self {
        let result = Self(LuaInner(unsafe { State::from_raw_state(State::new()) }).into());
        result
            .registry()
            .set(
                Value::light_userdata(Lua::new as *const ()),
                Value::light_userdata(Arc::as_ptr(&result.0)),
            )
            .expect("init luainner");
        result
    }

    pub fn with_open_libs() -> Self {
        let this = Self::new();
        this.open_libs();
        this
    }
}

#[derive(Debug)]
pub(crate) struct LuaInner(pub State);

impl Drop for LuaInner {
    fn drop(&mut self) {
        self.0.close();
    }
}

impl State {
    pub(crate) fn lua_inner(&self) -> ArcLuaInner {
        match self
            .registry()
            .get(Value::light_userdata(Lua::new as *const ()))
            .expect("get")
            .into_value()
        {
            Value::LightUserdata(p) => unsafe {
                Arc::increment_strong_count(p);
                Arc::from_raw(p as *const LuaInner)
            },
            _ => panic!("main state pointer not set"),
        }
    }
}
