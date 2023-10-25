use std::{
    fs::File,
    io::{BufReader, BufWriter},
};

use crate::{impl_fromlua_as_serde, impl_tolua_as_serde, prelude::*};

impl_tolua_as_serde!(serde_json::Value);
impl_fromlua_as_serde!(owned serde_json::Value);

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
            s.load_from_deserializer(&mut serde_json::Deserializer::from_reader(BufReader::new(
                File::open(path).lua_result()?,
            )))
        })?,
    )?;
    m.set_closure("dump", |val: ValRef, pretty: LuaValue| match pretty {
        LuaValue::Bool(true) => serde_json::to_vec_pretty(&val).map(LuaBytes),
        _ => serde_json::to_vec(&val).map(LuaBytes),
    })?;
    m.set_closure("dump_pretty", |val: ValRef| {
        serde_json::to_vec_pretty(&val).map(LuaBytes)
    })?;
    m.set_closure("dumpfile", |path: &str, val: ValRef| {
        serde_json::to_writer(BufWriter::new(File::create(path).lua_result()?), &val).lua_result()
    })?;
    m.set_closure("print", |val: ValRef| {
        serde_json::to_writer(&mut std::io::stdout(), &val)
    })?;
    m.set_closure("pprint", |val: ValRef| {
        serde_json::to_writer_pretty(&mut std::io::stdout(), &val)
    })?;

    Ok(m)
}
