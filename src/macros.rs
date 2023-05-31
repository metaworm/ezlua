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

#[macro_export]
macro_rules! impl_tolua_as_serde {
    ($t:ty) => {
        impl $crate::prelude::ToLua for $t {
            fn to_lua<'a>(
                self,
                lua: &'a $crate::prelude::LuaState,
            ) -> $crate::prelude::LuaResult<$crate::prelude::ValRef<'a>> {
                $crate::prelude::ToLua($crate::serde::SerdeValue(self), lua)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_fromlua_as_serde {
    ($t:ty) => {
        impl $crate::prelude::FromLua<'_> for $t {
            fn from_lua(
                lua: &$crate::prelude::LuaState,
                val: $crate::prelude::ValRef,
            ) -> Option<Self> {
                <$crate::serde::SerdeValue<Self> as $crate::prelude::FromLua>::from_lua(lua, val)
                    .map(|s| s.0)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_toluamulti {
    ($t:ty as $as:ty: |$self:ident| $map:expr) => {
        impl $crate::prelude::ToLuaMulti for $t {
            const VALUE_COUNT: Option<usize> = <$as as $crate::prelude::ToLuaMulti>::VALUE_COUNT;

            fn push_multi(
                $self,
                lua: &$crate::prelude::LuaState,
            ) -> $crate::prelude::LuaResult<usize> {
                $crate::prelude::ToLuaMulti::push_multi($map, lua)
            }
        }
    };
}
