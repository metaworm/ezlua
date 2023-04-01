//! Error handling

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use core::fmt::Debug;

use crate::luaapi::Type;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(From)]
pub enum Error {
    Runtime(String),
    #[from(ignore)]
    Memory(String),
    #[from(ignore)]
    Syntax(String),
    #[from(ignore)]
    Gc(String),
    Yield,
    #[from(ignore)]
    Convert(String),
    ConvertFailed,
    Else(Box<dyn Debug + Send + Sync>),
    TypeNotMatch(Type),
}

impl Debug for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Runtime(arg0) => f.write_str(arg0),
            Self::Memory(arg0) => f.debug_tuple("Memory").field(arg0).finish(),
            Self::Syntax(arg0) => f.debug_tuple("Syntax").field(arg0).finish(),
            Self::Gc(arg0) => f.debug_tuple("Gc").field(arg0).finish(),
            Self::Yield => write!(f, "Yield"),
            Self::Convert(arg0) => write!(f, "convert: {arg0}"),
            Self::ConvertFailed => write!(f, "ConvertFailed"),
            Self::Else(arg0) => f.debug_tuple("Else").field(arg0).finish(),
            Self::TypeNotMatch(arg0) => f.debug_tuple("TypeNotMatch").field(arg0).finish(),
        }
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Error {
    pub fn from_debug<D: Debug + Send + Sync + 'static>(dbg: D) -> Self {
        Self::Else(Box::new(dbg))
    }

    pub fn convert<S: Into<String>>(s: S) -> Self {
        Self::Convert(s.into())
    }

    pub fn runtime<S: Into<String>>(s: S) -> Self {
        Self::Runtime(s.into())
    }

    pub fn runtime_debug<E: Debug>(err: E) -> Self {
        Self::runtime(format!("{err:?}"))
    }
}

pub trait ToLuaResult<T, E> {
    fn lua_result(self) -> Result<T>;

    fn lua_debug_result(self) -> Result<T>
    where
        E: Send + Sync + 'static;

    fn convert_error(self) -> Result<T>;
}

impl<T, E: Debug> ToLuaResult<T, E> for core::result::Result<T, E> {
    #[inline(always)]
    fn lua_result(self) -> Result<T> {
        self.map_err(|e| Error::runtime(format!("{e:?}")))
    }

    #[inline(always)]
    fn lua_debug_result(self) -> Result<T>
    where
        E: Send + Sync + 'static,
    {
        self.map_err(Error::from_debug)
    }

    #[inline(always)]
    fn convert_error(self) -> Result<T> {
        self.map_err(|e| Error::convert(format!("{e:?}")))
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
