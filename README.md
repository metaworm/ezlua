
[![crates.io](https://img.shields.io/crates/v/ezlua.svg)](https://crates.io/crates/ezlua)
[![docs.rs](https://docs.rs/ezlua/badge.svg)](https://docs.rs/ezlua)

Ergonomic, efficient and Zero-cost rust bindings to Lua5.4

## Features

- Serialization (serde) support
- Async function bindings support
- Ergonomic binding for functions and userdata methods
- Ergonomic stack values operation, you don't need to pay attention to the stack details
- Efficient: no auxiliary stack, reference type conversion support
- Builtin bindings to rust standard library
- Mutilple thread support
- [ ] nostd support

## Limits

- Nightly rust compiler needed (1.70+)
- Only support lua5.4 currently

## Examples

See [builtin bindings](https://github.com/metaworm/ezlua/tree/master/src/binding) [tests](https://github.com/metaworm/ezlua/tree/master/tests)
