use alloc::string::*;
use alloc::vec;
use alloc::vec::Vec;
use core::{ops::Range, result::Result as StdResult};

use crate::error::Result;
use crate::marker::ArgRef;
use crate::prelude::*;
use crate::userdata::UserdataRegistry;

pub mod io;
pub mod net;
#[cfg(all(feature = "thread"))]
pub mod thread;

impl<T: ToLua> ToLuaMulti for Range<T> {
    const VALUE_COUNT: Option<usize> = Some(2);

    fn push_multi(self, s: &LuaState) -> Result<usize> {
        (self.start, self.end).push_multi(s)
    }
}

pub mod path {
    use super::*;
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

    pub fn init(s: &LuaState) -> Result<LuaTable> {
        let t = s.new_table_with_size(0, 8)?;
        t.set_closure("dirname", Path::parent)?;
        t.set_closure("exists", Path::exists)?;
        t.set_closure("abspath", std::fs::canonicalize::<&str>)?;
        t.set_closure("isabs", Path::is_absolute)?;
        t.set_closure("isdir", Path::is_dir)?;
        t.set_closure("isfile", Path::is_file)?;
        t.set_closure("issymlink", Path::is_symlink)?;
        t.set_closure("basename", Path::file_name)?;
        t.set_closure("withext", Path::with_extension::<&str>)?;
        t.set_closure("withfilename", Path::with_file_name::<&str>)?;
        t.set(
            "split",
            s.new_closure1(|_, path: &str| {
                Path::new(path)
                    .parent()
                    .and_then(Path::to_str)
                    .map(|dir| {
                        let name = &path[dir.len()..];
                        (dir, name.trim_start_matches('\\'))
                    })
                    .ok_or(())
            })?,
        )?;
        t.set(
            "splitext",
            s.new_closure1(|_, path: &str| {
                Path::new(path)
                    .extension()
                    .and_then(std::ffi::OsStr::to_str)
                    .map(|ext| {
                        let p = &path[..path.len() - ext.len()];
                        (p.trim_end_matches('.'), ext)
                    })
                    .unwrap_or_else(|| (path, ""))
            })?,
        )?;
        t.set_closure("copy", std::fs::copy::<&str, &str>)?;
        t.set_closure("rename", std::fs::rename::<&str, &str>)?;
        t.set_closure("removedir", std::fs::remove_dir::<&str>)?;
        t.set_closure("removefile", std::fs::remove_file::<&str>)?;
        // t.set_closure("softlink", std::fs::soft_link::<&str, &str>)?;
        t.set_closure("hardlink", std::fs::hard_link::<&str, &str>)?;
        t.set_closure("readlink", Path::read_link)?;
        t.set_closure("metadata", Path::metadata)?;
        t.set_closure("join", |s: &LuaState, path: &Path, args: MultiRet<&str>| {
            let mut buf = path.to_path_buf();
            for n in args.0 {
                buf = buf.join(n);
            }
            buf
        })?;
        t.set_closure("exepath", std::env::current_exe)?;

        Ok(t)
    }

    impl<'a> FromLua<'a> for &'a Path {
        #[inline(always)]
        fn from_lua(lua: &'a LuaState, val: ValRef<'a>) -> Result<Self> {
            Ok(Path::new(<&str as FromLua>::from_lua(lua, val)?))
        }
    }

    impl ToLua for &Path {
        #[inline(always)]
        fn to_lua<'a>(self, s: &'a LuaState) -> Result<ValRef<'a>> {
            let p = self.to_string_lossy();
            p.strip_prefix(r"\\?\").unwrap_or(&p).to_lua(s)
        }
    }

    impl ToLua for PathBuf {
        #[inline(always)]
        fn to_lua<'a>(self, s: &'a LuaState) -> Result<ValRef<'a>> {
            ToLua::to_lua(self.as_path(), s)
        }
    }

    impl FromLua<'_> for PathBuf {
        #[inline(always)]
        fn from_lua(s: &LuaState, val: ValRef) -> Result<Self> {
            Ok(Path::new(
                val.to_str()
                    .ok_or_else(|| LuaError::TypeNotMatch(val.type_of()))?,
            )
            .to_path_buf())
        }
    }
}

pub mod time {
    use super::*;
    use std::time::*;

    impl ToLua for SystemTime {
        fn to_lua<'a>(self, s: &'a LuaState) -> Result<ValRef<'a>> {
            self.duration_since(UNIX_EPOCH)
                .ok()
                .map(|dur| dur.as_secs_f64())
                .to_lua(s)
        }
    }

    impl ToLua for Duration {
        fn to_lua<'a>(self, s: &'a LuaState) -> LuaResult<ValRef<'a>> {
            self.as_secs_f64().to_lua(s)
        }
    }

    impl<'a> FromLua<'a> for Duration {
        fn from_lua(_: &'a LuaState, val: ValRef<'a>) -> Result<Self> {
            let ty = val.type_of();
            Ok(match val.into_value() {
                LuaValue::Integer(n) => Duration::from_secs(n as _),
                LuaValue::Number(n) => Duration::from_secs_f64(n),
                // TODO: 1s 1ms 1ns
                // LuaValue::Str(_) => todo!(),
                _ => return Err(LuaError::TypeNotMatch(ty)),
            })
        }
    }

    pub fn init(lua: &LuaState) -> LuaResult<LuaTable> {
        let t = lua.new_table()?;

        t.set_closure("now", SystemTime::now)?;
        t.set_closure("ms", Duration::from_millis)?;
        t.set_closure("ns", Duration::from_nanos)?;

        Ok(t)
    }
}

pub mod process {
    use super::*;
    use core::cell::RefCell;
    use std::{
        io::Read,
        process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio},
    };

    enum ReadArg {
        Exact(usize),
        All,
    }

    impl<'a> FromLua<'a> for ReadArg {
        fn from_lua(lua: &'a LuaState, val: ValRef<'a>) -> Result<Self> {
            if val.is_integer() {
                Ok(Self::Exact(val.cast()?))
            } else {
                match <&str as FromLua>::from_lua(lua, val)? {
                    "a" | "*" | "*a" => Ok(Self::All),
                    _ => Err("invalid read arg").lua_result(),
                }
            }
        }
    }

    impl FromLua<'_> for Stdio {
        fn from_lua(lua: &LuaState, val: ValRef) -> Result<Self> {
            Ok(match <&str as FromLua>::from_lua(lua, val)? {
                "pipe" | "piped" => Stdio::piped(),
                "inherit" => Stdio::inherit(),
                "null" | _ => Stdio::null(),
            })
        }
    }

    crate::impl_toluamulti!(ExitStatus as (bool, Option<i32>): |self| (self.success(), self.code()));

    #[derive(derive_more::AsMut, derive_more::Into)]
    struct LuaStdin(ChildStdin);

    impl UserData for LuaStdin {
        type Trans = RefCell<Self>;

        fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
            self::io::bind_write(methods)
        }
    }

    #[derive(derive_more::AsMut, derive_more::Into)]
    struct LuaStdout(ChildStdout);

    impl UserData for LuaStdout {
        type Trans = RefCell<Self>;

        fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
            self::io::bind_read(methods)
        }
    }

    #[derive(derive_more::AsMut, derive_more::Into)]
    struct LuaStderr(ChildStderr);

    impl UserData for LuaStderr {
        type Trans = RefCell<Self>;

        fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
            self::io::bind_read(methods)
        }
    }

    impl UserData for Command {
        #[cfg(feature = "parking_lot")]
        type Trans = parking_lot::RwLock<Self>;
        #[cfg(not(feature = "parking_lot"))]
        type Trans = RefCell<Self>;

        fn methods(mt: UserdataRegistry<Self>) -> Result<()> {
            mt.add_mut("arg", |this: &mut Self, arg: &str| {
                this.arg(arg);
                ArgRef(1)
            })?;
            mt.add_mut("args", |this: &mut Self, arg: Vec<String>| {
                this.args(arg.as_slice());
                ArgRef(1)
            })?;
            mt.add_mut("current_dir", |this: &mut Self, arg: &str| {
                this.current_dir(arg);
                ArgRef(1)
            })?;
            mt.add_mut("env_clear", |this: &mut Self| {
                this.env_clear();
                ArgRef(1)
            })?;
            mt.add_mut("stdin", |this: &mut Self, arg: Stdio| {
                this.stdin(arg);
                ArgRef(1)
            })?;
            mt.add_mut("stdout", |this: &mut Self, arg: Stdio| {
                this.stdout(arg);
                ArgRef(1)
            })?;
            mt.add_mut("stderr", |this: &mut Self, arg: Stdio| {
                this.stderr(arg);
                ArgRef(1)
            })?;
            mt.add_mut("env", |this: &mut Self, k: &str, v: Option<&str>| {
                if let Some(v) = v {
                    this.env(k, v);
                } else {
                    this.env_remove(k);
                }
                ArgRef(1)
            })?;
            mt.add_mut("spawn", |this: &mut Self| this.spawn())?;

            Ok(())
        }
    }

    impl UserData for Child {
        #[cfg(feature = "parking_lot")]
        type Trans = parking_lot::RwLock<Self>;
        #[cfg(not(feature = "parking_lot"))]
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
            mt.add_mut("wait_output", |this: &mut Self| {
                this.wait().map(|status| {
                    (
                        this.stdout
                            .as_mut()
                            .and_then(|o| read_std(o, ReadArg::All).ok()),
                        this.stderr
                            .as_mut()
                            .and_then(|o| read_std(o, ReadArg::All).ok()),
                        status.success(),
                        status.code(),
                    )
                })
            })?;
            mt.add_mut("take_stdin", |this: &mut Self| {
                this.stdin.take().map(LuaStdin)
            })?;
            mt.add_mut("take_stdout", |this: &mut Self| {
                this.stdout.take().map(LuaStdout)
            })?;
            mt.add_mut("take_stderr", |this: &mut Self| {
                this.stderr.take().map(LuaStderr)
            })?;

            fn read_std(r: &mut dyn Read, size: ReadArg) -> Result<LuaBytes> {
                let mut buf = vec![];
                match size {
                    ReadArg::All => {
                        r.read_to_end(&mut buf).lua_result()?;
                    }
                    ReadArg::Exact(size) => {
                        buf.resize(size, 0);
                        let len = r.read(buf.as_mut()).lua_result()?;
                        buf.resize(len, 0);
                    }
                }
                Ok(LuaBytes(buf))
            }

            Ok(())
        }
    }
}

pub fn extend_os(s: &LuaState) -> Result<()> {
    use std::fs::DirEntry;
    use std::fs::FileType;

    let g = s.global();
    let os: LuaTable = g.get("os")?.try_into()?;
    os.setf(crate::cstr!("path"), path::init(s)?)?;

    os.set("name", std::env::consts::OS)?;
    os.set("arch", std::env::consts::ARCH)?;
    os.set("family", std::env::consts::FAMILY)?;
    os.set("dllextension", std::env::consts::DLL_EXTENSION)?;
    os.set("pointersize", core::mem::size_of::<usize>())?;

    os.set_closure("mkdir", std::fs::create_dir::<&str>)?;
    os.set_closure("mkdirs", std::fs::create_dir_all::<&str>)?;
    os.set_closure("rmdir", std::fs::remove_dir::<&str>)?;

    os.set_closure("chdir", std::env::set_current_dir::<&str>)?;
    os.set_closure("getcwd", std::env::current_dir)?;
    os.set_closure("getexe", std::env::current_exe)?;

    impl ToLua for FileType {
        fn to_lua<'a>(self, s: &'a LuaState) -> LuaResult<ValRef<'a>> {
            let result = s.new_table()?;
            result.set("is_dir", self.is_dir())?;
            result.set("is_file", self.is_file())?;
            result.set("is_symlink", self.is_symlink())?;
            result.to_lua(s)
        }
    }

    impl UserData for DirEntry {
        fn getter(fields: UserdataRegistry<Self>) -> LuaResult<()> {
            fields.add("path", Self::path)?;
            fields.add("metadata", Self::metadata)?;
            fields.add("file_name", Self::file_name)?;
            fields.add("file_type", Self::file_type)?;

            Ok(())
        }

        fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
            Ok(())
        }
    }

    os.set_closure("read_dir", |path: &str| {
        LuaResult::Ok(StaticIter::from(
            std::fs::read_dir(path).lua_result()?.flatten(),
        ))
    })?;

    os.set_closure("glob", |pattern: &str| {
        use glob::MatchOptions;

        glob::glob_with(
            pattern,
            MatchOptions {
                case_sensitive: false,
                ..MatchOptions::new()
            },
        )
        .map(|iter| StaticIter::from(iter.filter_map(StdResult::ok)))
    })?;

    os.set_closure("env", |var: &str| std::env::var(var).ok())?;
    os.set_closure("putenv", |var: &str, val: Option<&str>| {
        if let Some(val) = val {
            std::env::set_var(var, val);
        } else {
            std::env::remove_var(var);
        };
    })?;

    use std::collections::HashMap;
    use std::process::{Command, Stdio};

    fn init_command(arg: LuaTable) -> Result<Command> {
        let mut args: Vec<String> = arg.cast()?;
        if args.is_empty() {
            Err("empty command").lua_result()?;
        }
        let mut cmd = Command::new(args.remove(0));
        cmd.args(args.as_slice());
        let args = arg;
        args.getopt::<_, Stdio>("stdin")?.map(|v| cmd.stdin(v));
        args.getopt::<_, Stdio>("stdout")?.map(|v| cmd.stdout(v));
        args.getopt::<_, Stdio>("stderr")?.map(|v| cmd.stderr(v));
        args.getopt::<_, &str>("cwd")?.map(|v| cmd.current_dir(v));
        args.getopt::<_, HashMap<String, String>>("env")?.map(|v| {
            for (k, val) in v.into_iter() {
                cmd.env(k, val);
            }
        });
        Ok(cmd)
    }
    os.set_closure("command", |s: &LuaState, arg: LuaValue| match arg {
        LuaValue::String(cmd) => Ok(Command::new(cmd.to_string_lossy().as_ref())),
        LuaValue::Table(t) => init_command(t),
        _ => Err("string|table").convert_error(),
    })?;
    os.set_closure("spawn_child", |arg| init_command(arg)?.spawn().lua_result())?;

    Ok(())
}

pub fn extend_string(s: &LuaState) -> Result<()> {
    let string: LuaTable = s.global().get("string")?.try_into()?;

    string.set(
        "to_utf16",
        s.new_closure1(|s: &LuaState, t: &str| unsafe {
            let mut r = t.encode_utf16().collect::<Vec<_>>();
            r.push(0);
            s.new_val(core::slice::from_raw_parts(
                r.as_ptr() as *const u8,
                r.len() * 2 - 1,
            ))
        })?,
    )?;
    string.set(
        "from_utf16",
        s.new_closure1(|_, t: &[u8]| unsafe {
            let u = core::slice::from_raw_parts(t.as_ptr() as *const u16, t.len() / 2);
            String::from_utf16_lossy(u)
        })?,
    )?;
    string.set_closure("starts_with", str::starts_with::<&str>)?;
    string.set_closure("ends_with", str::ends_with::<&str>)?;
    string.set_closure("equal", |t1: &str, t2: &str, case_sensitive: bool| {
        if case_sensitive {
            t1.eq(t2)
        } else {
            t1.eq_ignore_ascii_case(t2)
        }
    })?;
    string.set_closure("trim", str::trim)?;
    string.set_closure("trim_start", str::trim_start)?;
    string.set_closure("trim_end", str::trim_end)?;

    impl FromLua<'_> for glob::Pattern {
        fn from_lua(lua: &LuaState, val: ValRef) -> Result<Self> {
            glob::Pattern::new(<&str as FromLua>::from_lua(lua, val)?).lua_result()
        }
    }

    string.set_closure(
        "wildmatch",
        |t1: &str, pattern: glob::Pattern, case_sensitive: bool| {
            let options = glob::MatchOptions {
                case_sensitive,
                ..Default::default()
            };
            pattern.matches_with(t1, options)
        },
    )?;

    Ok(())
}

pub fn init_global(lua: &LuaState) -> Result<()> {
    extend_os(lua)?;
    extend_string(lua)?;
    #[cfg(feature = "thread")]
    lua.register_module("thread", thread::init, true)?;

    let g = lua.global();
    g.set_closure("readfile", |path: &str| {
        NilError(std::fs::read(path).map(LuaBytes))
    })?;
    g.set(
        "__file__",
        lua.new_closure(|s: &LuaState| {
            use crate::luaapi::UnsafeLuaApi;

            s.get_stack(1).and_then(|mut dbg| {
                s.get_info(crate::cstr!("S"), &mut dbg);
                if dbg.source.is_null() {
                    return None;
                }
                let src = unsafe { std::ffi::CStr::from_ptr(dbg.source) };
                let src = src.to_string_lossy();
                Some(src.strip_prefix("@").unwrap_or(&src).to_string())
            })
        })?,
    )?;
    g.set_closure("writefile", std::fs::write::<&std::path::Path, &[u8]>)?;
    g.set_closure("closeobject", ValRef::close_and_remove_metatable)?;
    g.set(
        "array",
        lua.new_function(|lua, table: ValRef| {
            table.set_metatable(lua.array_metatable()?).map(|_| table)
        })?,
    )?;

    Ok(())
}
