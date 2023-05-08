
Ergonomic, efficient and Zero-cost rust bindings to Lua5.4

## Features

- Serialization (serde) support
- Async function bindings support
- Ergonomic binding for functions and userdata methods
- Ergonomic stack values operation, you don't need to pay attention to the stack details
- Efficient: no auxiliary stack
- Builtin bindings to rust standard library
- [ ] nostd support

## Limits

- Nightly rust compiler needed (1.70+)
- Only support lua5.4 currently

## Examples

See [tests](https://github.com/metaworm/ezlua/tree/master/tests)