/// Helper macro to create lua module
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

/// Helper macro to create lua module
#[cfg(feature = "vendored")]
#[macro_export]
macro_rules! lua_module {
    ($name:ident, $init:expr) => {
        compile_error!("Feature `vendored` should be not enabled when crate build as a lua module");
    };
}

/// Helper macro to `impl ToLua` for serializable types easily
///
/// ```rust
/// use elua::prelude::*;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Default, Deserialize, Serialize)]
/// struct Config {
///     name: String,
///     path: String,
///     timeout: u64,
///     // ...
/// }
///
/// ezlua::impl_tolua_as_serde!(Config);
///
/// let lua = Lua::with_open_libs();
/// lua.global().set("config", Config::default())?;
/// // config in lua is `{name = '', path = '', timeout = 0}`
/// ```
#[cfg(feature = "serde")]
#[macro_export]
macro_rules! impl_tolua_as_serde {
    ($t:ty) => {
        impl $crate::prelude::ToLua for $t {
            fn to_lua<'a>(
                self,
                lua: &'a $crate::prelude::LuaState,
            ) -> $crate::prelude::LuaResult<$crate::prelude::ValRef<'a>> {
                $crate::prelude::ToLua::to_lua($crate::serde::SerdeValue(self), lua)
            }
        }
    };
}

/// Helper macro to `impl FromLua` for serializable types easily
///
/// ```rust
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Default, Deserialize, Serialize)]
/// struct Config {
///     name: String,
///     path: String,
///     timeout: u64,
///     // ...
/// }
///
/// ezlua::impl_fromlua_as_serde!(Config);
///
/// let lua = Lua::with_open_libs();
/// lua.global().set_closure("set_config", |config: Config| {
///     println!("{config:?}");
/// })?;
/// lua.do_string("set_config({name = 'test', path = '/', timeout = 0})", None)?;
/// // will print `Config { name: "test", path: "/", timeout: 0 }`
/// ```
#[cfg(feature = "serde")]
#[macro_export]
macro_rules! impl_fromlua_as_serde {
    ($t:ty) => {
        impl $crate::prelude::FromLua<'_> for $t {
            fn from_lua(
                lua: &$crate::prelude::LuaState,
                val: $crate::prelude::ValRef,
            ) -> $crate::prelude::LuaResult<Self> {
                <$crate::serde::SerdeValue<Self> as $crate::prelude::FromLua>::from_lua(lua, val)
                    .map(|s| s.0)
            }
        }
    };

    (owned $t:ty) => {
        impl $crate::prelude::FromLua<'_> for $t {
            fn from_lua(
                lua: &$crate::prelude::LuaState,
                val: $crate::prelude::ValRef,
            ) -> $crate::prelude::LuaResult<Self> {
                <$crate::serde::SerdeOwnedValue<Self> as $crate::prelude::FromLua>::from_lua(
                    lua, val,
                )
                .map(|s| s.0)
            }
        }
    };
}

/// Helper macro to `impl ToLuaMulti` for types which can convert to another type implemented ToLuaMulti easily
///
/// ```rust
/// use std::process::{Command, ExitStatus};
///
/// struct CmdExitStatus(ExitStatus);
///
/// ezlua::impl_toluamulti! {
///     CmdExitStatus as (bool, Option<i32>): |self| (self.0.success(), self.0.code())
/// }
///
/// lua.global()
///     .set_closure("execute", |cmd: &str, args: MultiRet<&str>| {
///         Command::new(cmd).args(args.0).status().map(CmdExitStatus)
///     })?;
/// lua.do_string("print(execute('ls', '-l'))", None)?; // true 0
/// ```
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

#[macro_export]
macro_rules! impl_fromlua_as_bitflags {
    ($t:ty) => {
        impl $crate::prelude::FromLua<'_> for $t {
            fn from_lua(
                lua: &$crate::prelude::LuaState,
                val: $crate::prelude::ValRef,
            ) -> $crate::prelude::LuaResult<Self> {
                <$crate::marker::BitFlags<Self> as $crate::prelude::FromLua>::from_lua(lua, val)
                    .map(|s| s.0)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_tolua_as_bitflags {
    ($t:ty) => {
        impl $crate::prelude::ToLua for $t {
            fn to_lua<'a>(
                self,
                lua: &'a $crate::prelude::LuaState,
            ) -> $crate::prelude::LuaResult<$crate::prelude::ValRef<'a>> {
                $crate::prelude::ToLua::to_lua($crate::marker::BitFlags(self), lua)
            }
        }
    };
}

#[macro_export]
macro_rules! bitflags_table {
    ($fn:ident => $t:ty) => {
        fn $fn(
            lua: &$crate::prelude::LuaState,
        ) -> $crate::prelude::LuaResult<$crate::prelude::LuaTable> {
            use ::bitflags::Flag;

            let t = lua.new_table()?;
            for flag in <$t as ::bitflags::Flags>::FLAGS {
                t.set(flag.name(), flag.value().bits())?;
                t.set(flag.value().bits(), flag.name())?;
            }
            $crate::prelude::LuaResult::Ok(t)
        }
    };
}

#[macro_export]
macro_rules! enum_table {
    ($fn:ident => $(($t:expr, $v:expr),)+) => {
        fn $fn(
            lua: &$crate::prelude::LuaState,
        ) -> $crate::prelude::LuaResult<$crate::prelude::LuaTable> {
            let t = lua.new_table()?;
            $(t.set($t, $v)?;)+
            $(t.set($v, $t)?;)+
            $crate::prelude::LuaResult::Ok(t)
        }
    };
}
