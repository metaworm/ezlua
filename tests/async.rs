#![feature(async_closure)]

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
    g.set_async_closure("echo_async2", async move |vals: MultiValue| {
        {
            // std::println!("vals: {:?}", vals.0);
            vals
        }
    })
    .unwrap();
    g.set(
        "sleep_async",
        lua.async_closure(tokio::time::sleep).unwrap(),
    )
    .unwrap();

    let foo = lua
        .load(
            "
local a, b, c = echo_async2(...)
print(a, b, c)
assert(a == 111 and b == 222 and c == 333)
a, b = echo_async(11, 22)
print(a, b)
assert(a == 11 and b == nil)

-- error 'error test'
sleep_async(0.5)
assert(echo_async(2) == 2)
assert(echo_async2(3) == 3)
return 1, 2
",
            None,
        )
        .unwrap();

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
    // let _occupation = (0..20)
    //     .map(|_| lua.new_val(()).unwrap())
    //     .collect::<Vec<_>>();

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

    // TODO: more error case
}
