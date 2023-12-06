use alloc::{boxed::Box, vec};

use crate::{prelude::*, userdata::UserDataTrans};
use core::ops::DerefMut;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};

impl<'a> UserData for BufReader<Box<dyn Read + 'a>> {
    #[cfg(feature = "parking_lot")]
    type Trans = parking_lot::RwLock<Self>;
    #[cfg(not(feature = "parking_lot"))]
    type Trans = core::cell::RefCell<Self>;

    fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
        methods.set_function("lines", |lua, this: OwnedUserdata<Self>| unsafe {
            let u: LuaUserData = lua.arg_val(1).unwrap().try_into()?;
            let result = lua.new_iter(this.0.lines(), MultiRet(u.uservalues()?));
            result
        })?;
        methods.set_function("split", |lua, (OwnedUserdata::<Self>(this), byte)| unsafe {
            let u: LuaUserData = lua.arg_val(1).unwrap().try_into()?;
            let result = lua.new_iter(
                this.split(byte).map(|x| NilError(x.map(LuaBytes))),
                MultiRet(u.uservalues()?),
            );
            result
        })?;
        methods.add_method_mut("read_line", |_, this, ()| {
            let mut line = Default::default();
            this.read_line(&mut line).map(|_| line)
        })?;
        methods.add_method_mut("read_until", |_, this, byte| {
            let mut line = Default::default();
            this.read_until(byte, &mut line).map(|_| LuaBytes(line))
        })?;

        Ok(())
    }
}

#[inline(always)]
pub fn bind_read<'a, R: Read + 'a, T: UserData + AsMut<R> + Into<R>>(
    t: UserdataRegistry<'a, T>,
) -> LuaResult<()>
where
    <T::Trans as UserDataTrans<T>>::Write<'a>: DerefMut<Target = T> + FromLua<'a> + 'a,
{
    t.add_method_mut("read", |_, this, size: Option<_>| match size {
        Some(size) => {
            let mut buf = vec![0u8; size];
            this.as_mut().read(&mut buf).map(|_| LuaBytes(buf))
        }
        None => {
            let mut buf = vec![];
            this.as_mut().read_to_end(&mut buf).map(|_| LuaBytes(buf))
        }
    })?;
    t.add_method_mut("read_exact", |_, this, size| {
        let mut buf = vec![0u8; size];
        this.as_mut().read_exact(&mut buf).map(|_| LuaBytes(buf))
    })?;
    t.set_function("bufreader", |lua, this: OwnedUserdata<T>| {
        let u: LuaUserData = lua.arg_val(1).unwrap().try_into()?;
        let result = lua.new_userdata_with_values(
            BufReader::new(Box::new(this.0.into()) as Box<dyn Read + 'a>),
            MultiRet(u.uservalues()?),
        );
        result
    })?;

    Ok(())
}

#[inline(always)]
pub fn bind_write<'a, W: Write, T: UserData + AsMut<W>>(t: UserdataRegistry<'a, T>) -> LuaResult<()>
where
    <T::Trans as UserDataTrans<T>>::Write<'a>: DerefMut<Target = T> + FromLua<'a> + 'a,
{
    t.add_method_mut("flush", |_, this, ()| this.as_mut().flush())?;
    t.add_method_mut("write", |_, this, buf| this.as_mut().write(buf))?;
    t.add_method_mut("write_all", |_, this, buf| this.as_mut().write_all(buf))?;

    Ok(())
}

#[inline(always)]
pub fn bind_seek<'a, S: Seek, T: UserData + AsMut<S>>(t: UserdataRegistry<'a, T>) -> LuaResult<()>
where
    <T::Trans as UserDataTrans<T>>::Write<'a>: DerefMut<Target = T> + FromLua<'a> + 'a,
{
    t.add_method_mut("seek", |_, this, from| this.as_mut().seek(from))?;
    t.add_method_mut("rewind", |_, this, ()| this.as_mut().rewind())?;
    // t.add_method_mut("stream_len", |_, this, ()| this.as_mut().stream_len())?;

    Ok(())
}

impl<'a> FromLuaMulti<'a> for SeekFrom {
    fn from_lua_multi(lua: &'a LuaState, begin: Index) -> LuaResult<Self> {
        Ok(match <(&'a str, i64)>::from_lua_multi(lua, begin)? {
            ("start", n) => Self::Start(n as _),
            ("end", n) => Self::End(n as _),
            ("current", n) => Self::Current(n as _),
            _ => return Err("invalid SeekFrom").lua_result(),
        })
    }
}
