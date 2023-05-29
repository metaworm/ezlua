#[cfg(not(feature = "vendored"))]
#[macro_export]
macro_rules! lua_module {
    ($name:ident, $init:expr) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(l: *mut $crate::ffi::lua_State) -> i32 {
            $crate::convert::module_function_wrapper($init)(l)
        }
    };
}

#[cfg(feature = "vendored")]
#[macro_export]
macro_rules! lua_module {
    ($name:ident, $init:expr) => {
        compile_error!("Feature `vendored` should be not enabled when crate build as a lua module");
    };
}
