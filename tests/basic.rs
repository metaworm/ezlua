use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

use ::serde::{Deserialize, Serialize};
use ezlua::prelude::*;

struct Test {
    a: i32,
}

impl UserData for Test {
    type Trans = RefCell<Self>;

    const INDEX_USERVALUE: bool = true;

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

#[derive(derive_more::Deref, Clone)]
struct RcTest(Rc<Test>);

impl UserData for RcTest {
    fn key_to_cache(&self) -> *const () {
        self.as_ref() as *const _ as _
    }

    fn getter(fields: UserdataRegistry<Self>) -> LuaResult<()> {
        fields.set_closure("a", |this: &Self| this.a)?;
        Ok(())
    }

    fn methods(_: UserdataRegistry<Self>) -> LuaResult<()> {
        Ok(())
    }
}

#[test]
fn userdata() {
    let s = Lua::with_open_libs();
    let _occupation = (0..20).map(|_| s.new_val(()).unwrap()).collect::<Vec<_>>();

    let uv = s.new_val(Test { a: 0 }).unwrap();
    assert_eq!(uv.type_of(), LuaType::Userdata);

    // test getter/setter/method
    s.global().set("uv", uv).unwrap();
    s.do_string("print(getmetatable(uv), type(uv))", None)
        .unwrap();
    s.do_string("assert(uv.a == 0)", None).unwrap();
    s.do_string("uv:inc(); assert(uv.a == 1)", None).unwrap();
    s.do_string("uv.a = 3; assert(uv.a == 3)", None).unwrap();

    // test uservalue access
    s.do_string("uv.abc = 3; assert(uv.abc == 3)", None)
        .unwrap();
    s.do_string("assert(debug.getuservalue(uv).abc == 3)", None)
        .unwrap();

    // test userdata cache
    let test = RcTest(Test { a: 123 }.into());
    s.global().set("uv", test.clone()).unwrap();
    s.global().set("uv1", test.clone()).unwrap();
    s.do_string("print(uv, uv1)", None).unwrap();
    s.do_string("assert(uv == uv1)", None).unwrap();
    s.do_string("assert(uv.a == 123)", None).unwrap();
}

#[test]
fn iter() {
    let lua = Lua::with_open_libs();
    let _occupation = (0..20)
        .map(|_| lua.new_val(()).unwrap())
        .collect::<Vec<_>>();

    lua.global()
        .set_closure("toiter", || StaticIter::new(0..3))
        .unwrap();
    lua.do_string(
        r#"
        local iter = toiter()
        assert(iter() == 0)
        assert(iter() == 1)
        assert(iter() == 2)
    "#,
        None,
    )
    .unwrap();
}

#[test]
fn arguments() -> LuaResult<()> {
    let s = Lua::with_open_libs();
    let _occupation = (0..20).map(|_| s.new_val(()).unwrap()).collect::<Vec<_>>();

    struct Test;

    impl UserData for Test {
        fn methods(_: UserdataRegistry<Self>) -> LuaResult<()> {
            Ok(())
        }
    }

    println!("top1 {}", s.stack_top());
    s.load("assert(... == 123)", None)?
        .pcall_void(123)
        .expect("integer");

    println!("top2 {}", s.stack_top());
    s.load("assert(... == 123.0)", None)?
        .pcall_void(123.0)
        .expect("number");

    let top = s.stack_top();
    println!("top3 {}", s.stack_top());
    s.load("assert(type(...) == 'userdata')", None)?
        .pcall_void(Test)
        .expect("userdata");

    println!("top4 {}", s.stack_top());
    s.load("assert(type(...) == 'userdata')", None)?
        .pcall_void(Ok::<_, ()>(Test))
        .expect("Ok(userdata)");
    s.load("assert(select('#', ...) == 0)", None)?
        .pcall_void(Err::<Test, ()>(()))
        .expect("Err(())");

    assert_eq!(top, s.stack_top());

    Ok(())
}

#[test]
fn call_lua() {
    let s = Lua::with_open_libs();
    let _occupation = (0..20).map(|_| s.new_val(()).unwrap()).collect::<Vec<_>>();

    let g = s.global();
    assert_eq!(g.type_of(), LuaType::Table);

    println!("top: {}", s.stack_top());
    for i in 1..10 {
        let t = s.new_table_with_size(0, 0).unwrap();
        t.set("key", i).unwrap();
        g.seti(i, t).unwrap();
    }

    println!("top: {}", s.stack_top());
    for i in 1..10 {
        let t = g.geti(i).unwrap();
        assert_eq!(t.get("key").unwrap().cast::<i32>().unwrap(), i);
    }
    println!("top: {}", s.stack_top());

    let fun = s.load("print(...)", None).unwrap();
    fun.pcall::<_, ()>((1, 2, 3)).unwrap();

    g.set("g_number", 12345).unwrap();

    let result = s
        .load(
            r"
        assert(g_number == 12345)
        return {abc = 1234}
    ",
            None,
        )
        .unwrap()
        .pcall::<_, ValRef>(())
        .unwrap();
    assert!(result.get("abc").unwrap().cast::<i32>().unwrap() == 1234);
    println!("top: {}", s.stack_top());
}

#[test]
fn push_check() {
    let s = Lua::new();
    let _occupation = (0..20).map(|_| s.new_val(()).unwrap()).collect::<Vec<_>>();

    struct Test;

    impl ToLua for Test {
        fn to_lua<'a>(self, s: &'a LuaState) -> LuaResult<ValRef<'a>> {
            let result = s.new_table_with_size(0, 0)?;
            let t2 = s.new_table_with_size(0, 0)?;
            t2.set("field", 1234)?;
            result.set("t2", t2)?;
            result.to_lua(s)
        }
    }

    let result = s
        .load(
            r"
        return ...
    ",
            None,
        )
        .unwrap()
        .pcall::<_, ValRef>(Test)
        .unwrap();

    let t2 = result.get("t2").unwrap();
    assert!(t2.type_of() == LuaType::Table);

    assert_eq!(t2.get("field").unwrap().cast::<i32>().unwrap(), 1234);
}

#[test]
fn stack_balance() {
    let lua = Lua::new();

    let mut stack_top = None;
    // pcall recycle multi value
    let foo = lua.load("return ...", None).unwrap();
    for i in 0..20 {
        println!("top{i} {}", lua.stack_top());
        let (_, _, s3) = foo
            .pcall::<_, (LuaValue, LuaValue, ValRef)>((1, 2, "3"))
            .unwrap();
        assert!(s3.index() <= lua.stack_top());
        assert_eq!(s3.to_str().unwrap(), "3");
        if stack_top.is_none() {
            println!("after first call: {}", lua.stack_top());
            stack_top.replace(lua.stack_top());
        } else {
            assert_eq!(lua.stack_top(), stack_top.unwrap());
        }
    }
}

#[test]
fn table() {
    let lua = Lua::with_open_libs();
    for _ in 0..100 {
        lua.new_table().unwrap();
    }
}

#[test]
fn table_iter() {
    let s = Lua::with_open_libs();
    let _occupation = (0..20).map(|_| s.new_val(()).unwrap()).collect::<Vec<_>>();

    let g = s.global();
    let table: LuaTable = g.get("table").unwrap().try_into().unwrap();
    table.set("bb", false).unwrap();

    #[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
    struct Test {
        bb: bool,
        array: Vec<i32>,
    }

    let t = s.new_table().unwrap();
    for i in 1..=100 {
        t.set(i, i).unwrap();
    }

    let print = g.get("print").unwrap();

    for _ in 0..100 {
        let assert = s
            .load(
                r#"
    local i = 1
    return function(k, v)
        assert(k == i and v == i)
        i = i + 1
    end
    "#,
                None,
            )
            .unwrap()
            .pcall::<_, LuaFunction>(())
            .unwrap();

        table.set("array", t.clone()).unwrap();
        for (k, v) in table.iter().unwrap() {
            if let Some(v) = v.as_table() {
                for (k, v) in v.iter().unwrap() {
                    k.check_type(LuaType::Number).unwrap();
                    v.check_type(LuaType::Number).unwrap();
                    print.pcall_void((&k, &v, s.stack_top())).unwrap();
                    assert.pcall_void((k, v, s.stack_top())).unwrap();
                }
            } else {
                // k.check_type(LuaType::String).unwrap();
                // v.check_type(LuaType::Function).unwrap();
                print.pcall_void((k, v, s.stack_top())).unwrap();
            }
        }
        std::println!("[top] {}", s.stack_top());
    }
}

#[test]
fn table_get() {
    let lua = Lua::with_open_libs();
    let _occupation = (0..20)
        .map(|_| lua.new_val(()).unwrap())
        .collect::<Vec<_>>();

    lua.do_string("t = {}", None).unwrap();
    let t = lua.global().get("t").unwrap();
    t.set("key", "value").unwrap();
    assert_eq!(t.get("key").unwrap().cast::<String>().unwrap(), "value");

    t.seti(1, 1).unwrap();
    t.seti(2, 2).unwrap();
    t.seti(3, 3).unwrap();
    assert_eq!(t.len().unwrap().cast::<usize>().unwrap(), 3);

    lua.do_string(
        "t = setmetatable({}, {__index = function() error() end, __newindex = function() error() end, __len = function() error() end})",
        None,
    )
    .unwrap();
    let t = lua.global().get("t").unwrap();
    t.get("key").unwrap_err();
    t.geti(1).unwrap_err();
    t.len().unwrap_err();
}

#[ignore]
#[test]
fn safe_reference() {
    let s = Lua::with_open_libs();

    let chunk = s
        .load(
            r#"
    local buf = ...
    print(#buf)
    assert(#buf == 256)
    return buf
    "#,
            None,
        )
        .unwrap();

    let buf = (0..=u8::MAX).collect::<Vec<_>>();
    std::println!("top: {}", s.stack_top());
    let bufret: &[u8] = chunk.pcall(buf.as_slice()).unwrap();
    std::println!("top: {}", s.stack_top());

    s.do_string(
        r"
    collectgarbage 'incremental'
    collectgarbage 'collect'
    ",
        None,
    )
    .unwrap();

    for _ in 0..10 {
        for i in 0..1000 {
            s.global().set(i, &b"0000000000000000000000"[..]).unwrap();
        }

        assert_eq!(bufret, buf);
    }
}

#[test]
fn error_balance() {
    let lua = Lua::with_open_libs();
    let _occupation = (0..20)
        .map(|_| lua.new_val(()).unwrap())
        .collect::<Vec<_>>();

    let top = lua.stack_top();
    for _ in 0..10 {
        lua.load("...", None).unwrap_err();
        assert_eq!(lua.stack_top(), top);
    }

    let foo = lua.load("error('error')", None).unwrap();
    let top = lua.stack_top();
    for _ in 0..10 {
        foo.pcall_void((1, 2, 3)).unwrap_err();
        // println!("stack top: {}", s.stack_top());
        assert_eq!(lua.stack_top(), top);
    }
}

#[test]
fn convert() {
    let s = Lua::with_open_libs();
    let _occupation = (0..20).map(|_| s.new_val(()).unwrap()).collect::<Vec<_>>();

    s.global()
        .set_closure("test", |args: MultiRet<i32>| args.0.get(0).copied())
        .unwrap()
        .set_closure("readfile", |file: &str| NilError(std::fs::read(file)))
        .unwrap()
        .set_closure("itervec", |n: i32| IterVec(0..n))
        .unwrap()
        .set_closure("iterstr", |n: i32| {
            IterVec(
                (0..n)
                    .map(|i| format!("{i}"))
                    .collect::<Vec<_>>()
                    .into_iter(),
            )
        })
        .unwrap();

    s.do_string(
        r"
        assert(test() == nil)
        assert(test(1234) == 1234)
        local t = itervec(3)
        assert(t[1] == 0 and t[2] == 1)
        local t = iterstr(3)
        assert(t[1] == '0' and t[2] == '1')
        ",
        None,
    )
    .unwrap();

    s.do_string(
        r"
    local ok, err = readfile('/not/exists')
    assert(string.find(err, 'NotFound'))
    ",
        None,
    )
    .unwrap();
}

#[test]
fn convert_closure() {
    let lua = Lua::with_open_libs();
    let _occupation = (0..20)
        .map(|_| lua.new_val(()).unwrap())
        .collect::<Vec<_>>();

    let g = lua.global();
    let magic = 0x3310234;
    g.set("magic", magic).unwrap();
    g.set("closure", lua.new_closure(move || magic).unwrap())
        .unwrap();
    g.set(
        "tostr",
        lua.new_closure1(|_, b: &[u8]| String::from_utf8_lossy(b))
            .unwrap(),
    )
    .unwrap();

    lua.do_string(
        r"
        assert(closure() == magic)
        assert(tostr('1234abcd') == '1234abcd')
        ",
        None,
    )
    .unwrap();

    let test_value = 0x11223344;
    g.set_closure("test", move || test_value).unwrap();
    lua.do_string("assert(test() == 0x11223344)", None).unwrap();
}

#[test]
fn stack() {
    let lua = Lua::with_open_libs();
    let args = (0..100)
        .map(|i| lua.new_val(i))
        .flatten()
        .collect::<Vec<_>>();
    lua.load("print(...)", None)
        .unwrap()
        .pcall_void(MultiRet(args))
        .unwrap();
}

#[test]
fn non_table_access() {
    let lua = Lua::new();
    let t = lua.new_table().unwrap();
    let nil = lua.new_val(()).unwrap();
    let num = lua.new_val(123).unwrap();

    t.get("key").unwrap();

    nil.get("key").unwrap_err();
    num.get("key").unwrap_err();
    nil.set("key", "val").unwrap_err();
    num.set("key", "val").unwrap_err();

    nil.geti(1).unwrap_err();
    num.geti(1).unwrap_err();
    nil.seti(1, 2).unwrap_err();
    num.seti(1, 2).unwrap_err();

    nil.set_metatable(t).unwrap();
}
