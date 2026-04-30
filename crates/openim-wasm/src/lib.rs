#[cfg(target_arch = "wasm32")]
use std::collections::BTreeMap;

#[cfg(target_arch = "wasm32")]
use js_sys::Function;
use openim_session::{
    map_session_event_payload_to_go_listener_dispatches, LoginCredentials, OpenImSession,
    SessionConfig, SessionState,
};
use openim_types::Platform;
use serde_json::json;
use wasm_bindgen::prelude::*;

const WASM_CALLBACK_THREAD_POLICY: &str = "host_event_loop";
const TRANSPORT_TASK: &str = "transport";
const SYNC_TASK: &str = "sync";

#[wasm_bindgen]
pub struct OpenImWasmSession {
    inner: OpenImSession,
    #[cfg(target_arch = "wasm32")]
    listeners: BTreeMap<u64, Function>,
    #[cfg(target_arch = "wasm32")]
    next_listener_id: u64,
}

#[wasm_bindgen]
impl OpenImWasmSession {
    #[wasm_bindgen(constructor)]
    pub fn new(api_addr: String, ws_addr: String, platform_id: i32) -> Result<Self, JsValue> {
        let platform = Platform::from_i32(platform_id)
            .ok_or_else(|| JsValue::from_str("invalid platform_id"))?;
        let config = SessionConfig::new(platform, api_addr, ws_addr);
        let inner = OpenImSession::new(config).map_err(js_error)?;
        Ok(Self {
            inner,
            #[cfg(target_arch = "wasm32")]
            listeners: BTreeMap::new(),
            #[cfg(target_arch = "wasm32")]
            next_listener_id: 0,
        })
    }

    pub fn init(&mut self) -> Result<(), JsValue> {
        let should_emit = matches!(
            self.inner.state(),
            SessionState::Created | SessionState::Uninitialized
        );
        self.inner.init().map_err(js_error)?;
        if should_emit {
            self.emit_wasm_event("initialized", "{}".to_string());
        }
        Ok(())
    }

    pub fn login(&mut self, user_id: String, token: String) -> Result<(), JsValue> {
        let payload = json!({ "userId": &user_id }).to_string();
        self.inner
            .login(LoginCredentials::new(user_id, token))
            .map_err(js_error)?;
        self.emit_wasm_task_event("taskStarted", TRANSPORT_TASK);
        self.emit_wasm_task_event("taskStarted", SYNC_TASK);
        self.emit_wasm_event("loggedIn", payload);
        Ok(())
    }

    pub fn logout(&mut self) -> Result<(), JsValue> {
        let payload = self
            .inner
            .login_user_id()
            .map(|user_id| json!({ "userId": user_id }).to_string());
        self.inner.logout().map_err(js_error)?;
        if let Some(payload) = payload {
            self.emit_wasm_task_event("taskStopped", SYNC_TASK);
            self.emit_wasm_task_event("taskStopped", TRANSPORT_TASK);
            self.emit_wasm_event("loggedOut", payload);
        }
        Ok(())
    }

    pub fn uninit(&mut self) -> Result<(), JsValue> {
        let should_stop_tasks = self.inner.login_user_id().is_some();
        self.inner.uninit().map_err(js_error)?;
        if should_stop_tasks {
            self.emit_wasm_task_event("taskStopped", SYNC_TASK);
            self.emit_wasm_task_event("taskStopped", TRANSPORT_TASK);
        }
        self.emit_wasm_event("uninitialized", "{}".to_string());
        Ok(())
    }

    #[wasm_bindgen(js_name = stateCode)]
    pub fn state_code(&self) -> i32 {
        state_code(self.inner.state())
    }

    #[wasm_bindgen(js_name = loginUserId)]
    pub fn login_user_id(&self) -> Option<String> {
        self.inner.login_user_id().map(ToOwned::to_owned)
    }

    #[wasm_bindgen(js_name = callbackThreadPolicy)]
    pub fn callback_thread_policy() -> String {
        WASM_CALLBACK_THREAD_POLICY.to_string()
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = addListener)]
    pub fn add_listener(&mut self, callback: Function) -> u64 {
        self.next_listener_id += 1;
        let listener_id = self.next_listener_id;
        self.listeners.insert(listener_id, callback);
        self.emit_wasm_event(
            "listenerRegistered",
            json!({ "listenerId": listener_id }).to_string(),
        );
        listener_id
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = removeListener)]
    pub fn remove_listener(&mut self, listener_id: u64) -> bool {
        let removed = self.listeners.remove(&listener_id).is_some();
        if removed {
            self.emit_wasm_event(
                "listenerUnregistered",
                json!({ "listenerId": listener_id }).to_string(),
            );
        }
        removed
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = listenerCount)]
    pub fn listener_count(&self) -> usize {
        self.listeners.len()
    }
}

#[wasm_bindgen(js_name = mapSessionEventToGoListeners)]
pub fn map_session_event_to_go_listeners(
    event: String,
    payload_json: String,
) -> Result<String, JsValue> {
    let dispatches = map_session_event_payload_to_go_listener_dispatches(&event, &payload_json)
        .map_err(js_error)?;
    serde_json::to_string(&dispatches).map_err(js_error)
}

impl OpenImWasmSession {
    fn emit_wasm_task_event(&self, event: &str, task_name: &str) {
        self.emit_wasm_event(event, json!({ "name": task_name }).to_string());
    }

    #[cfg(target_arch = "wasm32")]
    fn emit_wasm_event(&self, event: &str, payload_json: String) {
        let event = JsValue::from_str(event);
        let payload = JsValue::from_str(&payload_json);
        for callback in self.listeners.values() {
            let _ = callback.call2(&JsValue::NULL, &event, &payload);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn emit_wasm_event(&self, _event: &str, _payload_json: String) {}
}

fn state_code(state: SessionState) -> i32 {
    match state {
        SessionState::Created => 0,
        SessionState::Initialized => 1,
        SessionState::LoggedIn => 2,
        SessionState::LoggedOut => 3,
        SessionState::Uninitialized => 4,
    }
}

fn js_error(error: impl ToString) -> JsValue {
    JsValue::from_str(&error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const WEB_EXAMPLE: &str = include_str!("../../../examples/web/openim_lifecycle.ts");

    #[test]
    fn wasm_session_lifecycle_exports_basic_state() {
        let mut session = OpenImWasmSession::new(
            "https://api.openim.test".to_string(),
            "wss://ws.openim.test".to_string(),
            Platform::Web.as_i32(),
        )
        .unwrap();

        assert_eq!(session.state_code(), 0);
        session.init().unwrap();
        assert_eq!(session.state_code(), 1);
        session
            .login("u1".to_string(), "token".to_string())
            .unwrap();
        assert_eq!(session.state_code(), 2);
        assert_eq!(session.login_user_id(), Some("u1".to_string()));
        session.logout().unwrap();
        assert_eq!(session.state_code(), 3);
        session.uninit().unwrap();
        assert_eq!(session.state_code(), 4);
        assert_eq!(
            OpenImWasmSession::callback_thread_policy(),
            "host_event_loop"
        );
    }

    #[test]
    fn wasm_maps_generic_session_event_to_go_listener_dispatches() {
        let mapped = map_session_event_to_go_listeners(
            "conversationChanged".to_string(),
            r#"{"conversations":[{"conversationId":"c1"}]}"#.to_string(),
        )
        .unwrap();
        let dispatches: serde_json::Value = serde_json::from_str(&mapped).unwrap();
        assert_eq!(dispatches.as_array().unwrap().len(), 1);
        assert_eq!(dispatches[0]["listener"], "OnConversationListener");
        assert_eq!(dispatches[0]["method"], "OnConversationChanged");
        assert_eq!(dispatches[0]["dataJson"], r#"[{"conversationId":"c1"}]"#);
    }

    #[test]
    fn web_example_uses_wasm_lifecycle_exports() {
        for export in [
            "OpenImWasmSession",
            "mapSessionEventToGoListeners",
            "session.addListener",
            "session.init()",
            "session.login",
            "session.logout()",
            "session.uninit()",
            "session.removeListener",
            "session.stateCode()",
        ] {
            assert!(WEB_EXAMPLE.contains(export), "missing {export}");
        }
    }
}
