use crate::{prelude::*, serde::SerdeValue};

impl ToLua for serde_json::Value {
    fn to_lua<'a>(self, s: &'a LuaState) -> LuaResult<ValRef<'a>> {
        s.new_val(SerdeValue(self))
    }
}

pub fn open(s: &LuaState) -> LuaResult<LuaTable> {
    let m = s.new_table()?;
    m.register1("load", |s: &LuaState, buf: &[u8]| {
        s.load_from_deserializer(&mut serde_json::Deserializer::from_slice(buf))
    })?;
    m.register("dump", |val: ValRef, pretty: LuaValue| match pretty {
        LuaValue::Bool(true) => serde_json::to_vec_pretty(&val),
        _ => serde_json::to_vec(&val),
    })?;
    m.register("print", |val: ValRef| {
        serde_json::to_writer(&mut std::io::stdout(), &val)
    })?;
    m.register("pprint", |val: ValRef| {
        serde_json::to_writer_pretty(&mut std::io::stdout(), &val)
    })?;

    Ok(m)
}
