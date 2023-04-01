use alloc::string::*;
use alloc::vec;
use alloc::vec::Vec;
use core::{ops::Range, result::Result as StdResult};

use crate::error::Result;
use crate::marker::ArgRef;
use crate::prelude::*;
use crate::serde::SerdeValue;
use crate::userdata::UserdataRegistry;

impl<T: ToLua> ToLuaMulti for Range<T> {
    fn value_count(&self) -> Option<usize> {
        Some(2)
    }

    fn push_multi(self, s: &LuaState) -> Result<usize> {
        (self.start, self.end).push_multi(s)
    }
}

pub mod path {
    use super::*;
    use std::{fs::Metadata, path::*};

    impl UserData for Metadata {
        fn getter(fields: UserdataRegistry<Self>) -> Result<()> {
            fields.register("size", Self::len)?;
            fields.register("modified", Self::modified)?;
            fields.register("created", Self::created)?;
            fields.register("accessed", Self::accessed)?;
            fields.register("readonly", |this: &Self| this.permissions().readonly())?;

            Ok(())
        }

        fn methods(mt: UserdataRegistry<Self>) -> Result<()> {
            mt.register("len", Self::len)?;
            mt.register("is_dir", Self::is_dir)?;
            mt.register("is_file", Self::is_file)?;
            mt.register("is_symlink", Self::is_symlink)?;

            Ok(())
        }
    }

    pub fn init(s: &LuaState) -> Result<ValRef> {
        let t = s.create_table(0, 8)?;
        t.register("dirname", Path::parent)?;
        t.register("exists", Path::exists)?;
        t.register("abspath", std::fs::canonicalize::<&str>)?;
        t.register("isabs", Path::is_absolute)?;
        t.register("isdir", Path::is_dir)?;
        t.register("isfile", Path::is_file)?;
        t.register("issymlink", Path::is_symlink)?;
        t.register("basename", Path::file_name)?;
        t.register("withext", Path::with_extension::<&str>)?;
        t.register("withfilename", Path::with_file_name::<&str>)?;
        t.register1("split", |_, path: &str| {
            Path::new(path)
                .parent()
                .and_then(Path::to_str)
                .map(|dir| {
                    let name = &path[dir.len()..];
                    (dir, name.trim_start_matches('\\'))
                })
                .ok_or(())
        })?;
        t.register1("splitext", |_, path: &str| {
            Path::new(path)
                .extension()
                .and_then(std::ffi::OsStr::to_str)
                .map(|ext| {
                    let p = &path[..path.len() - ext.len()];
                    (p.trim_end_matches('.'), ext)
                })
                .unwrap_or_else(|| (path, ""))
        })?;
        t.register("copy", std::fs::copy::<&str, &str>)?;
        t.register("rename", std::fs::rename::<&str, &str>)?;
        t.register("removedir", std::fs::remove_dir::<&str>)?;
        t.register("removefile", std::fs::remove_file::<&str>)?;
        // t.register("softlink", std::fs::soft_link::<&str, &str>)?;
        t.register("hardlink", std::fs::hard_link::<&str, &str>)?;
        t.register("readlink", Path::read_link)?;
        t.register("meta", Path::metadata)?;
        t.register("join", |s: &LuaState, path: &Path, args: MultiRet<&str>| {
            let mut buf = path.to_path_buf();
            for n in args.0 {
                buf = buf.join(n);
            }
            buf
        })?;
        t.register("exepath", std::env::current_exe)?;

        Ok(t)
    }

    impl<'a> FromLua<'a> for &'a Path {
        #[inline(always)]
        fn from_lua(s: &'a LuaState, val: ValRef<'a>) -> Option<Self> {
            Path::new(val.to_safe_str()?).into()
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
        fn from_lua(s: &LuaState, val: ValRef) -> Option<Self> {
            Path::new(val.to_str()?).to_path_buf().into()
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

    impl<'a> FromLua<'a> for Duration {
        fn from_lua(s: &'a LuaState, val: ValRef<'a>) -> Option<Self> {
            Some(match val.into_value() {
                Value::Integer(n) => Duration::from_secs(n as _),
                Value::Number(n) => Duration::from_secs_f64(n),
                // TODO: 1s 1ms 1ns
                // Value::Str(_) => todo!(),
                _ => return None,
            })
        }
    }
}

pub mod process {
    use super::*;
    use core::cell::RefCell;
    use std::io::{Read, Write};
    use std::process::{Child, Command, ExitStatus, Stdio};

    enum ReadArg {
        Exact(usize),
        All,
    }

    impl<'a> FromLua<'a> for ReadArg {
        fn from_lua(s: &'a LuaState, val: ValRef<'a>) -> Option<Self> {
            if val.is_integer() {
                Some(Self::Exact(val.cast()?))
            } else {
                match <&str as FromLua>::from_index(s, val.index())? {
                    "a" | "*" | "*a" => Some(Self::All),
                    _ => None,
                }
            }
        }
    }

    impl FromLua<'_> for Stdio {
        fn from_index(s: &LuaState, i: Index) -> Option<Self> {
            Some(match <&str as FromLua>::from_index(s, i)? {
                "pipe" | "piped" => Stdio::piped(),
                "inherit" => Stdio::inherit(),
                "null" | _ => Stdio::null(),
            })
        }
    }

    impl ToLuaMulti for ExitStatus {
        fn push_multi(self, s: &LuaState) -> Result<usize> {
            (self.success(), self.code()).push_multi(s)
        }
    }

    impl UserData for Command {
        type Trans = RefCell<Self>;

        fn methods(mt: UserdataRegistry<Self>) -> Result<()> {
            mt.add_mut("arg", |this: &mut Self, arg: &str| {
                this.arg(arg);
                ArgRef(1)
            })?;
            mt.add_mut("args", |this: &mut Self, arg: SerdeValue<Vec<&str>>| {
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
        type Trans = RefCell<Self>;

        fn getter(fields: UserdataRegistry<Self>) -> LuaResult<()> {
            fields.add("id", Child::id)?;

            Ok(())
        }

        fn methods(mt: UserdataRegistry<Self>) -> Result<()> {
            mt.add_mut("kill", Child::kill)?;
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
            mt.add_mut("write", |this: &mut Self, data: &[u8]| {
                let stdin = this.stdin.as_mut().ok_or("stdin").lua_result()?;
                stdin.write(data).lua_result()
            })?;
            mt.add_mut("read", |this: &mut Self, size: ReadArg| {
                read_std(this.stdout.as_mut().ok_or("stdout").lua_result()?, size).lua_result()
            })?;
            mt.add_mut("read_error", |this: &mut Self, size: ReadArg| {
                read_std(this.stderr.as_mut().ok_or("stderr").lua_result()?, size)
            })?;

            fn read_std(r: &mut dyn Read, size: ReadArg) -> Result<Vec<u8>> {
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
                Ok(buf)
            }

            Ok(())
        }
    }
}

pub fn extend_os(s: &LuaState) -> Result<()> {
    use std::ffi::OsString;
    use std::fs::DirEntry;
    use std::fs::FileType;

    let g = s.global();
    let os = g.get("os")?;
    os.setf(crate::cstr!("path"), path::init(s)?)?;

    os.set("name", std::env::consts::OS)?;
    os.set("arch", std::env::consts::ARCH)?;
    os.set("family", std::env::consts::FAMILY)?;
    os.set("dllextension", std::env::consts::DLL_EXTENSION)?;
    os.set("pointersize", core::mem::size_of::<usize>())?;

    os.register("mkdir", std::fs::create_dir::<&str>)?;
    os.register("mkdirs", std::fs::create_dir_all::<&str>)?;
    os.register("rmdir", std::fs::remove_dir::<&str>)?;

    os.register("chdir", std::env::set_current_dir::<&str>)?;
    os.register("getcwd", std::env::current_dir)?;
    os.register("getexe", std::env::current_exe)?;

    impl ToLua for OsString {
        fn to_lua<'a>(self, s: &'a LuaState) -> LuaResult<ValRef<'a>> {
            self.to_string_lossy().to_lua(s)
        }
    }

    impl ToLua for FileType {
        fn to_lua<'a>(self, s: &'a LuaState) -> LuaResult<ValRef<'a>> {
            let result = s.new_table()?;
            result.set("is_dir", self.is_dir())?;
            result.set("is_file", self.is_file())?;
            result.set("is_symlink", self.is_symlink())?;
            Ok(result)
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

    os.register("read_dir", |path: &str| {
        LuaResult::Ok(StaticIter::from(
            std::fs::read_dir(path).lua_result()?.flatten(),
        ))
    })?;

    os.register("glob", |pattern: &str| {
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

    os.register("env", std::env::var::<&str>)?;
    os.register("putenv", |s: &LuaState, var: &str, val: Option<&str>| {
        if let Some(val) = val {
            std::env::set_var(var, val);
        } else {
            std::env::remove_var(var);
        };
    })?;

    use std::collections::HashMap;
    use std::process::{Command, Stdio};

    fn init_command(arg: ValRef) -> Result<Command> {
        let mut args: SerdeValue<Vec<&str>> = arg.check_cast()?;
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
        args.getopt::<_, SerdeValue<HashMap<&str, &str>>>("env")?
            .map(|v| {
                for (k, val) in v.iter() {
                    cmd.env(k, val);
                }
            });
        Ok(cmd)
    }
    os.register("command", |s: &LuaState, arg: Value| match arg {
        Value::String(cmd) => Ok(Command::new(
            cmd.to_str_lossy().unwrap_or_default().as_ref(),
        )),
        Value::Table(t) => init_command(t),
        _ => Err("string|table").convert_error(),
    })?;
    os.register("spawn_child", |arg| init_command(arg)?.spawn().lua_result())?;

    Ok(())
}

pub fn extend_string(s: &LuaState) -> Result<()> {
    let string = s.global().get("string")?;

    string.register1("to_utf16", |s: &LuaState, t: &str| unsafe {
        let mut r = t.encode_utf16().collect::<Vec<_>>();
        r.push(0);
        s.new_val(core::slice::from_raw_parts(
            r.as_ptr() as *const u8,
            r.len() * 2 - 1,
        ))
    })?;
    string.register("from_utf16", |t: &[u8]| unsafe {
        let u = core::slice::from_raw_parts(t.as_ptr() as *const u16, t.len() / 2);
        String::from_utf16_lossy(u)
    })?;
    string.register("starts_with", str::starts_with::<&str>)?;
    string.register("ends_with", str::ends_with::<&str>)?;
    string.register("equal", |t1: &str, t2: &str, case_sensitive: bool| {
        if case_sensitive {
            t1.eq(t2)
        } else {
            t1.eq_ignore_ascii_case(t2)
        }
    })?;
    string.register("trim", str::trim)?;
    string.register("trim_start", str::trim_start)?;
    string.register("trim_end", str::trim_end)?;

    impl FromLua<'_> for glob::Pattern {
        fn from_lua(s: &LuaState, val: ValRef) -> Option<Self> {
            glob::Pattern::new(val.to_str()?).ok()
        }
    }

    string.register(
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

#[cfg(all(feature = "thread"))]
mod thread {
    use super::*;

    use core::cell::{Ref, RefCell, RefMut};
    #[cfg(not(target_os = "windows"))]
    use std::os::unix::thread::{JoinHandleExt, RawPthread as RawHandle};
    #[cfg(target_os = "windows")]
    use std::os::windows::io::{AsRawHandle, RawHandle};
    use std::sync::*;
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    struct LuaThread {
        handle: RawHandle,
        join: Option<JoinHandle<()>>,
    }

    impl LuaThread {
        #[inline]
        fn get(&self) -> StdResult<&JoinHandle<()>, &'static str> {
            self.join.as_ref().ok_or("thread joined")
        }
    }

    impl UserData for LuaThread {
        const TYPE_NAME: &'static str = "LLuaThread";

        type Trans = RefCell<Self>;

        fn getter(fields: UserdataRegistry<Self>) -> Result<()> {
            fields.register("handle", |this: Ref<Self>| this.handle as usize)?;
            fields.register1("name", |s, this: Ref<Self>| {
                s.new_val(this.get().map(|j| j.thread().name()).lua_result()?)
            })?;
            fields.register("id", |this: Ref<Self>| {
                this.get().map(|j| j.thread().id().as_u64().get())
            })?;

            Ok(())
        }

        fn methods(mt: UserdataRegistry<Self>) -> Result<()> {
            mt.register("join", |mut this: RefMut<Self>| {
                this.join
                    .take()
                    .ok_or("thread joined")
                    .lua_result()?
                    .join()
                    .map_err(LuaError::runtime_debug)
            })?;
            mt.register("unpark", |this: Ref<Self>| {
                this.get().map(|j| j.thread().unpark())
            })?;

            Ok(())
        }
    }

    #[cfg(target_os = "windows")]
    const RAW_NULL: RawHandle = pstd::ptr::null_mut();
    #[cfg(not(target_os = "windows"))]
    const RAW_NULL: RawHandle = 0;

    #[derive(Default, Deref, AsRef)]
    struct LuaMutex(Mutex<()>);
    struct LuaMutexGaurd<'a>(MutexGuard<'a, ()>);

    impl<'a> UserData for LuaMutexGaurd<'a> {
        const TYPE_NAME: &'static str = "LuaMutexGaurd";

        fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
            methods.register("unlock", |this: ValRef| this.close_userdata())?;
            Ok(())
        }
    }

    impl UserData for LuaMutex {
        const TYPE_NAME: &'static str = "LuaMutex";

        fn methods(mt: UserdataRegistry<Self>) -> Result<()> {
            mt.as_deref()
                .add_deref("is_poisoned", Mutex::<()>::is_poisoned)?;
            mt.register1("lock", |s, this: &Self| {
                s.new_userdata_with_values(LuaMutexGaurd(this.0.lock().lua_result()?), [ArgRef(1)])
            })?;
            mt.register1("try_lock", |s, this: &Self| {
                NilError(
                    this.0
                        .try_lock()
                        .lua_result()
                        .map(LuaMutexGaurd)
                        .map(|guard| s.new_userdata_with_values(guard, [ArgRef(1)])),
                )
            })?;

            Ok(())
        }
    }

    #[derive(Default)]
    struct LuaCondVar {
        lock: Mutex<i32>,
        cvar: Condvar,
    }

    impl UserData for LuaCondVar {
        const TYPE_NAME: &'static str = "LuaCondVar";

        fn methods(mt: UserdataRegistry<Self>) -> Result<()> {
            mt.register2("wait", |s: &LuaState, this: &Self, tm: Option<u64>| {
                this.wait(s, tm)
            })?;
            mt.register("notify_one", |s: &LuaState, this: &Self, val: ValRef| {
                this.push_some(s, val).map(|_| this.cvar.notify_one())
            })?;
            mt.register("notify_all", |s: &LuaState, this: &Self, val: ValRef| {
                this.push_some(s, val).map(|_| this.cvar.notify_all())
            })?;

            Ok(())
        }
    }

    impl LuaCondVar {
        fn push_some(&self, s: &LuaState, val: ValRef) -> Result<()> {
            let mut i = self.lock.lock().lua_result()?;
            let creg = s.registry();
            creg.unreference((*i).into());
            *i = creg.reference(val)?.0;

            Ok(())
        }

        fn wait<'a>(&self, s: &'a LuaState, timeout: Option<u64>) -> Result<ValRef<'a>> {
            let lock = &self.lock;
            let cvar = &self.cvar;
            Ok(if let Some(tm) = timeout {
                let (i, r) = cvar
                    .wait_timeout(lock.lock().lua_result()?, Duration::from_millis(tm))
                    .map_err(LuaError::runtime_debug)?;
                if r.timed_out() {
                    return s.new_val(());
                }
                s.registry().raw_geti((*i) as i64)
            } else {
                let i = cvar
                    .wait(lock.lock().lua_result()?)
                    .map_err(LuaError::runtime_debug)?;
                s.registry().raw_geti((*i) as i64)
            })
        }
    }

    pub fn init(s: &LuaState) -> Result<()> {
        let t = s.create_table(0, 4)?;
        t.register("spawn", |routine: Coroutine, name: Option<&str>| {
            thread::Builder::new()
                .name(name.unwrap_or("<lua>").into())
                .spawn(move || {
                    if let Err(err) = routine.val(1).pcall::<_, ()>(()) {
                        call_print(
                            &routine,
                            &std::format!(
                                "<thread#{} \"{}\"> {err:?}",
                                thread::current().id().as_u64().get(),
                                thread::current().name().unwrap_or_default(),
                            ),
                        );
                    }
                })
                .map(|join| {
                    #[cfg(target_os = "windows")]
                    let handle = join.as_raw_handle();
                    #[cfg(not(target_os = "windows"))]
                    let handle = join.as_pthread_t();

                    LuaThread {
                        join: Some(join),
                        handle,
                    }
                })
        })?;
        t.register("sleep", |time: u64| {
            thread::sleep(Duration::from_millis(time))
        })?;
        t.register("park", thread::park)?;
        t.register("id", || thread::current().id().as_u64().get())?;
        t.register0("name", |s: &LuaState| s.new_val(thread::current().name()))?;
        t.register("yield_now", thread::yield_now)?;
        t.register("mutex", LuaMutex::default)?;
        t.register("condvar", LuaCondVar::default)?;

        s.global().set("thread", t)?;

        Ok(())
    }
}

pub fn init_global(s: &LuaState) -> Result<()> {
    extend_os(s)?;
    extend_string(s)?;
    #[cfg(feature = "thread")]
    thread::init(s)?;

    let g = s.global();
    g.register("readfile", |path: &str| {
        NilError(std::fs::read(path).map(serde_bytes::ByteBuf::from))
    })?;
    g.set(
        "__file__",
        s.new_closure(|s: &LuaState| {
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
    g.register("writefile", std::fs::write::<&std::path::Path, &[u8]>)?;

    Ok(())
}

pub fn call_print(s: &LuaState, err: &str) {
    let g = s.global();
    let error = g.getf(crate::cstr!("__elua_error"));
    let print = g.getf(crate::cstr!("print"));
    if error.type_of() == LuaType::Function {
        error.pcall_void(err).ok();
    } else if print.type_of() != LuaType::Function {
        print.pcall_void(err).ok();
    } else {
        std::eprintln!("[callback error] {}", err);
    }
}
