#![cfg(feature = "serde")]

use ::serde::{Deserialize, Serialize};
use ezlua::{prelude::*, serde::SerdeValue};

#[test]
fn overview() {
    #[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
    struct Test<'a> {
        str: &'a str,
        int: i32,
        flt: f64,
    }

    let lua = Lua::with_open_libs();
    let _occupation = (0..20)
        .map(|_| lua.new_val(()).unwrap())
        .collect::<Vec<_>>();

    let global = lua.global();
    let test = Test {
        str: "abc",
        int: 333,
        flt: 123.0,
    };
    global.set("test", SerdeValue(test.clone())).unwrap();
    global
        .set_closure("print_test", |SerdeValue::<Test>(x)| std::println!("{x:?}"))
        .unwrap();
    let test1 = test.clone();
    global
        .set_closure("assert_test", move |SerdeValue::<Test>(x)| {
            assert_eq!(test1, x)
        })
        .unwrap();
    lua.do_string("assert_test(test); print_test(test)", None)
        .unwrap();
    assert!(global
        .getopt::<_, SerdeValue<Test>>("test")
        .unwrap()
        .is_none());

    let t = global.getopt::<_, ValRef>("test").unwrap().unwrap();
    assert_eq!(test, t.deserialize::<Test>().unwrap());

    global.set("test", SerdeValue(1234)).unwrap();
    lua.do_string("print('regval', test)", None).unwrap();
}

#[test]
fn array_null() {
    let lua = Lua::with_open_libs();
    let _occupation = (0..20)
        .map(|_| lua.new_val(()).unwrap())
        .collect::<Vec<_>>();

    lua.global()
        .set(
            "array",
            lua.new_function(|lua, t: LuaTable| {
                t.set_metatable(lua.array_metatable()?)?;
                LuaResult::Ok(t)
            })
            .unwrap(),
        )
        .unwrap();

    let obj: ValRef = lua.load("return {}", None).unwrap().pcall(()).unwrap();
    let arr: ValRef = lua
        .load("return array {}", None)
        .unwrap()
        .pcall(())
        .unwrap();

    assert_eq!(serde_json::to_string(&obj).unwrap(), "{}");
    assert_eq!(serde_json::to_string(&arr).unwrap(), "[]");

    lua.global().set("null", lua.null_value()).unwrap();
    let null: ValRef = lua
        .load("return array {null}", None)
        .unwrap()
        .pcall(())
        .unwrap();
    let empty: ValRef = lua
        .load("return array {nil}", None)
        .unwrap()
        .pcall(())
        .unwrap();

    assert_eq!(serde_json::to_string(&null).unwrap(), "[null]");
    assert_eq!(serde_json::to_string(&empty).unwrap(), "[]");
}

#[test]
fn reference() {
    let lua = Lua::with_open_libs();
    let _occupation = (0..20)
        .map(|_| lua.new_val(()).unwrap())
        .collect::<Vec<_>>();

    let g = lua.global();
    lua.do_string("s = '123'", None).unwrap();
    lua.do_string("t = {'1','2','3'}", None).unwrap();

    g.get("s").unwrap().cast_into::<&str>().unwrap_err();
    // g.get("t").unwrap().cast_into::<Vec<&str>>().unwrap_err();

    g.get("s").unwrap().deserialize::<&str>().unwrap();
    g.get("t").unwrap().deserialize::<Vec<&str>>().unwrap();
}

#[cfg(feature = "json")]
#[test]
fn nested() {
    let lua = Lua::with_open_libs();
    let _occupation = (0..20)
        .map(|_| lua.new_val(()).unwrap())
        .collect::<Vec<_>>();

    // let g = lua.global();
    lua.register_module("json", ezlua::binding::json::open, true)
        .unwrap();
    lua.do_string(
        "
    local t1 = {abc = 123}
    local t2 = {parent = t1}
    t1.child = t2
    json.dump(t1)
    ",
        None,
    )
    .unwrap_err();
}

#[test]
fn serde_enum() {
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    enum Enum {
        Null,
        Empty,
        Int(isize),
        Float(f64),
        Vec(Vec<i32>),
        NewTuple(i32, String),
        NewStruct { abc: i32, def: String },
    }

    let lua = Lua::with_open_libs();
    let empty = lua.new_val(SerdeValue(Enum::Empty)).unwrap();
    assert_eq!(empty.deserialize::<Enum>().unwrap(), Enum::Empty);

    let null = lua.new_val(SerdeValue(Enum::Null)).unwrap();
    assert_eq!(null.deserialize::<Enum>().unwrap(), Enum::Null);

    let int = lua.new_val(SerdeValue(Enum::Int(123))).unwrap();
    assert_eq!(int.deserialize::<Enum>().unwrap(), Enum::Int(123));

    let flt = lua.new_val(SerdeValue(Enum::Float(123.0))).unwrap();
    assert_eq!(flt.deserialize::<Enum>().unwrap(), Enum::Float(123.0));

    let newtype = Enum::NewTuple(111, "ddd".into());
    let new = lua.new_val(SerdeValue(&newtype)).unwrap();
    assert_eq!(new.deserialize::<Enum>().unwrap(), newtype);

    let newtype = Enum::NewStruct {
        abc: 111,
        def: "ddd".into(),
    };
    let new = lua.new_val(SerdeValue(&newtype)).unwrap();
    // println!("{:?}", serde_json::to_string(&new).unwrap());
    assert_eq!(new.deserialize::<Enum>().unwrap(), newtype);
}

#[ignore = "manual"]
#[test]
fn memory_leak() {
    let lua = Lua::with_open_libs();

    lua.register_module("json", ezlua::binding::json::open, true)
        .unwrap();

    lua.load(
        r"
    local data = json.loadfile('./target/.rustc_info.json')
    for i = 1, 10000000000 do
        -- json.dump(data)
        json.dump(json.loadfile('./target/.rustc_info.json'))
        collectgarbage()
    end
    ",
        None,
    )
    .unwrap()
    .pcall_void(())
    .unwrap();
}
