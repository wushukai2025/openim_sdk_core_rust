use std::ffi::{CStr, CString};
use std::fmt::Display;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

use openim_session::{
    ListenerId, LoginCredentials, OpenImSession, SessionConfig, SessionEvent, SessionResourceKind,
    SessionState,
};
use openim_session_native::NativeSessionResourceAdapter;
use openim_types::Platform;
use serde_json::json;

pub const OPENIM_FFI_OK: c_int = 0;
pub const OPENIM_FFI_NULL: c_int = 1;
pub const OPENIM_FFI_INVALID_UTF8: c_int = 2;
pub const OPENIM_FFI_INVALID_ARGS: c_int = 3;
pub const OPENIM_FFI_ERROR: c_int = 4;

const OPENIM_FFI_VERSION: &[u8] = b"openim-rust-ffi/0.1.0\0";
const OPENIM_NATIVE_CALLBACK_THREAD: &[u8] = b"sdk_serialized_callback_queue\0";

pub type OpenImFfiSessionEventCallback =
    unsafe extern "C" fn(user_data: *mut c_void, event: *const c_char, payload_json: *const c_char);

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
    openim_session_create_with_data_dir(api_addr, ws_addr, platform_id, ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn openim_session_create_with_data_dir(
    api_addr: *const c_char,
    ws_addr: *const c_char,
    platform_id: c_int,
    data_dir: *const c_char,
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

    let mut config = SessionConfig::new(platform, api_addr, ws_addr);
    if !data_dir.is_null() {
        let Ok(data_dir) = c_str(data_dir) else {
            return ptr::null_mut();
        };
        config = config.with_data_dir(data_dir);
    }

    match OpenImSession::with_resource_adapter(
        config,
        Box::new(NativeSessionResourceAdapter::new()),
    ) {
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

#[no_mangle]
pub unsafe extern "C" fn openim_session_register_listener(
    handle: *mut OpenImFfiSession,
    callback: Option<OpenImFfiSessionEventCallback>,
    user_data: *mut c_void,
) -> ListenerId {
    if handle.is_null() {
        return 0;
    }
    let Some(callback) = callback else {
        set_handle_error(handle, OPENIM_FFI_INVALID_ARGS, "listener callback is null");
        return 0;
    };

    let session = &mut *handle;
    let user_data = user_data as usize;
    let listener_id = session.session.register_listener(move |event| {
        let event_name = c_string_lossy(session_event_name(event));
        let payload_json = c_string_lossy(&session_event_payload_json(event));
        unsafe {
            callback(
                user_data as *mut c_void,
                event_name.as_ptr(),
                payload_json.as_ptr(),
            );
        }
    });
    session.last_error = empty_c_string();
    listener_id
}

#[no_mangle]
pub unsafe extern "C" fn openim_session_unregister_listener(
    handle: *mut OpenImFfiSession,
    listener_id: ListenerId,
) -> c_int {
    if handle.is_null() {
        return OPENIM_FFI_NULL;
    }
    if listener_id == 0 {
        return set_handle_error(handle, OPENIM_FFI_INVALID_ARGS, "listener_id is invalid");
    }

    let session = &mut *handle;
    if session.session.unregister_listener(listener_id) {
        session.last_error = empty_c_string();
        OPENIM_FFI_OK
    } else {
        set_handle_error(
            handle,
            OPENIM_FFI_INVALID_ARGS,
            "listener_id was not registered",
        )
    }
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

fn session_event_name(event: &SessionEvent) -> &'static str {
    match event {
        SessionEvent::Initialized => "initialized",
        SessionEvent::LoggedIn { .. } => "loggedIn",
        SessionEvent::LoggedOut { .. } => "loggedOut",
        SessionEvent::Uninitialized => "uninitialized",
        SessionEvent::ListenerRegistered { .. } => "listenerRegistered",
        SessionEvent::ListenerUnregistered { .. } => "listenerUnregistered",
        SessionEvent::TaskStarted { .. } => "taskStarted",
        SessionEvent::TaskStopped { .. } => "taskStopped",
        SessionEvent::ResourceOpened { .. } => "resourceOpened",
        SessionEvent::ResourceClosed { .. } => "resourceClosed",
        SessionEvent::NewMessages { .. } => "newMessages",
        SessionEvent::ConversationChanged { .. } => "conversationChanged",
    }
}

fn session_event_payload_json(event: &SessionEvent) -> String {
    match event {
        SessionEvent::Initialized | SessionEvent::Uninitialized => "{}".to_string(),
        SessionEvent::LoggedIn { user_id } | SessionEvent::LoggedOut { user_id } => {
            json!({ "userId": user_id }).to_string()
        }
        SessionEvent::ListenerRegistered { listener_id }
        | SessionEvent::ListenerUnregistered { listener_id } => {
            json!({ "listenerId": listener_id }).to_string()
        }
        SessionEvent::TaskStarted { name } | SessionEvent::TaskStopped { name } => {
            json!({ "name": name }).to_string()
        }
        SessionEvent::ResourceOpened { kind, name }
        | SessionEvent::ResourceClosed { kind, name } => {
            json!({ "kind": resource_kind_name(*kind), "name": name }).to_string()
        }
        SessionEvent::NewMessages { messages } => json!({ "messages": messages }).to_string(),
        SessionEvent::ConversationChanged { conversations } => {
            json!({ "conversations": conversations }).to_string()
        }
    }
}

fn resource_kind_name(kind: SessionResourceKind) -> &'static str {
    match kind {
        SessionResourceKind::Storage => "storage",
        SessionResourceKind::Transport => "transport",
        SessionResourceKind::Sync => "sync",
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
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::thread;

    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, tungstenite::Message as WsMessage};

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
    const DATA_DIR_CREATE_EXPORTS: &[&str] = &["openim_session_create_with_data_dir"];
    const LISTENER_EXPORTS: &[&str] = &[
        "OpenImFfiSessionEventCallback",
        "openim_session_register_listener",
        "openim_session_unregister_listener",
    ];
    const LISTENER_FUNCTIONS: &[&str] = &[
        "openim_session_register_listener",
        "openim_session_unregister_listener",
    ];

    fn c_string(value: &str) -> CString {
        CString::new(value).unwrap()
    }

    #[test]
    fn c_abi_session_lifecycle_uses_opaque_handle() {
        let api_addr = c_string("https://api.openim.test");
        let ws_addr = c_string(&spawn_transport_server());
        let user_id = c_string("u1");
        let token = c_string("token");

        unsafe {
            let handle = openim_session_create(
                api_addr.as_ptr(),
                ws_addr.as_ptr(),
                Platform::Macos.as_i32(),
            );
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
            let handle = openim_session_create(
                api_addr.as_ptr(),
                ws_addr.as_ptr(),
                Platform::Macos.as_i32(),
            );
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
        let invalid_utf8_dir = [0xff_u8, 0x00];

        unsafe {
            assert!(openim_session_create(api_addr.as_ptr(), ws_addr.as_ptr(), 99).is_null());
            assert!(openim_session_create_with_data_dir(
                api_addr.as_ptr(),
                ws_addr.as_ptr(),
                Platform::Macos.as_i32(),
                invalid_utf8_dir.as_ptr().cast()
            )
            .is_null());
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
    fn c_abi_listener_dispatches_lifecycle_events() {
        let api_addr = c_string("https://api.openim.test");
        let ws_addr = c_string(&spawn_transport_server());
        let user_id = c_string("u1");
        let token = c_string("token");
        let mut events = Vec::<(String, String)>::new();

        unsafe {
            let handle = openim_session_create(
                api_addr.as_ptr(),
                ws_addr.as_ptr(),
                Platform::Macos.as_i32(),
            );
            let listener_id = openim_session_register_listener(
                handle,
                Some(collect_event),
                (&mut events as *mut Vec<(String, String)>).cast(),
            );
            assert!(listener_id > 0);
            assert_eq!(openim_session_init(handle), OPENIM_FFI_OK);
            assert_eq!(
                openim_session_login(handle, user_id.as_ptr(), token.as_ptr()),
                OPENIM_FFI_OK
            );
            assert_eq!(openim_session_logout(handle), OPENIM_FFI_OK);
            assert_eq!(
                openim_session_unregister_listener(handle, listener_id),
                OPENIM_FFI_OK
            );
            assert_eq!(openim_session_uninit(handle), OPENIM_FFI_OK);
            openim_session_destroy(handle);
        }

        let event_names = events
            .iter()
            .map(|(event, _)| event.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            event_names,
            [
                "listenerRegistered",
                "initialized",
                "taskStarted",
                "taskStarted",
                "resourceOpened",
                "resourceOpened",
                "loggedIn",
                "taskStopped",
                "taskStopped",
                "resourceClosed",
                "resourceClosed",
                "loggedOut",
            ]
        );
        let registered_payload: serde_json::Value = serde_json::from_str(&events[0].1).unwrap();
        assert_eq!(registered_payload["listenerId"], 1);
        let first_resource_payload: serde_json::Value = serde_json::from_str(&events[4].1).unwrap();
        assert_eq!(first_resource_payload["kind"], "transport");
        let login_payload: serde_json::Value = serde_json::from_str(&events[6].1).unwrap();
        assert_eq!(login_payload["userId"], "u1");
        assert!(!event_names.contains(&"uninitialized"));
    }

    #[test]
    fn c_abi_create_with_data_dir_opens_native_storage_resource() {
        let api_addr = c_string("https://api.openim.test");
        let ws_addr = c_string(&spawn_transport_server());
        let user_id = c_string("u1");
        let token = c_string("token");
        let data_dir = unique_temp_dir("ffi-data-dir");
        let data_dir_c = c_string(data_dir.to_string_lossy().as_ref());
        let expected_db_path = data_dir.join("OpenIM_v3_u1.db");

        unsafe {
            let handle = openim_session_create_with_data_dir(
                api_addr.as_ptr(),
                ws_addr.as_ptr(),
                Platform::Macos.as_i32(),
                data_dir_c.as_ptr(),
            );
            assert!(!handle.is_null());
            assert_eq!(openim_session_init(handle), OPENIM_FFI_OK);
            assert_eq!(
                openim_session_login(handle, user_id.as_ptr(), token.as_ptr()),
                OPENIM_FFI_OK
            );
            assert!(expected_db_path.exists());
            assert_eq!(openim_session_logout(handle), OPENIM_FFI_OK);
            openim_session_destroy(handle);
        }

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn native_header_and_examples_cover_lifecycle_exports() {
        assert_contains_all(HEADER, LIFECYCLE_EXPORTS);
        assert_contains_all(HEADER, DATA_DIR_CREATE_EXPORTS);
        assert_contains_all(HEADER, LISTENER_EXPORTS);
        assert_contains_all(DESKTOP_EXAMPLE, LISTENER_FUNCTIONS);
        assert_contains_all(IOS_EXAMPLE, LISTENER_FUNCTIONS);
        assert_contains_all(ANDROID_JNI_EXAMPLE, LISTENER_FUNCTIONS);
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
                "openimSessionRegisterListener",
                "openimSessionUnregisterListener",
                "OpenIMSessionEventListener",
            ],
        );
        assert!(HEADER.contains("OpenImFfiSession"));
        assert!(HEADER.contains("OPENIM_PLATFORM_IOS"));
        assert!(HEADER.contains("OPENIM_PLATFORM_ANDROID"));
        assert!(HEADER.contains("OPENIM_PLATFORM_MACOS"));
        assert_contains_all(
            DESKTOP_EXAMPLE,
            &[
                "OPENIM_PLATFORM_MACOS",
                "OPENIM_API_ADDR",
                "OPENIM_WS_ADDR",
                "OPENIM_USER_ID",
                "OPENIM_TOKEN",
                "OPENIM_DATA_DIR",
            ],
        );
        assert_contains_all(DESKTOP_EXAMPLE, DATA_DIR_CREATE_EXPORTS);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn desktop_c_example_builds_and_runs_against_local_staticlib() {
        use std::fs;
        use std::path::PathBuf;
        use std::process::Command;
        use std::time::{SystemTime, UNIX_EPOCH};

        let target_dir = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let workspace_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let staticlib_path = target_dir.join("libopenim_ffi.a");
        assert!(
            staticlib_path.is_file(),
            "missing staticlib at {}",
            staticlib_path.display()
        );

        let temp_dir = std::env::temp_dir().join(format!(
            "openim-desktop-c-example-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let binary_path = temp_dir.join("openim_desktop_lifecycle");

        let build = Command::new("cargo")
            .current_dir(&workspace_dir)
            .arg("build")
            .arg("-p")
            .arg("openim-ffi")
            .output()
            .unwrap();
        assert!(
            build.status.success(),
            "cargo build failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&build.stdout),
            String::from_utf8_lossy(&build.stderr)
        );

        let compile = Command::new("clang")
            .current_dir(&workspace_dir)
            .arg("-I")
            .arg("crates/openim-ffi/include")
            .arg("examples/desktop-c/openim_desktop_lifecycle.c")
            .arg(&staticlib_path)
            .arg("-o")
            .arg(&binary_path)
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "clang failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&compile.stdout),
            String::from_utf8_lossy(&compile.stderr)
        );

        let run = Command::new(&binary_path)
            .env("OPENIM_API_ADDR", "https://api.openim.test")
            .env("OPENIM_WS_ADDR", spawn_transport_server())
            .env("OPENIM_USER_ID", "u1")
            .env("OPENIM_TOKEN", "token")
            .env("OPENIM_DATA_DIR", temp_dir.join("db"))
            .output()
            .unwrap();
        assert!(
            run.status.success(),
            "desktop example failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&run.stdout),
            String::from_utf8_lossy(&run.stderr)
        );

        let stdout = String::from_utf8_lossy(&run.stdout);
        assert!(stdout.contains("OpenIM session event: listenerRegistered"));
        assert!(stdout.contains("OpenIM session event: initialized {}"));
        assert!(stdout.contains("OpenIM session event: resourceOpened"));
        assert!(stdout.contains("OpenIM session event: loggedIn {\"userId\":\"u1\"}"));
        assert!(stdout.contains("OpenIM session event: resourceClosed"));
        assert!(stdout.contains("OpenIM session event: loggedOut {\"userId\":\"u1\"}"));
        assert!(stdout.contains("OpenIM session event: uninitialized {}"));

        let _ = fs::remove_file(&binary_path);
        let _ = fs::remove_dir_all(&temp_dir);
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("openim-ffi-{label}-{}-{now}", std::process::id()))
    }

    fn spawn_transport_server() -> String {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
                let addr = listener.local_addr().unwrap();
                tx.send(format!("ws://{addr}/msg_gateway")).unwrap();
                let (stream, _) = listener.accept().await.unwrap();
                let mut ws = accept_async(stream).await.unwrap();
                ws.send(WsMessage::Text(r#"{"errCode":0}"#.into()))
                    .await
                    .unwrap();

                while let Some(frame) = ws.next().await {
                    match frame.unwrap() {
                        WsMessage::Text(text) if text.contains(r#""type":"ping""#) => {
                            ws.send(WsMessage::Text(r#"{"type":"pong"}"#.into()))
                                .await
                                .unwrap();
                        }
                        WsMessage::Close(_) => break,
                        _ => {}
                    }
                }
            });
        });
        rx.recv().unwrap()
    }

    fn assert_contains_all(source: &str, needles: &[&str]) {
        for needle in needles {
            assert!(source.contains(needle), "missing {needle}");
        }
    }

    unsafe extern "C" fn collect_event(
        user_data: *mut c_void,
        event: *const c_char,
        payload_json: *const c_char,
    ) {
        let events = &mut *(user_data as *mut Vec<(String, String)>);
        events.push((
            CStr::from_ptr(event).to_str().unwrap().to_string(),
            CStr::from_ptr(payload_json).to_str().unwrap().to_string(),
        ));
    }
}
