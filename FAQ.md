
## How does ezlua compare to mlua/rlua, and why might I use it over mlua

Without doubt, mlua is already a relatively mature project, while ezlua is still relatively young. ezlua lacks some check for extreme cases, so its stability is not as good as mlua's.

The advantage of mlua is that it is stable, and it has complete document， and it supports multiple version of lua, while ezlua supports only lua5.4 currently

The advantage of ezlua is **ergonomic** and **efficient**, ant it supports **multi-threaded environment**

- Ergonomic: you can bind existing function directly that args/return types impls ToLua/FromLua, such as some functions in rust std lib, instead of having to write a wrapper like `Fn(lua, args) -> Result<ret>` in mlua
- Efficient: ezlua supports conversion to reference types such as `&[u8]` and `&str`, the access to them in rust is zero-cost. And ezlua impls stack-slots manager for each call invocation, while mlua uses an auxiliary lua stack, so the types conversion between rust and lua in mlua is relatively heavy

To be honest, I suggest you use mlua in formal product projects, but you can use ezlua in experimental projects or if you want to optimize performance or use lua in multi-threaded environments
