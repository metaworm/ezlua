#![feature(async_closure)]

use std::time::Duration;

use ezlua::prelude::*;

#[tokio::test]
async fn elua_async() {
    let lua = Lua::with_open_libs();
    let _occupation = (0..20)
        .map(|_| lua.new_val(()).unwrap())
        .collect::<Vec<_>>();

    println!("lua: {:p}", lua.raw_state());

    async fn echo_async<'a>(method: ValRef<'a>) -> impl ToLuaMulti + 'a {
        method
    }

    let g = lua.global();
    g.set_async_closure("echo_async", echo_async).unwrap();
    g.set_async_closure("echo_async_multi", async move |vals: MultiValue| vals)
        .unwrap();
    g.set(
        "sleep_async",
        lua.async_closure(tokio::time::sleep).unwrap(),
    )
    .unwrap();

    let foo = lua
        .load(
            "
print('args:', ...)
local a, b, c = ...
print('unpacked:', a, b, c)
a, b, c = echo_async_multi(...)
print('echo:', a, b, c)
assert(a == 111 and b == 222 and c == 333)
a, b = echo_async(11, 22)
print(a, b)
assert(a == 11 and b == nil)

if error then error 'error test' end

sleep_async(0.5)
assert(echo_async(2) == 2)
assert(echo_async_multi(3) == 3)
return 1, 2
",
            None,
        )
        .unwrap();
    g.set("foo", foo.clone()).unwrap();

    let err = foo
        .call_async::<_, (i32, i32)>((111, 222, 333))
        .await
        .unwrap_err();
    println!("capture error: {err:?}");

    lua.global().set("error", LuaValue::Nil).unwrap();
    let foo = lua
        .global()
        .get("foo")
        .unwrap()
        .as_function()
        .unwrap()
        .clone();
    let ret = foo
        .call_async::<_, (i32, i32)>((111, 222, 333))
        .await
        .unwrap();
    assert_eq!(ret, (1, 2));
}

#[tokio::test]
async fn sync() {
    let lua = Lua::with_open_libs();
    let _occupation = (0..20)
        .map(|_| lua.new_val(()).unwrap())
        .collect::<Vec<_>>();

    lua.do_string(
        "
    function add(_, t) return t[1] + t[2] end
    ",
        None,
    )
    .unwrap();

    let add = lua
        .global()
        .getopt::<_, LuaFunction>("add")
        .unwrap()
        .unwrap();

    // let co = Coroutine::new(add).unwrap();
    let ret = add
        .call_async::<_, ValRef>((lua.global(), vec![1, 2]))
        .await
        .unwrap();
    assert_eq!(ret.cast::<i32>().unwrap(), 3);
}

#[tokio::test]
async fn async_stack_balance() {
    let lua = Lua::new();
    let _occupation = (0..20)
        .map(|_| lua.new_val(()).unwrap())
        .collect::<Vec<_>>();

    let mut stack_top = None;
    // pcall recycle multi value
    let foo = lua.load("return ...", None).unwrap();
    for i in 0..20 {
        println!("top{i} {}", lua.stack_top());
        let (_, _, s3) = foo
            .call_async::<_, (LuaValue, LuaValue, ValRef)>((1, 2, "3"))
            .await
            .unwrap();
        assert_eq!(s3.to_str().unwrap(), "3");
        if stack_top.is_none() {
            println!("after first call: {}", lua.stack_top());
            stack_top.replace(lua.stack_top());
        } else {
            assert_eq!(lua.stack_top(), stack_top.unwrap());
        }
    }
}

#[tokio::test]
async fn async_error_balance() {
    let lua = Lua::with_open_libs();
    let _occupation = (0..20)
        .map(|_| lua.new_val(()).unwrap())
        .collect::<Vec<_>>();

    let foo = lua.load("error('error')", None).unwrap();

    let mut stack_top = None;
    for _ in 0..10 {
        foo.call_async_void((1, 2, 3)).await.unwrap_err();
        // println!("stack top: {}", s.stack_top());
        if stack_top.is_none() {
            println!("after first call: {}", lua.stack_top());
            stack_top.replace(lua.stack_top());
        } else {
            assert_eq!(lua.stack_top(), stack_top.unwrap());
        }
    }

    let co = Coroutine::empty(&lua);
    let async_error = co
        .async_closure(async move |err: &str| {
            tokio::time::sleep(Duration::from_millis(1)).await;
            Err::<(), _>(err).lua_result()
        })
        .unwrap();
    for _ in 0..10 {
        println!(
            "{}",
            async_error.call_async_void("error").await.unwrap_err()
        );
    }

    // error catch in async context
    co.global().set("asyncerror", async_error).unwrap();
    let foo = co
        .load(
            "print(xpcall(function() asyncerror('error') end, debug.traceback))",
            None,
        )
        .unwrap();
    foo.call_async_void((1, 2, 3)).await.unwrap();

    // TODO: more error case
}

#[ignore = "manual"]
#[tokio::test]
async fn memory_leak() {
    let lua = Lua::with_open_libs();
    lua.register_module("json", ezlua::binding::json::open, true)
        .unwrap();
    lua.global()
        .set_function("test", |lua, b: bool| {
            if b {
                lua.new_val(1).map(Some)
            } else {
                Ok(None)
            }
        })
        .unwrap();

    lua.load(
        r"
        local data = json.loadfile('./target/.rustc_info.json')
        local b = true
        for i = 1, 10000000000000 do
            -- json.dump(json.loadfile('./target/.rustc_info.json'))
            json.dump(data)
            pcall(json.load, '[{}, --]')
            test(b) b = not b
            -- print(collectgarbage('count'))
        end
        ",
        None,
    )
    .unwrap()
    .call_async_void(())
    .await
    .unwrap();

    drop(lua);
}
