#![feature(async_closure)]

use ezlua::{prelude::*, serde::SerdeValue};

#[tokio::test]
async fn elua_async() {
    let lua = Lua::with_open_libs();
    let s = &lua;

    println!("lua: {:p}", s.raw_state());

    async fn echo_async<'a>(method: ValRef<'a>) -> impl ToLuaMulti + 'a {
        method
    }

    let g = s.global();
    g.register_async("echo_async", echo_async).unwrap();
    g.register_async("echo_async2", async move |vals: MultiValue| {
        {
            // std::println!("vals: {:?}", vals.0);
            vals
        }
    })
    .unwrap();
    g.set("sleep_async", s.async_closure(tokio::time::sleep).unwrap())
        .unwrap();

    let foo = s
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
    let s = &lua;

    s.do_string(
        "
    function add(_, t) return t[1] + t[2] end
    ",
        None,
    )
    .unwrap();

    let add = s.global().get("add").unwrap();
    assert_eq!(add.type_of(), LuaType::Function);

    // let co = Coroutine::new(add).unwrap();
    let ret = add
        .call_async::<_, ValRef>((s.global(), SerdeValue((1, 2))))
        .await
        .unwrap();
    assert_eq!(ret.cast::<i32>(), Some(3));
}

#[tokio::test]
async fn async_stack_balance() {
    let s = Lua::new();

    // pcall recycle multi value
    let foo = s.load("return ...", None).unwrap();
    for i in 0..20 {
        println!("top{i} {}", s.stack_top());
        let (_, _, s3) = foo
            .call_async::<_, (LuaValue, LuaValue, ValRef)>((1, 2, "3"))
            .await
            .unwrap();
        assert_eq!(s3.to_str().unwrap(), "3");
    }
}

#[tokio::test]
async fn async_error_balance() {
    let s = Lua::with_open_libs();

    let foo = s.load("error('error')", None).unwrap();
    let top = s.stack_top();
    for _ in 0..10 {
        foo.call_async_void((1, 2, 3)).await.unwrap_err();
        // println!("stack top: {}", s.stack_top());
        assert_eq!(s.stack_top(), top);
    }

    // TODO: more error case
}
