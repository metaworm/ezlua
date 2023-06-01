
[![crates.io](https://img.shields.io/crates/v/ezlua.svg)](https://crates.io/crates/ezlua)
[![docs.rs](https://docs.rs/ezlua/badge.svg)](https://docs.rs/ezlua)
[![Build Status]](https://github.com/metaworm/ezlua/actions)

[ChangeLog] | [FAQ] | [Known issues]

[Build Status]: https://github.com/metaworm/ezlua/workflows/CI/badge.svg
[ChangeLog]: CHANGELOG.md
[FAQ]: FAQ.md
[Known issues]: https://github.com/metaworm/ezlua/labels/known%20issues

Ergonomic, efficient and Zero-cost rust bindings to Lua5.4

## Features

- Serialization (serde) support
- Async function bindings support
- Ergonomic binding for functions and userdata methods
- Ergonomic stack values operation, you don't need to pay attention to the stack details
- Efficient: no auxiliary stack, support reference type conversion
- Builtin bindings to most commonly used rust std functions and types
- Mutilple thread support
- [ ] nostd support

## Limits

- Nightly rust compiler needed (1.70+)
- Only support lua5.4 currently

## Examples

See [builtin bindings](https://github.com/metaworm/ezlua/tree/master/src/binding) [tests](https://github.com/metaworm/ezlua/tree/master/tests)

## Usage

### Feature flags

* `async`: enable async/await support (any executor can be used, eg. [tokio] or [async-std])
* `serde`: add serialization and deserialization support to `ezlua` types using [serde] framework
* `vendored`: build static Lua library from sources during `ezlua` compilation using [lua-src] crates
* `thread` enable the [multiple thread support](#multiple-thread-usage)
* `std`: enable the builtin bindings for rust std functions and types
* `json`: enable the builtin bindings for [serde_json] crate
* `regex`: enable the builtin bindings for [regex] crate

### Basic

First, add ezlua to your dependencies in Cargo.toml
```toml
[dependencies]
ezlua = { version = '0.3' }
```

Then, use ezlua in rust, the code framework like this
```rust
use ezlua::prelude::*;

fn main() -> LuaResult<()> {
    // create a lua VM
    let lua = Lua::with_open_libs();

    // load your lua script and execute it
    lua.do_string(r#"function add(a, b) return a + b end"#, None)?;

    // get function named add from lua global table
    let add = lua.global().get("add")?;

    // call add function and get its result
    let result = add.pcall::<_, u32>((111, 222))?;
    assert_eq!(result, 333);

    // ... for the following code

    Ok(())
}
```

### Bind your function

Of course, you can provide your rust function to lua via ezlua binding, and it's very simple, like this
```rust
lua.global().set("add", lua.new_closure(|a: u32, b: u32| a + b)?)?;
lua.do_string("assert(add(111, 222) == 333)", None)?;
```

And you can bind exists function easily
```rust
let string: LuaTable = lua.global().get("string")?.try_into()?;
string.set_closure("trim", str::trim)?;
string.set_closure("trim_start", str::trim_start)?;
string.set_closure("trim_end", str::trim_end)?;

let os: LuaTable = lua.global().get("os")?.try_into()?;
os.set_closure("mkdir", std::fs::create_dir::<&str>)?;
os.set_closure("mkdirs", std::fs::create_dir_all::<&str>)?;
os.set_closure("rmdir", std::fs::remove_dir::<&str>)?;
os.set_closure("chdir", std::env::set_current_dir::<&str>)?;
os.set_closure("getcwd", std::env::current_dir)?;
os.set_closure("getexe", std::env::current_exe)?;
```

### Bind your type

Implement `ToLua` trait for your type, and then you can pass it to lua

```rust
#[derive(Debug, Default)]
struct Config {
    name: String,
    path: String,
    timeout: u64,
    // ...
}

impl ToLua for Config {
    fn to_lua<'a>(self, lua: &'a LuaState) -> LuaResult<ValRef<'a>> {
        let conf = lua.new_table()?;
        conf.set("name", self.name)?;
        conf.set("path", self.path)?;
        conf.set("timeout", self.timeout)?;
        conf.to_lua(lua)
    }
}

lua.global().set_closure("default_config", Config::default)?;
```

### Simply bindings via serde

Continuing with the example above, you can simply the binding code via serde

```rust
use serde::{Deserialize, Serialize};
use ezlua::serde::SerdeValue;

#[derive(Debug, Default, Deserialize, Serialize)]
struct Config {
    name: String,
    path: String,
    timeout: u64,
    // ...
}

// You can use impl_tolua_as_serde macro to simply this after version v0.3.1
// ezlua::impl_tolua_as_serde!(Config);
impl ToLua for Config {
    fn to_lua<'a>(self, lua: &'a LuaState) -> LuaResult<ValRef<'a>> {
        SerdeValue(self).to_lua(lua)
    }
}

// You can use impl_fromlua_as_serde macro to simply this after version v0.3.1
// ezlua::impl_fromlua_as_serde!(Config);
impl FromLua<'_> for Config {
    fn from_lua(lua: &LuaState, val: ValRef) -> Option<Self> {
        SerdeValue::<Self>::from_lua(lua, val).map(|s| s.0)
    }
}

lua.global().set("DEFAULT_CONFIG", SerdeValue(Config::default()))?;
lua.global()
    .set_closure("set_config", |config: Config| {
        // ... set your config
    })?;
```

### Bind custom object (userdata)

ezlua's userdata binding mechanism is powerful, the following code comes from [std bindings](https://github.com/metaworm/ezlua/tree/master/src/binding/std.rs)

```rust
use std::{fs::Metadata, path::*};

impl UserData for Metadata {
    fn getter(fields: UserdataRegistry<Self>) -> Result<()> {
        fields.set_closure("size", Self::len)?;
        fields.set_closure("modified", Self::modified)?;
        fields.set_closure("created", Self::created)?;
        fields.set_closure("accessed", Self::accessed)?;
        fields.set_closure("readonly", |this: &Self| this.permissions().readonly())?;

        Ok(())
    }

    fn methods(mt: UserdataRegistry<Self>) -> Result<()> {
        mt.set_closure("len", Self::len)?;
        mt.set_closure("is_dir", Self::is_dir)?;
        mt.set_closure("is_file", Self::is_file)?;
        mt.set_closure("is_symlink", Self::is_symlink)?;

        Ok(())
    }
}
```

Types impls the `UserData` trait, ezlua also impls `ToLua` for itself, and impls `FromLua` for its reference
```rust
lua.global().set("path_metadata", Path::metadata)?;
```

Defaultly, types binded as userdata is immutable, if you need mutable reference, you can specific a `UserData::Trans` type, and there is a builtin impl that is `RefCell`, so the mutable binding impls looks like this
```rust
use core::cell::RefCell;
use std::process::{Child, Command, ExitStatus, Stdio};

impl UserData for Child {
    type Trans = RefCell<Self>;

    fn getter(fields: UserdataRegistry<Self>) -> LuaResult<()> {
        fields.add("id", Self::id)?;

        Ok(())
    }

    fn methods(mt: UserdataRegistry<Self>) -> Result<()> {
        mt.add_mut("kill", Self::kill)?;
        mt.add_mut("wait", Self::wait)?;

        mt.add_mut("try_wait", |this: &mut Self| {
            this.try_wait().ok().flatten().ok_or(())
        })?;
    }
}
```

### Register your own module

To register a lua module, you can provide a rust function return a lua table via `LuaState::register_module` method
```rust
lua.register_module("json", ezlua::binding::json::open, false)?;
lua.register_module("path", |lua| {
    let t = lua.new_table()?;

    t.set_closure("dirname", Path::parent)?;
    t.set_closure("exists", Path::exists)?;
    t.set_closure("abspath", std::fs::canonicalize::<&str>)?;
    t.set_closure("isabs", Path::is_absolute)?;
    t.set_closure("isdir", Path::is_dir)?;
    t.set_closure("isfile", Path::is_file)?;
    t.set_closure("issymlink", Path::is_symlink)?;

    return Ok(t);
}, false)?;
```

And then use them in lua
```lua
local json = require 'json'
local path = require 'path'

local dir = path.abspath('.')
assert(json.load(json.dump(dir)) == dir)
```

### Multiple thread usage

To use multiple thread feature in lua, you need to specify the `thread` feature in Cargo.toml, and patch the lua-src crate with [ezlua's custom](https://github.com/metaworm/lua-src-rs)
```toml
[dependencies]
ezlua = { version = '0.3', features = ['thread'] }

[patch.crates-io]
lua-src = { git = "https://github.com/metaworm/lua-src-rs" }
```

And then, register the thread module for lua
```rust
lua.register_module("thread", ezlua::binding::std::thread::init, true)?;
```

And then, use it in lua
```lua
local thread = require 'thread'
local threads = {}
local tt = { n = 0 }
local count = 64
for i = 1, count do
    threads[i] = thread.spawn(function()
        tt.n = tt.n + 1
        -- print(tt.n)
    end)
end

for i, t in ipairs(threads) do
    t:join()
    print('#' .. i .. ' finished')
end
assert(tt.n == count)
```

In addition, you can also start a new thread with the same lua VM
```rust
let co = Coroutine::empty(&lua);
std::thread::spawn(move || {
    let print = co.global().get("print")?;
    print.pcall_void("running lua in another thread")?;

    LuaResult::Ok(())
})
.join()
.unwrap();
```

### Module mode

In a module mode `ezlua` allows to create a compiled Lua module that can be loaded from Lua code using [`require`](https://www.lua.org/manual/5.4/manual.html#pdf-require).

First, disable the default vendored feature, and keep std feature only, and config your crate as a cdylib in `Cargo.toml` :
``` toml
[dependencies]
ezlua = {version = '0.3', default-features = false, features = ['std']}

[lib]
crate-type = ['cdylib']
```

Then, export your `luaopen_` function by using `ezlua::lua_module!` macro, where the first argument is `luaopen_<Your module name>`
``` rust
use ezlua::prelude::*;

ezlua::lua_module!(luaopen_ezluamod, |lua| {
    let module = lua.new_table()?;

    module.set("_VERSION", "0.1.0")?;
    // ... else module functions

    return Ok(module);
});
```

## Internal design

TODO
