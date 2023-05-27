use crate::prelude::*;
use alloc::string::String;
use std::{
    fs::{self, FileTimes, Metadata},
    path::Path,
    time::{Duration, SystemTime},
};

pub fn open(lua: &LuaState) -> LuaResult<LuaTable> {
    let module = lua.new_table()?;

    module.set_closure("chdir", std::env::set_current_dir::<&str>)?;
    module.set_closure("currentdir", std::env::current_dir)?;

    module.set_closure("dir", |dir: &str| {
        fs::read_dir(dir).map(|iter| StaticIter::new(iter.flatten().map(|e| e.file_name())))
    })?;

    module.set_closure("link", |old: &str, new: &str, symbol: Option<bool>| {
        #[allow(deprecated)]
        if symbol.unwrap_or(false) {
            fs::soft_link(old, new)
        } else {
            fs::hard_link(old, new)
        }
        .lua_result()
    })?;

    module.set_closure("readlink", |path: &str| NilError(std::fs::read_link(path)))?;
    module.set_closure("copy", std::fs::copy::<&str, &str>)?;
    module.set_closure("rename", std::fs::rename::<&str, &str>)?;
    module.set_closure("removedir", std::fs::remove_dir::<&str>)?;
    module.set_closure("remove", std::fs::remove_file::<&str>)?;

    module.set_closure("mkdir", std::fs::create_dir::<&str>)?;
    module.set_closure("mkdirs", std::fs::create_dir_all::<&str>)?;
    module.set_closure("rmdir", std::fs::remove_dir::<&str>)?;
    module.set_closure("rmdir_all", std::fs::remove_dir_all::<&str>)?;

    module.set(
        "attributes",
        lua.new_closure2(|lua, path: &Path, arg2: Option<LuaValue>| {
            if !path.exists() {
                return LuaResult::Ok(LuaValue::Nil);
            }

            let arg2 = arg2.unwrap_or(LuaValue::Nil);
            let mut res = None;
            let mut key = None;
            match arg2 {
                LuaValue::Nil => {}
                LuaValue::Table(t) => {
                    res.replace(t);
                }
                _ => {
                    key.replace(arg2);
                }
            }
            let meta = fs::metadata(path).lua_result()?;
            let res = lua_attribute(lua, res, path, meta)?;
            if let Some(key) = key {
                res.get(key).map(ValRef::into_value)
            } else {
                Ok(LuaValue::Table(res))
            }
        })?,
    )?;

    module.set(
        "symlinkattributes",
        lua.new_closure(|lua, path: &Path, arg2: Option<LuaValue>| {
            if !path.exists() {
                return LuaResult::Ok(LuaValue::Nil);
            }

            let meta = fs::symlink_metadata(path).lua_result()?;
            let res = lua_attribute(lua, None, path, meta)?;
            res.set(
                "target",
                fs::read_link(path)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            )?;

            if let Some(key) = arg2 {
                res.get(key).map(ValRef::into_value)
            } else {
                LuaResult::Ok(LuaValue::Table(res))
            }
        })?,
    )?;

    module.set_closure("touch", |file: &str, t: Option<f64>, a: Option<f64>| {
        let file = fs::File::open(file).lua_result()?;
        file.set_times(
            FileTimes::new()
                .set_modified(
                    t.map(|t| SystemTime::UNIX_EPOCH + Duration::from_secs_f64(t))
                        .unwrap_or(SystemTime::now()),
                )
                .set_accessed(
                    a.map(|t| SystemTime::UNIX_EPOCH + Duration::from_secs_f64(t))
                        .unwrap_or(SystemTime::now()),
                ),
        )
        .lua_result()
    })?;

    Ok(module)
}

fn lua_attribute<'a>(
    lua: &'a LuaState,
    res: Option<LuaTable<'a>>,
    path: &Path,
    meta: Metadata,
) -> LuaResult<LuaTable<'a>> {
    let res = if let Some(res) = res {
        res
    } else {
        lua.new_table()?
    };
    res.set(
        "access",
        meta.accessed()
            .lua_result()?
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|x| x.as_secs())
            .unwrap_or(0),
    )?;
    res.set(
        "modification",
        meta.modified()
            .lua_result()?
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|x| x.as_secs())
            .unwrap_or(0),
    )?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        res.set("uid", meta.uid())?;
        res.set("gid", meta.gid())?;
        res.set("dev", meta.dev())?;
        res.set("ino", meta.ino())?;
        res.set("rdev", meta.rdev())?;
        res.set("blocks", meta.blocks())?;
    }

    #[cfg(windows)]
    {
        res.set(
            "created",
            meta.created()
                .lua_result()?
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|x| x.as_secs())
                .unwrap_or(0),
        )?;
    }

    res.set("readonly", meta.permissions().readonly())?;

    res.set("size", meta.len())?;

    let mut perms = [b'-'; 9];
    let mode;
    #[cfg(windows)]
    unsafe {
        use std::os::windows::ffi::OsStrExt;

        let mut path = path.as_os_str().encode_wide().collect::<Vec<_>>();
        path.push(0);
        let mut st = std::mem::zeroed();
        mode = if libc::wstat(path.as_ptr(), &mut st) == 0 {
            st.st_mode as i32
        } else {
            0
        };

        if mode & libc::S_IREAD != 0 {
            perms[0] = b'r';
            perms[3] = b'r';
            perms[6] = b'r';
        }
        if mode & libc::S_IWRITE != 0 {
            perms[1] = b'w';
            perms[4] = b'w';
            perms[7] = b'w';
        }
        if mode & libc::S_IEXEC != 0 {
            perms[2] = b'x';
            perms[5] = b'x';
            perms[8] = b'x';
        }
    };
    #[cfg(unix)]
    let st_mode = {
        use std::os::unix::fs::PermissionsExt;

        mode = meta.permissions().mode() as libc::mode_t;
        if mode & libc::S_IRUSR != 0 {
            perms[0] = b'r';
        }
        if mode & libc::S_IWUSR != 0 {
            perms[1] = b'w';
        }
        if mode & libc::S_IXUSR != 0 {
            perms[2] = b'x';
        }
        if mode & libc::S_IRGRP != 0 {
            perms[3] = b'r';
        }
        if mode & libc::S_IWGRP != 0 {
            perms[4] = b'w';
        }
        if mode & libc::S_IXGRP != 0 {
            perms[5] = b'x';
        }
        if mode & libc::S_IROTH != 0 {
            perms[6] = b'r';
        }
        if mode & libc::S_IWOTH != 0 {
            perms[7] = b'w';
        }
        if mode & libc::S_IXOTH != 0 {
            perms[8] = b'x';
        }
    };
    res.set("st_mode", mode)?;
    res.set("permissions", String::from_utf8_lossy(&perms[..]))?;

    if meta.file_type().is_dir() {
        res.set("mode", "directory")?;
    }
    if meta.file_type().is_file() {
        res.set("mode", "file")?;
    }
    if meta.file_type().is_symlink() {
        res.set("mode", "link")?;
    }

    Ok(res)
}
