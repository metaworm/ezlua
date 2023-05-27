
[![crates.io](https://img.shields.io/crates/v/ezlua.svg)](https://crates.io/crates/ezlua)
[![docs.rs](https://docs.rs/ezlua/badge.svg)](https://docs.rs/ezlua)

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

### Basic

First, add ezlua to your dependencies in Cargo.toml
```toml
[dependencies]
ezlua = { version = '0.2' }
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

Implement [`ToLua`] trait for your type, and then you can pass it to lua

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

lua.global()
    .set_closure("default_config", Config::default)?;
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

impl ToLua for Config {
    fn to_lua<'a>(self, lua: &'a LuaState) -> LuaResult<ValRef<'a>> {
        SerdeValue(self).to_lua(lua)
    }
}

impl FromLua<'_> for Config {
    fn from_lua(s: &State, val: ValRef) -> Option<Self> {
        SerdeValue::<Self>::from_lua(val).map(|s| s.0)
    }
}

lua.global()
    .set_closure("set_config", |config: Config| {
        // ... set your config
    })?;
```

### Bind custom object (userdata)

### Register your own module

### Multiple thread usage

## Internal