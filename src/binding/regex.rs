use crate::{
    error::{Result, ToLuaResult},
    prelude::*,
    userdata::{MaybePointer, MaybePtrRef},
};

use ::regex::{Captures, Regex};
use alloc::{string::String, vec::Vec};
use regex::Match;

impl UserData for Captures<'_> {
    type Trans = MaybePointer<Self>;

    fn methods(mt: UserdataRegistry<Self>) -> Result<()> {
        Ok(())
    }

    fn metatable(mt: UserdataRegistry<Self>) -> Result<()> {
        mt.add("__len", Captures::len)?;
        mt.add("__index", |this: &Self, arg: LuaValue| {
            match arg {
                LuaValue::Integer(i) => this.get(i as _),
                LuaValue::String(s) => this.name(s.to_str().unwrap_or_default()),
                _ => None,
            }
            .map(|m| m.as_str())
        })?;
        mt.add_method("__call", |s, this, arg: LuaValue| {
            match arg {
                LuaValue::Integer(i) => this.get(i as _),
                LuaValue::String(s) => this.name(s.to_str().unwrap_or_default()),
                _ => None,
            }
            .map(|m| s.new_userdata_with_values(m, [ArgRef(1)]))
            .ok_or(())
        })?;

        Ok(())
    }
}

impl UserData for Match<'_> {
    fn getter(fields: UserdataRegistry<Self>) -> Result<()> {
        fields
            .set_closure("start", Self::start)?
            .set_closure("string", Self::as_str)?
            .set_closure("end_", Self::end)?;
        Ok(())
    }

    fn methods(mt: UserdataRegistry<Self>) -> Result<()> {
        mt.set_closure("range", Self::range)?;
        Ok(())
    }
}

impl UserData for Regex {
    const TYPE_NAME: &'static str = "Regex";

    fn methods(mt: UserdataRegistry<Self>) -> Result<()> {
        mt.set_closure("shortest_match", Regex::shortest_match)?;
        // https://docs.rs/regex/latest/regex/struct.Regex.html#method.find
        mt.add_method("find", |s, this, (text, pos): (&str, Option<usize>)| {
            pos.map(|p| this.find_at(text, p))
                .unwrap_or_else(|| this.find(text))
                .map(|res| s.new_userdata_with_values(res, [ArgRef(1), ArgRef(2)]))
                .ok_or(())
        })?;
        // // https://docs.rs/regex/latest/regex/struct.Regex.html#method.find_iter
        mt.add_method("gmatch", |s, this, text: &str| unsafe {
            let iter = this.find_iter(text);
            s.new_iter(
                iter.map(|m| (m.as_str(), m.start() + 1, m.end())),
                [ArgRef(2)],
            )
        })?;
        // https://docs.rs/regex/latest/regex/struct.Regex.html#method.split
        mt.add_method("gsplit", |s, this, text: &str| unsafe {
            s.new_iter(this.split(text), [ArgRef(2)])
        })?;
        mt.add_method("split", |s, this, text: &str| {
            s.new_val(IterVec(this.split(text)))
        })?;
        // https://docs.rs/regex/latest/regex/struct.Regex.html#method.replace
        mt.add_method(
            "replace",
            |_, this: &Self, (text, sub): (&str, LuaValue)| {
                Ok(match sub {
                    LuaValue::String(s) => this.replace(text, s.to_string_lossy().as_ref()),
                    LuaValue::Function(f) => this.replace(text, |caps: &Captures| {
                        f.pcall::<_, String>(MaybePtrRef(caps))
                            .map_err(|err| {
                                // TODO: log err
                                std::eprintln!("{err:?}");
                                err
                            })
                            .unwrap_or_default()
                    }),
                    _ => return Err("expect a string/function").convert_error(),
                })
            },
        )?;
        // https://docs.rs/regex/latest/regex/struct.Regex.html#method.captures
        mt.add_method("capture", |s, this, text: &str| {
            this.captures(text)
                .map(|cap| s.new_userdata_with_values(cap, [ArgRef(1), ArgRef(2)]))
                .ok_or(())
        })?;
        mt.add_method("match", |_, this, text: &str| {
            MultiRet(
                this.captures(text)
                    .map(|cap| {
                        cap.iter()
                            .skip(1)
                            .filter_map(|m| Some(m?.as_str()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default(),
            )
        })?;

        Ok(())
    }

    fn metatable(mt: UserdataRegistry<Self>) -> LuaResult<()> {
        mt.set_closure("new", Regex::new)?;
        Ok(())
    }
}

pub fn open(s: &LuaState) -> Result<LuaTable> {
    s.register_usertype::<Regex>()
}
