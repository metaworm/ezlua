use ezlua::prelude::*;

macro_rules! binding_test {
    ($name:ident, $path:expr) => {
        #[test]
        fn $name() {
            let s = Lua::with_open_libs();
            ezlua::binding::init_global(&s).unwrap();
            s.load_file($path).unwrap().pcall_void(()).unwrap();
        }
    };
}

#[cfg(feature = "regex")]
binding_test!(regex, "tests/test_regex.lua");

#[cfg(feature = "thread")]
binding_test!(thread, "tests/thread.lua");

binding_test!(stdlib, "tests/test_std.lua");

#[cfg(feature = "json")]
binding_test!(json, "tests/json.lua");
