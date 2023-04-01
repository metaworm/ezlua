//! Implementation to lua_lock/lua_unlock, for multi-thread purpose

use crate::ffi::{lua_State, lua_getextraspace};

use alloc::boxed::Box;
use parking_lot::{lock_api::RawMutex, Mutex};

#[repr(C)]
pub struct Extra {
    mutex: Mutex<()>,
}

#[inline(always)]
pub fn get_extra<'a>(l: *mut lua_State) -> &'a mut Extra {
    unsafe { *core::mem::transmute::<_, *mut &mut Extra>(lua_getextraspace(l)) }
}

#[no_mangle]
unsafe extern "C" fn ezlua_lock(l: *mut lua_State) {
    let extra = get_extra(l);
    extra.mutex.raw().lock();
}

#[no_mangle]
unsafe extern "C" fn ezlua_unlock(l: *mut lua_State) {
    get_extra(l).mutex.force_unlock()
}

#[no_mangle]
unsafe extern "C" fn ezlua_userstateopen(l: *mut lua_State) {
    let extra = Box::new(Extra {
        mutex: Mutex::new(()),
    });
    *core::mem::transmute::<_, *mut *mut Extra>(lua_getextraspace(l)) = Box::into_raw(extra);
}

#[no_mangle]
unsafe extern "C" fn ezlua_userstateclose(l: *mut lua_State) {
    let e = get_extra(l);
    drop(Box::from_raw(e));
}
