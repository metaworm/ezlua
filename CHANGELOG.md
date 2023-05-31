
## v0.3.0

- Remove `FromLua::from_index`, now all use `FromLua::from_lua`
- Remove `ValRef::check_cast`
- `FromLua::from_lua` returns `Result<Self>` instead of `Option<Self>`
- `ValRef::cast` returns `Result<>` instead of `Option<>`
- More soundness
  1. More `check_stack` for preventing assert(aborting) from lua
  2. Non-table and non-userdata access check