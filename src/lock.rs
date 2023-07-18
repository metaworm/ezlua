//! Implementation to lua_lock/lua_unlock, for multi-thread purpose

use crate::ffi::{lua_State, lua_getextraspace};

use alloc::boxed::Box;
#[cfg(feature = "parking_lot")]
use parking_lot::{Mutex, MutexGuard};
#[cfg(not(feature = "parking_lot"))]
use std::sync::{Mutex, MutexGuard};

#[repr(C)]
pub struct Extra {
    mutex: Mutex<()>,
    guard: Option<MutexGuard<'static, ()>>,
}

#[inline(always)]
pub fn get_extra<'a>(l: *mut lua_State) -> &'a mut Extra {
    unsafe { *core::mem::transmute::<_, *mut &mut Extra>(lua_getextraspace(l)) }
}

#[no_mangle]
unsafe extern "C" fn ezlua_lock(l: *mut lua_State) {
    let extra: &'static mut Extra = get_extra(l);
    #[cfg(not(feature = "parking_lot"))]
    extra.guard.replace(extra.mutex.lock().expect("lualock"));
    #[cfg(feature = "parking_lot")]
    extra.guard.replace(extra.mutex.lock());
}

#[no_mangle]
unsafe extern "C" fn ezlua_unlock(l: *mut lua_State) {
    get_extra(l).guard.take();
}

#[no_mangle]
unsafe extern "C" fn ezlua_userstateopen(l: *mut lua_State) {
    let extra = Box::new(Extra {
        mutex: Mutex::new(()),
        guard: None,
    });
    *core::mem::transmute::<_, *mut *mut Extra>(lua_getextraspace(l)) = Box::into_raw(extra);
}

#[no_mangle]
unsafe extern "C" fn ezlua_userstateclose(l: *mut lua_State) {
    let e = get_extra(l);
    drop(Box::from_raw(e));
}
