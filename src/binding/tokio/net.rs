use super::*;
use tokio::net::{TcpListener, TcpStream};

impl UserData for TcpListener {
    fn getter(fields: UserdataRegistry<Self>) -> LuaResult<()> {
        fields.set_closure("local_addr", Self::local_addr)?;
        Ok(())
    }

    fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
        methods.set_async_closure("accept", Self::accept)?;
        Ok(())
    }

    fn metatable(mt: UserdataRegistry<Self>) -> LuaResult<()> {
        mt.set_async_closure("bind", Self::bind::<std::net::SocketAddr>)?;
        Ok(())
    }
}

impl UserData for TcpStream {
    fn getter(fields: UserdataRegistry<Self>) -> LuaResult<()> {
        fields.set_closure("local_addr", Self::local_addr)?;
        fields.set_closure("peer_addr", Self::peer_addr)?;
        Ok(())
    }

    fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
        Ok(())
    }
}

pub fn init(lua: &LuaState) -> LuaResult<LuaTable> {
    let m = lua.new_table()?;

    m.set("TcpListner", lua.register_usertype::<TcpListener>()?)?;
    m.set("TcpStream", lua.register_usertype::<TcpStream>()?)?;

    Ok(m)
}
