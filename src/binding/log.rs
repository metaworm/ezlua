use alloc::{borrow::Cow, string::String};

use crate::prelude::*;

fn lualog(level: ::log::Level, s: &LuaState, arg: MultiValRef) {
    let d = s.stack(1);
    let source = d.as_ref().and_then(|d| d.source());
    let short_src = d.as_ref().map(|d| d.short_src());
    let name = d.as_ref().and_then(|d| d.name());
    let namewhat = d.as_ref().and_then(|d| d.namewhat());
    let mut buf = String::new();
    for arg in arg.0.into_iter() {
        if !buf.is_empty() {
            buf.push(' ');
        }
        buf.push_str(&arg.tostring());
    }

    ::log::logger().log(
        &::log::RecordBuilder::new()
            .module_path(name.or(namewhat).as_ref().map(Cow::as_ref))
            .file(source.or(short_src).as_ref().map(Cow::as_ref))
            .line(d.as_ref().map(|d| d.currentline as _))
            .level(level)
            .args(format_args!("{buf}"))
            .build(),
    )
}

pub fn open(s: &LuaState) -> LuaResult<LuaTable> {
    let m = s.new_table()?;

    m.set_closure("info", |s: &LuaState, a| lualog(log::Level::Info, s, a))?;
    m.set_closure("debug", |s: &LuaState, a| lualog(log::Level::Debug, s, a))?;
    m.set_closure("warn", |s: &LuaState, a| lualog(log::Level::Warn, s, a))?;
    m.set_closure("error", |s: &LuaState, a| lualog(log::Level::Error, s, a))?;
    m.set_closure("trace", |s: &LuaState, a| lualog(log::Level::Trace, s, a))?;

    Ok(m)
}
