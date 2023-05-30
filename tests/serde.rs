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
