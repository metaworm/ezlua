use std::path::Path;

#[derive(Debug, Default)]
struct Test {
    a: i32,
}

#[test]
fn test() {
    println!("------------- ezlua -------------");
    {
        use ezlua::prelude::*;
        use std::cell::*;

        impl UserData for Test {
            type Trans = RefCell<Self>;

            fn methods(mt: UserdataRegistry<Self>) -> LuaResult<()> {
                mt.set_closure("inc", |mut this: RefMut<Self>| this.a += 1)?;
                Ok(())
            }

            fn getter(fields: UserdataRegistry<Self>) -> LuaResult<()> {
                fields.set_closure("a", |this: Ref<Self>| this.a)?;
                Ok(())
            }

            fn setter(fields: UserdataRegistry<Self>) -> LuaResult<()> {
                fields.set_closure("a", |mut this: RefMut<Self>, val: i32| this.a = val)?;
                Ok(())
            }
        }

        let lua = Lua::with_open_libs();
        lua.global()
            .set_closure("add", |a: i64, b: i64| a + b)
            .unwrap();
        lua.global()
            .set(
                "strsub",
                lua.new_closure2(|_, s: &[u8], p: usize| &s[p..]).unwrap(),
            )
            .unwrap();
        lua.global().set_closure("getTest", Test::default).unwrap();

        lua.load_file("tests/bench.lua")
            .unwrap()
            .pcall_void(())
            .unwrap();
    }

    println!("------------- mlua -------------");
    {
        use mlua::prelude::*;

        impl LuaUserData for Test {
            fn add_fields<'lua, F: LuaUserDataFields<'lua, Self>>(fields: &mut F) {
                fields.add_field_method_get("a", |_, this| Ok(this.a));
                fields.add_field_method_set("a", |_, this, a: i32| {
                    this.a = a;
                    Ok(())
                });
            }

            fn add_methods<'lua, M: LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
                methods.add_method_mut("inc", |_, this, ()| {
                    this.a += 1;
                    Ok(())
                })
            }
        }

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
        lua.globals()
            .set(
                "getTest",
                lua.create_function(|_, ()| Ok(Test::default())).unwrap(),
            )
            .unwrap();

        lua.load(Path::new("tests/bench.lua"))
            .call::<_, ()>(())
            .unwrap();
    }
}
