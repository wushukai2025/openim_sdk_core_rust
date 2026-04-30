use std::ffi::{CStr, CString};
use std::fmt::Display;
use std::os::raw::{c_char, c_int};
use std::ptr;

use openim_session::{LoginCredentials, OpenImSession, SessionConfig, SessionState};
use openim_types::Platform;

pub const OPENIM_FFI_OK: c_int = 0;
pub const OPENIM_FFI_NULL: c_int = 1;
pub const OPENIM_FFI_INVALID_UTF8: c_int = 2;
pub const OPENIM_FFI_INVALID_ARGS: c_int = 3;
pub const OPENIM_FFI_ERROR: c_int = 4;

const OPENIM_FFI_VERSION: &[u8] = b"openim-rust-ffi/0.1.0\0";
const OPENIM_NATIVE_CALLBACK_THREAD: &[u8] = b"sdk_serialized_callback_queue\0";

pub struct OpenImFfiSession {
    session: OpenImSession,
    last_error: CString,
}

#[no_mangle]
pub extern "C" fn openim_ffi_version() -> *const c_char {
    OPENIM_FFI_VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn openim_native_callback_thread_policy() -> *const c_char {
    OPENIM_NATIVE_CALLBACK_THREAD.as_ptr().cast()
}

#[no_mangle]
pub unsafe extern "C" fn openim_session_create(
    api_addr: *const c_char,
    ws_addr: *const c_char,
    platform_id: c_int,
) -> *mut OpenImFfiSession {
    let Ok(api_addr) = c_str(api_addr) else {
        return ptr::null_mut();
    };
    let Ok(ws_addr) = c_str(ws_addr) else {
        return ptr::null_mut();
    };
    let Some(platform) = Platform::from_i32(platform_id) else {
        return ptr::null_mut();
    };

    let config = SessionConfig::new(platform, api_addr, ws_addr);
    match OpenImSession::new(config) {
        Ok(session) => Box::into_raw(Box::new(OpenImFfiSession {
            session,
            last_error: empty_c_string(),
        })),
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn openim_session_destroy(handle: *mut OpenImFfiSession) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

#[no_mangle]
pub unsafe extern "C" fn openim_session_init(handle: *mut OpenImFfiSession) -> c_int {
    run_session_op(handle, |session| session.session.init())
}

#[no_mangle]
pub unsafe extern "C" fn openim_session_login(
    handle: *mut OpenImFfiSession,
    user_id: *const c_char,
    token: *const c_char,
) -> c_int {
    let user_id = match c_str(user_id) {
        Ok(value) => value.to_string(),
        Err(code) => return set_handle_error(handle, code, "user_id is invalid"),
    };
    let token = match c_str(token) {
        Ok(value) => value.to_string(),
        Err(code) => return set_handle_error(handle, code, "token is invalid"),
    };

    run_session_op(handle, |session| {
        session.session.login(LoginCredentials::new(user_id, token))
    })
}

#[no_mangle]
pub unsafe extern "C" fn openim_session_logout(handle: *mut OpenImFfiSession) -> c_int {
    run_session_op(handle, |session| session.session.logout())
}

#[no_mangle]
pub unsafe extern "C" fn openim_session_uninit(handle: *mut OpenImFfiSession) -> c_int {
    run_session_op(handle, |session| session.session.uninit())
}

#[no_mangle]
pub unsafe extern "C" fn openim_session_state(handle: *const OpenImFfiSession) -> c_int {
    if handle.is_null() {
        return -1;
    }
    state_code((&*handle).session.state())
}

#[no_mangle]
pub unsafe extern "C" fn openim_session_last_error(
    handle: *const OpenImFfiSession,
) -> *const c_char {
    if handle.is_null() {
        return ptr::null();
    }
    (&*handle).last_error.as_ptr()
}

fn run_session_op<E, F>(handle: *mut OpenImFfiSession, op: F) -> c_int
where
    E: Display,
    F: FnOnce(&mut OpenImFfiSession) -> std::result::Result<(), E>,
{
    if handle.is_null() {
        return OPENIM_FFI_NULL;
    }

    let session = unsafe { &mut *handle };
    match op(session) {
        Ok(()) => {
            session.last_error = empty_c_string();
            OPENIM_FFI_OK
        }
        Err(err) => {
            session.last_error = c_string_lossy(&err.to_string());
            OPENIM_FFI_ERROR
        }
    }
}

fn set_handle_error(handle: *mut OpenImFfiSession, code: c_int, message: &str) -> c_int {
    if !handle.is_null() {
        unsafe {
            (*handle).last_error = c_string_lossy(message);
        }
    }
    code
}

unsafe fn c_str<'a>(ptr: *const c_char) -> std::result::Result<&'a str, c_int> {
    if ptr.is_null() {
        return Err(OPENIM_FFI_NULL);
    }
    CStr::from_ptr(ptr)
        .to_str()
        .map_err(|_| OPENIM_FFI_INVALID_UTF8)
}

fn state_code(state: SessionState) -> c_int {
    match state {
        SessionState::Created => 0,
        SessionState::Initialized => 1,
        SessionState::LoggedIn => 2,
        SessionState::LoggedOut => 3,
        SessionState::Uninitialized => 4,
    }
}

fn empty_c_string() -> CString {
    CString::new("").expect("empty string has no nul byte")
}

fn c_string_lossy(value: &str) -> CString {
    CString::new(value).unwrap_or_else(|_| CString::new("openim error contains nul byte").unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = include_str!("../include/openim_ffi.h");
    const DESKTOP_EXAMPLE: &str =
        include_str!("../../../examples/desktop-c/openim_desktop_lifecycle.c");
    const IOS_EXAMPLE: &str =
        include_str!("../../../examples/ios-swift/OpenIMLifecycleExample.swift");
    const ANDROID_KOTLIN_EXAMPLE: &str =
        include_str!("../../../examples/android-kotlin/OpenIMLifecycleExample.kt");
    const ANDROID_JNI_EXAMPLE: &str =
        include_str!("../../../examples/android-kotlin/openim_jni_lifecycle.cc");
    const LIFECYCLE_EXPORTS: &[&str] = &[
        "openim_session_create",
        "openim_session_init",
        "openim_session_login",
        "openim_session_logout",
        "openim_session_uninit",
        "openim_session_destroy",
    ];

    fn c_string(value: &str) -> CString {
        CString::new(value).unwrap()
    }

    #[test]
    fn c_abi_session_lifecycle_uses_opaque_handle() {
        let api_addr = c_string("https://api.openim.test");
        let ws_addr = c_string("wss://ws.openim.test");
        let user_id = c_string("u1");
        let token = c_string("token");

        unsafe {
            let handle =
                openim_session_create(api_addr.as_ptr(), ws_addr.as_ptr(), Platform::Web.as_i32());
            assert!(!handle.is_null());
            assert_eq!(openim_session_state(handle), 0);
            assert_eq!(openim_session_init(handle), OPENIM_FFI_OK);
            assert_eq!(openim_session_state(handle), 1);
            assert_eq!(
                openim_session_login(handle, user_id.as_ptr(), token.as_ptr()),
                OPENIM_FFI_OK
            );
            assert_eq!(openim_session_state(handle), 2);
            assert_eq!(openim_session_logout(handle), OPENIM_FFI_OK);
            assert_eq!(openim_session_state(handle), 3);
            assert_eq!(openim_session_uninit(handle), OPENIM_FFI_OK);
            assert_eq!(openim_session_state(handle), 4);
            assert_eq!(
                CStr::from_ptr(openim_session_last_error(handle))
                    .to_str()
                    .unwrap(),
                ""
            );
            openim_session_destroy(handle);
        }
    }

    #[test]
    fn c_abi_reports_invalid_state_errors_on_handle() {
        let api_addr = c_string("https://api.openim.test");
        let ws_addr = c_string("wss://ws.openim.test");
        let user_id = c_string("u1");
        let token = c_string("token");

        unsafe {
            let handle =
                openim_session_create(api_addr.as_ptr(), ws_addr.as_ptr(), Platform::Web.as_i32());
            assert_eq!(
                openim_session_login(handle, user_id.as_ptr(), token.as_ptr()),
                OPENIM_FFI_ERROR
            );
            assert!(CStr::from_ptr(openim_session_last_error(handle))
                .to_str()
                .unwrap()
                .contains("not initialized"));
            openim_session_destroy(handle);
        }
    }

    #[test]
    fn c_abi_rejects_null_or_invalid_inputs() {
        let api_addr = c_string("https://api.openim.test");
        let ws_addr = c_string("wss://ws.openim.test");

        unsafe {
            assert!(openim_session_create(api_addr.as_ptr(), ws_addr.as_ptr(), 99).is_null());
            assert_eq!(openim_session_state(ptr::null()), -1);
            assert_eq!(openim_session_init(ptr::null_mut()), OPENIM_FFI_NULL);
            assert_eq!(
                CStr::from_ptr(openim_native_callback_thread_policy())
                    .to_str()
                    .unwrap(),
                "sdk_serialized_callback_queue"
            );
        }
    }

    #[test]
    fn native_header_and_examples_cover_lifecycle_exports() {
        assert_contains_all(HEADER, LIFECYCLE_EXPORTS);
        assert_contains_all(DESKTOP_EXAMPLE, LIFECYCLE_EXPORTS);
        assert_contains_all(IOS_EXAMPLE, LIFECYCLE_EXPORTS);
        assert_contains_all(ANDROID_JNI_EXAMPLE, LIFECYCLE_EXPORTS);
        assert_contains_all(
            ANDROID_KOTLIN_EXAMPLE,
            &[
                "openimSessionCreate",
                "openimSessionInit",
                "openimSessionLogin",
                "openimSessionLogout",
                "openimSessionUninit",
                "openimSessionDestroy",
            ],
        );
        assert!(HEADER.contains("OpenImFfiSession"));
        assert!(HEADER.contains("OPENIM_PLATFORM_IOS"));
        assert!(HEADER.contains("OPENIM_PLATFORM_ANDROID"));
        assert!(HEADER.contains("OPENIM_PLATFORM_LINUX"));
    }

    fn assert_contains_all(source: &str, needles: &[&str]) {
        for needle in needles {
            assert!(source.contains(needle), "missing {needle}");
        }
    }
}
