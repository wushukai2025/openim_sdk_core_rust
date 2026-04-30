use std::collections::BTreeSet;

#[cfg(test)]
use openim_errors::ErrorCode;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ContractFixture {
    pub version: u32,
    pub source: FixtureSource,
    pub api_contracts: Vec<ApiContract>,
    pub listener_contracts: Vec<ListenerContract>,
    pub event_scenarios: Vec<EventScenario>,
    pub error_contracts: Vec<ErrorContract>,
    pub required_scenarios: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct FixtureSource {
    pub go_sdk_root: String,
    pub captured_from: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApiContract {
    pub name: String,
    pub category: String,
    pub callback: Option<String>,
    pub return_kind: String,
    pub requires_login: bool,
}

#[derive(Debug, Deserialize)]
pub struct ListenerContract {
    pub name: String,
    pub methods: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct EventScenario {
    pub name: String,
    pub required_order: Vec<String>,
    #[serde(default)]
    pub optional_events: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ErrorContract {
    pub name: String,
    pub code: i32,
    pub category: String,
}

pub fn load_phase0_contract_fixture() -> ContractFixture {
    serde_json::from_str(include_str!("../fixtures/phase0_contract_baseline.json"))
        .expect("phase0 contract fixture must be valid JSON")
}

pub fn validate_fixture(fixture: &ContractFixture) {
    assert_eq!(fixture.version, 1);
    assert!(
        fixture.source.go_sdk_root.ends_with("openim-sdk-core"),
        "fixture must identify the Go SDK source root"
    );
    assert_non_empty_unique(
        "api contract",
        fixture.api_contracts.iter().map(|contract| &contract.name),
    );
    assert_non_empty_unique(
        "listener contract",
        fixture
            .listener_contracts
            .iter()
            .map(|contract| &contract.name),
    );
    assert_non_empty_unique(
        "event scenario",
        fixture
            .event_scenarios
            .iter()
            .map(|scenario| &scenario.name),
    );
    assert_non_empty_unique(
        "error contract",
        fixture
            .error_contracts
            .iter()
            .map(|contract| &contract.name),
    );
}

fn assert_non_empty_unique<'a>(label: &str, values: impl Iterator<Item = &'a String>) {
    let mut seen = BTreeSet::new();
    for value in values {
        assert!(!value.is_empty(), "{label} contains empty value");
        assert!(seen.insert(value), "{label} has duplicate value: {value}");
    }
    assert!(!seen.is_empty(), "{label} list is empty");
}

#[cfg(test)]
fn rust_error_code(name: &str) -> Option<ErrorCode> {
    Some(match name {
        "NetworkError" => ErrorCode::NETWORK,
        "NetworkTimeoutError" => ErrorCode::NETWORK_TIMEOUT,
        "ArgsError" => ErrorCode::ARGS,
        "CtxDeadlineExceededError" => ErrorCode::CTX_DEADLINE_EXCEEDED,
        "UnknownCode" => ErrorCode::UNKNOWN,
        "SdkInternalError" => ErrorCode::SDK_INTERNAL,
        "NoUpdateError" => ErrorCode::NO_UPDATE,
        "SDKNotInitError" => ErrorCode::SDK_NOT_INIT,
        "SDKNotLoginError" => ErrorCode::SDK_NOT_LOGIN,
        "UserIDNotFoundError" => ErrorCode::USER_ID_NOT_FOUND,
        "LoginOutError" => ErrorCode::LOGIN_OUT,
        "LoginRepeatError" => ErrorCode::LOGIN_REPEAT,
        "FileNotFoundError" => ErrorCode::FILE_NOT_FOUND,
        "MsgDeCompressionError" => ErrorCode::MSG_DECOMPRESSION,
        "MsgDecodeBinaryWsError" => ErrorCode::MSG_DECODE_BINARY_WS,
        "MsgBinaryTypeNotSupportError" => ErrorCode::MSG_BINARY_TYPE_NOT_SUPPORT,
        "MsgRepeatError" => ErrorCode::MSG_REPEAT,
        "MsgContentTypeNotSupportError" => ErrorCode::MSG_CONTENT_TYPE_NOT_SUPPORT,
        "MsgHasNoSeqError" => ErrorCode::MSG_HAS_NO_SEQ,
        "MsgHasDeletedError" => ErrorCode::MSG_HAS_DELETED,
        "NotSupportOptError" => ErrorCode::NOT_SUPPORT_OPT,
        "NotSupportTypeError" => ErrorCode::NOT_SUPPORT_TYPE,
        "UnreadCountError" => ErrorCode::UNREAD_COUNT,
        "GroupIDNotFoundError" => ErrorCode::GROUP_ID_NOT_FOUND,
        "GroupTypeErr" => ErrorCode::GROUP_TYPE,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase0_contract_fixture_is_well_formed() {
        let fixture = load_phase0_contract_fixture();

        validate_fixture(&fixture);
    }

    #[test]
    fn fixture_covers_required_core_scenarios() {
        let fixture = load_phase0_contract_fixture();
        let scenarios = fixture
            .event_scenarios
            .iter()
            .map(|scenario| scenario.name.as_str())
            .collect::<BTreeSet<_>>();

        for required in &fixture.required_scenarios {
            assert!(
                scenarios.contains(required.as_str()),
                "missing required scenario: {required}"
            );
        }
    }

    #[test]
    fn fixture_covers_required_listener_surfaces() {
        let fixture = load_phase0_contract_fixture();
        let listeners = fixture
            .listener_contracts
            .iter()
            .map(|contract| contract.name.as_str())
            .collect::<BTreeSet<_>>();

        for required in [
            "Base",
            "OnConnListener",
            "OnConversationListener",
            "OnAdvancedMsgListener",
            "UploadFileCallback",
            "UploadLogProgress",
        ] {
            assert!(listeners.contains(required), "missing listener: {required}");
        }
    }

    #[test]
    fn fixture_covers_required_public_api_surfaces() {
        let fixture = load_phase0_contract_fixture();
        let apis = fixture
            .api_contracts
            .iter()
            .map(|contract| contract.name.as_str())
            .collect::<BTreeSet<_>>();

        for required in [
            "InitSDK",
            "Login",
            "Logout",
            "SetConversationListener",
            "SetAdvancedMsgListener",
            "SendMessage",
            "UploadFile",
        ] {
            assert!(apis.contains(required), "missing public API: {required}");
        }
    }

    #[test]
    fn go_error_codes_match_rust_error_constants() {
        let fixture = load_phase0_contract_fixture();

        for contract in &fixture.error_contracts {
            let rust_code = rust_error_code(&contract.name)
                .unwrap_or_else(|| panic!("missing Rust error mapping: {}", contract.name));
            assert_eq!(
                rust_code.as_i32(),
                contract.code,
                "error code mismatch for {}",
                contract.name
            );
        }
    }

    #[test]
    fn event_sequences_preserve_critical_order() {
        let fixture = load_phase0_contract_fixture();
        let login = fixture
            .event_scenarios
            .iter()
            .find(|scenario| scenario.name == "login_sync_message")
            .expect("login_sync_message scenario");

        assert_eq!(login.required_order.first().unwrap(), "OnConnecting");
        assert!(
            login
                .required_order
                .iter()
                .position(|event| event == "OnConnectSuccess")
                < login
                    .required_order
                    .iter()
                    .position(|event| event == "OnSyncServerStart"),
            "sync must not start before connection success"
        );
    }
}
