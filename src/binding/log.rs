use alloc::borrow::Cow;

use crate::prelude::*;

fn lualog(level: ::log::Level, arg: ValRef) {
    let d = arg.state().stack(1);
    let source = d.as_ref().and_then(|d| d.source());
    let short_src = d.as_ref().map(|d| d.short_src());
    let name = d.as_ref().and_then(|d| d.name());
    let namewhat = d.as_ref().and_then(|d| d.namewhat());
    ::log::logger().log(
        &::log::RecordBuilder::new()
            .module_path(name.or(namewhat).as_ref().map(Cow::as_ref))
            .file(source.or(short_src).as_ref().map(Cow::as_ref))
            .line(d.as_ref().map(|d| d.currentline as _))
            .level(level)
            .args(format_args!("{}", arg.to_str_lossy().unwrap_or_default()))
            .build(),
    )
}

pub fn open(s: &LuaState) -> LuaResult<LuaTable> {
    let m = s.new_table()?;

    m.register1("info", |s: &LuaState, arg: ValRef| {
        lualog(log::Level::Info, arg)
    })?;
    m.register1("debug", |s: &LuaState, arg: ValRef| {
        lualog(log::Level::Debug, arg)
    })?;
    m.register1("warn", |s: &LuaState, arg: ValRef| {
        lualog(log::Level::Warn, arg)
    })?;
    m.register1("error", |s: &LuaState, arg: ValRef| {
        lualog(log::Level::Error, arg)
    })?;
    m.register1("trace", |s: &LuaState, arg: ValRef| {
        lualog(log::Level::Trace, arg)
    })?;

    Ok(m)
}
