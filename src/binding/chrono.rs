use crate::prelude::*;
use ::chrono::prelude::*;
use alloc::string::ToString;
use chrono::{Days, Duration, Months};
use core::{fmt::Display, str::FromStr};

impl UserData for Duration {
    fn getter(fields: UserdataRegistry<Self>) -> LuaResult<()> {
        fields.set_closure("weeks", Self::num_weeks)?;
        fields.set_closure("days", Self::num_days)?;
        fields.set_closure("hours", Self::num_hours)?;
        fields.set_closure("minutes", Self::num_minutes)?;
        fields.set_closure("seconds", Self::num_seconds)?;
        fields.set_closure("milliseconds", Self::num_milliseconds)?;
        fields.set_closure("microseconds", Self::num_microseconds)?;
        fields.set_closure("nanoseconds", Self::num_nanoseconds)?;

        fields.set_closure("is_zero", Self::is_zero)?;
        Ok(())
    }

    fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
        methods.set_function("add", |lua, (this, val): (&Self, ValRef)| {
            if let Ok(d) = <&Self as FromLua>::from_lua(val.state(), val) {
                Ok(*this + *d)
            } else {
                Err("invalid arg").lua_result()
            }
        })?;
        methods.set_function("sub", |lua, (this, val): (&Self, ValRef)| {
            if let Ok(d) = <&Self as FromLua>::from_lua(val.state(), val) {
                Ok(*this - *d)
            } else {
                Err("invalid arg").lua_result()
            }
        })?;
        Ok(())
    }

    fn metatable(mt: UserdataRegistry<Self>) -> LuaResult<()> {
        mt.set_closure("weeks", Self::weeks)?;
        mt.set_closure("days", Self::days)?;
        mt.set_closure("hours", Self::hours)?;
        mt.set_closure("minutes", Self::minutes)?;
        mt.set_closure("seconds", Self::seconds)?;
        mt.set_closure("milliseconds", Self::milliseconds)?;
        mt.set_closure("microseconds", Self::microseconds)?;
        mt.set_closure("nanoseconds", Self::nanoseconds)?;

        mt.set_closure("zero", Self::zero)?;
        mt.set_closure("min", Self::min_value)?;
        mt.set_closure("max", Self::max_value)?;

        let methods = mt.get("__method")?;
        mt.set("__add", methods.get("add")?)?;
        mt.set("__sub", methods.get("sub")?)?;

        Ok(())
    }
}

impl FromLua<'_> for Days {
    fn from_lua(lua: &LuaState, val: ValRef) -> LuaResult<Self> {
        FromLua::from_lua(lua, val).map(Days::new)
    }
}

impl FromLua<'_> for Months {
    fn from_lua(lua: &LuaState, val: ValRef) -> LuaResult<Self> {
        FromLua::from_lua(lua, val).map(Months::new)
    }
}

impl UserData for NaiveDate {
    fn getter(fields: UserdataRegistry<Self>) -> LuaResult<()> {
        Ok(())
    }

    fn metatable(mt: UserdataRegistry<Self>) -> LuaResult<()> {
        mt.set_closure(
            "from_ymd",
            move |y: Option<i32>, m: Option<u32>, d: Option<u32>| {
                Self::from_ymd_opt(y.unwrap_or(0), m.unwrap_or(0), d.unwrap_or(0))
            },
        )?;
        Ok(())
    }
}

impl UserData for NaiveTime {
    fn metatable(mt: UserdataRegistry<Self>) -> LuaResult<()> {
        mt.set_closure(
            "from_hms",
            move |h: Option<u32>, min: Option<u32>, s: Option<u32>| {
                Self::from_hms_opt(h.unwrap_or(0), min.unwrap_or(0), s.unwrap_or(0))
            },
        )?;
        Ok(())
    }
}

impl UserData for NaiveDateTime {
    fn metatable(mt: UserdataRegistry<Self>) -> LuaResult<()> {
        mt.set_closure("parse", |s, fmt: Option<&str>| {
            if let Some(fmt) = fmt {
                NaiveDateTime::parse_from_str(s, fmt)
            } else {
                NaiveDateTime::from_str(s)
            }
        })?;
        mt.set_closure("new", |d: &NaiveDate, t: &NaiveTime| {
            NaiveDateTime::new(*d, *t)
        })?;
        mt.set_closure("__tostring", Self::to_string)?;

        Ok(())
    }

    fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
        methods.set_closure("timestamp", |this: &Self| this.and_utc().timestamp())?;
        methods.set_closure("timestamp_millis", |this: &Self| {
            this.and_utc().timestamp_millis()
        })?;
        methods.set_closure("timestamp_micros", |this: &Self| {
            this.and_utc().timestamp_micros()
        })?;
        methods.set_closure("timestamp_nanos", |this: &Self| {
            this.and_utc().timestamp_nanos_opt()
        })?;
        methods.set_closure("time", Self::time)?;
        methods.set_closure("date", Self::date)?;

        methods.set_closure("with_year", Self::with_year)?;
        methods.set_closure("with_month", Self::with_month)?;
        methods.set_closure("with_day", Self::with_day)?;
        methods.set_closure("with_hour", Self::with_hour)?;
        methods.set_closure("with_minute", Self::with_minute)?;
        methods.set_closure("with_second", Self::with_second)?;
        methods.set_closure("with_nanosecond", Self::with_nanosecond)?;

        methods.set_closure("add_months", |this: &Self, n: i64| {
            if n < 0 {
                this.checked_sub_months(Months::new((-n) as _))
            } else {
                this.checked_add_months(Months::new(n as _))
            }
        })?;
        methods.set_closure("add_days", |this: &Self, n: i64| {
            if n < 0 {
                this.checked_sub_days(Days::new((-n) as _))
            } else {
                this.checked_add_days(Days::new(n as _))
            }
        })?;
        methods.set_function("add", |lua, (this, val): (&Self, ValRef)| {
            if let Ok(d) = <&Duration as FromLua>::from_lua(val.state(), val) {
                Ok(this.clone() + *d)
            } else {
                Err("invalid arg").lua_result()
            }
        })?;
        methods.set_function("sub", |lua, (this, val): (&Self, ValRef)| {
            if let Ok(d) = <&Duration as FromLua>::from_lua(val.state(), val) {
                Ok(this.clone() - *d)
            } else {
                Err("invalid arg").lua_result()
            }
        })?;

        methods.set_closure("format", |this: &Self, fmt| this.format(fmt).to_string())?;
        methods.set_closure("to_utc", |this: &Self| {
            this.and_local_timezone(Utc).single()
        })?;
        methods.set_closure("to_local", |this: &Self| {
            this.and_local_timezone(Local).single()
        })?;
        Ok(())
    }
}

impl<Tz: TimeZone + Clone + 'static> UserData for DateTime<Tz>
where
    Tz::Offset: Display,
{
    fn getter(fields: UserdataRegistry<Self>) -> LuaResult<()> {
        fields.add("year", Self::year)?;
        fields.add("month", Self::month)?;
        fields.add("day", Self::day)?;
        fields.add("hour", Self::hour)?;
        fields.add("minute", Self::minute)?;
        fields.add("second", Self::second)?;
        fields.add("nanosecond", Self::nanosecond)?;
        Ok(())
    }

    fn methods(methods: UserdataRegistry<Self>) -> LuaResult<()> {
        methods.set_closure("timestamp", Self::timestamp)?;
        methods.set_closure("timestamp_millis", Self::timestamp_millis)?;
        methods.set_closure("timestamp_micros", Self::timestamp_micros)?;
        methods.set_closure("timestamp_nanos", Self::timestamp_nanos_opt)?;
        methods.set_closure("time", Self::time)?;
        methods.set_closure("date", Self::date_naive)?;
        methods.add("years_since", |this: &Self, base: &Self| {
            this.years_since(base.clone())
        })?;

        methods.set_closure("with_year", Self::with_year)?;
        methods.set_closure("with_month", Self::with_month)?;
        methods.set_closure("with_day", Self::with_day)?;
        methods.set_closure("with_hour", Self::with_hour)?;
        methods.set_closure("with_minute", Self::with_minute)?;
        methods.set_closure("with_second", Self::with_second)?;
        methods.set_closure("with_nanosecond", Self::with_nanosecond)?;

        methods.set_closure("add_months", |this: &Self, n: i64| {
            if n < 0 {
                this.clone().checked_sub_months(Months::new((-n) as _))
            } else {
                this.clone().checked_add_months(Months::new(n as _))
            }
        })?;
        methods.set_closure("add_days", |this: &Self, n: i64| {
            if n < 0 {
                this.clone().checked_sub_days(Days::new((-n) as _))
            } else {
                this.clone().checked_add_days(Days::new(n as _))
            }
        })?;
        methods.set_function("add", |lua, (this, val): (&Self, ValRef)| {
            if let Ok(d) = <&Duration as FromLua>::from_lua(val.state(), val) {
                Ok(this.clone() + *d)
            } else {
                Err("invalid arg").lua_result()
            }
        })?;
        methods.set_function("sub", |lua, (this, val): (&Self, ValRef)| {
            if let Ok(d) = <&Duration as FromLua>::from_lua(val.state(), val) {
                Ok(this.clone() - *d)
            } else {
                Err("invalid arg").lua_result()
            }
        })?;

        methods.set_closure("format", |this: &Self, fmt| this.format(fmt).to_string())?;
        methods.set_closure("to_rfc2822", Self::to_rfc2822)?;
        methods.set_closure("to_rfc3339", Self::to_rfc3339)?;

        methods.set_closure("to_utc", |this: &Self| this.with_timezone(&Utc))?;
        methods.set_closure("to_local", |this: &Self| this.with_timezone(&Local))?;
        Ok(())
    }

    fn metatable(mt: UserdataRegistry<Self>) -> LuaResult<()> {
        mt.set_closure("__tostring", Self::to_string)?;
        mt.set_closure("parse", |s, fmt| DateTime::parse_from_str(s, fmt))?;

        Ok(())
    }
}

pub fn open(lua: &LuaState) -> LuaResult<LuaTable> {
    let m = lua.new_table()?;

    let utc = lua.register_usertype::<DateTime<Utc>>()?;
    utc.set_closure("now", Utc::now)?;
    utc.set_closure("parse", |FromStr::<DateTime<Utc>>(dt)| dt)?;
    init_timezone(Utc, &utc)?;
    m.set("DateTimeUtc", utc)?;

    let local = lua.register_usertype::<DateTime<Local>>()?;
    local.set_closure("now", Local::now)?;
    local.set_closure("parse", |FromStr::<DateTime<Local>>(dt)| dt)?;
    init_timezone(Local, &local)?;
    m.set("DateTimeLocal", local)?;

    m.set("Duration", lua.register_usertype::<Duration>()?)?;
    m.set("NaiveDate", lua.register_usertype::<NaiveDate>()?)?;
    m.set("NaiveTime", lua.register_usertype::<NaiveTime>()?)?;
    m.set("NaiveDateTime", lua.register_usertype::<NaiveDateTime>()?)?;

    Ok(m)
}

fn init_timezone<Tz: TimeZone + Copy + 'static>(tz: Tz, t: &LuaTable) -> LuaResult<()>
where
    Tz::Offset: Display,
{
    t.set_closure(
        "from_ymd_hms",
        move |y: Option<i32>,
              m: Option<u32>,
              d: Option<u32>,
              h: Option<u32>,
              min: Option<u32>,
              s: Option<u32>| {
            tz.with_ymd_and_hms(
                y.unwrap_or(0),
                m.unwrap_or(0),
                d.unwrap_or(0),
                h.unwrap_or(0),
                min.unwrap_or(0),
                s.unwrap_or(0),
            )
            .single()
        },
    )?;
    t.set_closure(
        "from_timestamp",
        move |secs: Option<_>, nsecs: Option<_>| {
            tz.timestamp_opt(secs.unwrap_or(0), nsecs.unwrap_or(0))
                .single()
        },
    )?;
    t.set_closure("from_timestamp_millis", move |millis| {
        tz.timestamp_millis_opt(millis).single()
    })?;
    t.set_closure("from_timestamp_nanos", move |nanos| {
        tz.timestamp_nanos(nanos)
    })?;
    t.set_closure("from_timestamp_micros", move |micros| {
        tz.timestamp_micros(micros).single()
    })?;

    Ok(())
}
