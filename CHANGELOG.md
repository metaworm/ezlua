## v0.5.0

- Improve the support to no-std
- Improve the implementation of `new_iter_map`
- Added new marker `ScopeUserdata` `FromStr`
- Added new macros `impl_fromlua_as_bitflags` `impl_tolua_as_bitflags` `bitflags_table` `enum_table`
- Added bindings `OsStr`/`OsString`
- Added bindings to `std::io` module
- Added bindings to `chrono` crate
- Change the `refs` parameter of `new_userdata_with_values`/`new_iter`/`new_iter_map` to any `FromLuaMulti`
- Adapted macro syntax to rustc 1.76
- More strictly type checking and bugfix

## v0.4.4

- Improve builtin bindings
- Improve the implementation to async bindings
- Added `ValRef::tostring`, which corresponds to the tostring function in lua
- Added `LuaState::bind_async`, which can bind an async block as a lua userdata, and release the ownership after one call
- Added bindings to `bytes` crate
- Added bindings to `TcpListener` `TcpStream`

## v0.4.3

- Bugfix: memory leak
- Improve builtin bindings

## v0.4.2

- Bugfix for serialization to enum type
- Removed `Coroutine::call_async`

## v0.4.1

- Added [tokio bindings](https://ezlua-types.vercel.app/modules/tokio.html)
- Bugfix:
  - failed to catch error when calling async function with pcall
  - `__index` is overwritten when init userdata's metatable 

## v0.4.0

- Bugfix for async function and userdata bindings
- Improve bindings to rust thread
- Change binding for rust iterator to userdata instead of closure

## v0.3.2

- Added `compat` feature, impls a simple compatibility layer to mlua/rlua
- Added impls `Add/Sub/Mul/Div/...` for `ValRef`, which you can perform arithmetic operations in rust, just like in lua

## v0.3.1

- Added `impl_tolua_as_serde!`/`impl_fromlua_as_serde!` macro to `impl ToLua`/`impl FromLua` for serializable types easily
- Added `impl_toluamulti!` macro to `impl ToLuaMulti` for types which can convert to another type implemented ToLuaMulti easily
- Removed dependence to the unstable features fn_traits/tuple_trait
- Make `serde` as an optional feature

## v0.3.0

- Removed `FromLua::from_index`, now all use `FromLua::from_lua`
- Removed `ValRef::check_cast`
- `FromLua::from_lua` returns `Result<Self>` instead of `Option<Self>`
- `ValRef::cast` returns `Result<>` instead of `Option<>`
- More soundness
  1. More `check_stack` for preventing assert(aborting) from lua
  2. Non-table and non-userdata access check

## v0.2.2

- Added `lua_module!` macro to create lua module