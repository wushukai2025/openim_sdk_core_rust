use openim_session::{LoginCredentials, OpenImSession, SessionConfig, SessionState};
use openim_types::Platform;
use wasm_bindgen::prelude::*;

const WASM_CALLBACK_THREAD_POLICY: &str = "host_event_loop";

#[wasm_bindgen]
pub struct OpenImWasmSession {
    inner: OpenImSession,
}

#[wasm_bindgen]
impl OpenImWasmSession {
    #[wasm_bindgen(constructor)]
    pub fn new(api_addr: String, ws_addr: String, platform_id: i32) -> Result<Self, JsValue> {
        let platform = Platform::from_i32(platform_id)
            .ok_or_else(|| JsValue::from_str("invalid platform_id"))?;
        let config = SessionConfig::new(platform, api_addr, ws_addr);
        let inner = OpenImSession::new(config).map_err(js_error)?;
        Ok(Self { inner })
    }

    pub fn init(&mut self) -> Result<(), JsValue> {
        self.inner.init().map_err(js_error)
    }

    pub fn login(&mut self, user_id: String, token: String) -> Result<(), JsValue> {
        self.inner
            .login(LoginCredentials::new(user_id, token))
            .map_err(js_error)
    }

    pub fn logout(&mut self) -> Result<(), JsValue> {
        self.inner.logout().map_err(js_error)
    }

    pub fn uninit(&mut self) -> Result<(), JsValue> {
        self.inner.uninit().map_err(js_error)
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
    fn web_example_uses_wasm_lifecycle_exports() {
        for export in [
            "OpenImWasmSession",
            "session.init()",
            "session.login",
            "session.logout()",
            "session.uninit()",
            "session.stateCode()",
        ] {
            assert!(WEB_EXAMPLE.contains(export), "missing {export}");
        }
    }
}
