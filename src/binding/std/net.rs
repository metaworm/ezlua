use crate::prelude::*;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};

crate::impl_tolua_as_serde!(SocketAddr);
crate::impl_fromlua_as_serde!(SocketAddr);

impl UserData for TcpListener {
    fn getter(fields: UserdataRegistry<Self>) -> LuaResult<()> {
        fields.set_closure("local_addr", Self::local_addr)?;
        fields.set_closure("ttl", Self::ttl)?;
        Ok(())
    }

    fn setter(fields: UserdataRegistry<Self>) -> LuaResult<()> {
        fields.set_closure("nonblocking", Self::set_nonblocking)?;
        fields.set_closure("ttl", Self::set_ttl)?;
        Ok(())
    }

    fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
        methods.set_closure("accept", Self::accept)?;
        Ok(())
    }

    fn metatable(mt: UserdataRegistry<Self>) -> LuaResult<()> {
        mt.set_closure("bind", Self::bind::<std::net::SocketAddr>)?;
        Ok(())
    }
}

impl UserData for TcpStream {
    fn getter(fields: UserdataRegistry<Self>) -> LuaResult<()> {
        fields.set_closure("local_addr", Self::local_addr)?;
        fields.set_closure("peer_addr", Self::peer_addr)?;
        fields.set_closure("read_timeout", Self::read_timeout)?;
        fields.set_closure("write_timeout", Self::write_timeout)?;
        // fields.set_closure("linger", Self::linger)?;
        fields.set_closure("nodelay", Self::nodelay)?;
        fields.set_closure("ttl", Self::ttl)?;
        Ok(())
    }

    fn setter(fields: UserdataRegistry<Self>) -> LuaResult<()> {
        fields.set_closure("read_timeout", Self::set_read_timeout)?;
        fields.set_closure("write_timeout", Self::set_write_timeout)?;
        // fields.set_closure("linger", Self::set_linger)?;
        fields.set_closure("nodelay", Self::set_nodelay)?;
        fields.set_closure("ttl", Self::set_ttl)?;
        fields.set_closure("nonblocking", Self::set_nonblocking)?;
        Ok(())
    }

    fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
        methods.set_closure("shutdown", Self::shutdown)?;
        methods.set_closure("clone", Self::try_clone)?;
        Ok(())
    }
}

impl<'a> FromLua<'a> for Shutdown {
    fn from_lua(lua: &'a LuaState, val: ValRef<'a>) -> LuaResult<Self> {
        Ok(match <&'a str as FromLua>::from_lua(lua, val)? {
            "read" => Self::Read,
            "write" => Self::Write,
            "both" => Self::Both,
            _ => return Err(crate::error::Error::from_debug("invalid shutdown")),
        })
    }
}
