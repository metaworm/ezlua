use crate::{prelude::*, serde::SerdeValue};

impl ToLua for serde_json::Value {
    fn to_lua<'a>(self, s: &'a LuaState) -> LuaResult<ValRef<'a>> {
        s.new_val(SerdeValue(self))
    }
}

pub fn open(s: &LuaState) -> LuaResult<LuaTable> {
    let m = s.new_table()?;
    m.set(
        "load",
        s.new_closure1(|s: &LuaState, buf: &[u8]| {
            s.load_from_deserializer(&mut serde_json::Deserializer::from_slice(buf))
        })?,
    )?;
    m.set(
        "loadfile",
        s.new_closure1(|s: &LuaState, path: &str| {
            s.load_from_deserializer(&mut serde_json::Deserializer::from_reader(
                std::fs::File::open(path).lua_result()?,
            ))
        })?,
    )?;
    m.set_closure("dump", |val: ValRef, pretty: LuaValue| match pretty {
        LuaValue::Bool(true) => serde_json::to_vec_pretty(&val).map(LuaBytes),
        _ => serde_json::to_vec(&val).map(LuaBytes),
    })?;
    m.set_closure("dump_pretty", |val: ValRef| {
        serde_json::to_vec_pretty(&val).map(LuaBytes)
    })?;
    m.set_closure("print", |val: ValRef| {
        serde_json::to_writer(&mut std::io::stdout(), &val)
    })?;
    m.set_closure("pprint", |val: ValRef| {
        serde_json::to_writer_pretty(&mut std::io::stdout(), &val)
    })?;

    Ok(m)
}
