
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