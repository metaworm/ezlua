use std::path::Path;

#[test]
fn test() {
    println!("------------- ezlua -------------");
    {
        use ezlua::prelude::*;
        let lua = Lua::with_open_libs();
        lua.global()
            .register("add", |a: i64, b: i64| a + b)
            .unwrap();
        lua.global()
            .register2("strsub", |_, s: &[u8], p: usize| &s[p..])
            .unwrap();
        lua.load_file("tests/bench.lua")
            .unwrap()
            .pcall_void(())
            .unwrap();
    }

    println!("------------- mlua -------------");
    {
        use mlua::prelude::*;

        let lua = unsafe { Lua::unsafe_new() };
        lua.load_from_std_lib(
            LuaStdLib::TABLE
                | LuaStdLib::MATH
                | LuaStdLib::IO
                | LuaStdLib::STRING
                | LuaStdLib::COROUTINE
                | LuaStdLib::PACKAGE
                | LuaStdLib::DEBUG
                | LuaStdLib::OS,
        )
        .expect("load stdlib");
        lua.globals()
            .set(
                "add",
                lua.create_function(|_, (a, b): (i64, i64)| Ok(a + b))
                    .unwrap(),
            )
            .unwrap();
        lua.globals()
            .set(
                "strsub",
                lua.create_function(|lua, (s, p): (LuaString, usize)| {
                    lua.create_string(&s.as_bytes()[p..])
                })
                .unwrap(),
            )
            .unwrap();
        lua.load(Path::new("tests/bench.lua"))
            .call::<_, ()>(())
            .unwrap();
    }
}
