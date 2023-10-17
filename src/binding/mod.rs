//! Builtin bindings for rust std and some third-party lib

use crate::{error::Result, state::State};

#[cfg(feature = "std")]
pub mod fs;
#[cfg(feature = "json")]
pub mod json;
#[cfg(feature = "log")]
pub mod log;
#[cfg(feature = "regex")]
pub mod regex;
#[cfg(feature = "std")]
pub mod std;
#[cfg(feature = "tokio")]
pub mod tokio;

pub fn init_global(s: &State) -> Result<()> {
    #[cfg(feature = "std")]
    self::std::init_global(s)?;
    #[cfg(feature = "log")]
    s.register_module("log", log::open, false)?;
    #[cfg(feature = "regex")]
    s.register_module("regex", regex::open, false)?;
    #[cfg(feature = "json")]
    s.register_module("json", json::open, false)?;

    Ok(())
}
