use std::{collections::BTreeSet, fs, io, path::Path};

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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoSourceContract {
    pub public_apis: Vec<GoPublicApi>,
    pub listener_contracts: Vec<ListenerContract>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoPublicApi {
    pub name: String,
    pub signature: String,
    pub file: String,
    pub line: usize,
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

pub fn extract_go_source_contract(go_sdk_root: impl AsRef<Path>) -> io::Result<GoSourceContract> {
    let root = go_sdk_root.as_ref();
    let mut public_apis = extract_public_apis(&root.join("open_im_sdk"))?;
    let mut listener_contracts =
        extract_listener_contracts(&root.join("open_im_sdk_callback/callback_client.go"))?;

    public_apis.sort_by(|left, right| left.name.cmp(&right.name));
    listener_contracts.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(GoSourceContract {
        public_apis,
        listener_contracts,
    })
}

fn extract_public_apis(open_im_sdk_dir: &Path) -> io::Result<Vec<GoPublicApi>> {
    let mut files = fs::read_dir(open_im_sdk_dir)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<_>>>()?;
    files.retain(|path| path.extension().is_some_and(|extension| extension == "go"));
    files.sort();

    let mut apis = Vec::new();
    for path in files {
        let text = fs::read_to_string(&path)?;
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        for (line_index, line) in text.lines().enumerate() {
            let Some(name) = public_go_function_name(line) else {
                continue;
            };
            apis.push(GoPublicApi {
                name,
                signature: line.trim().trim_end_matches('{').trim().to_string(),
                file: format!("open_im_sdk/{file_name}"),
                line: line_index + 1,
            });
        }
    }

    Ok(apis)
}

fn public_go_function_name(line: &str) -> Option<String> {
    let rest = line.trim_start().strip_prefix("func ")?;
    let first = rest.chars().next()?;
    if !first.is_ascii_uppercase() {
        return None;
    }
    let name_end = rest.find('(')?;
    let name = &rest[..name_end];
    if name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        Some(name.to_string())
    } else {
        None
    }
}

fn extract_listener_contracts(callback_file: &Path) -> io::Result<Vec<ListenerContract>> {
    let text = fs::read_to_string(callback_file)?;
    let mut contracts = Vec::new();
    let mut current: Option<ListenerContract> = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(contract) = current.as_mut() {
            if trimmed == "}" {
                contracts.push(current.take().expect("current listener exists"));
                continue;
            }
            if !trimmed.is_empty() && !trimmed.starts_with("//") {
                contract.methods.push(trimmed.to_string());
            }
            continue;
        }

        let Some(name) = listener_interface_name(trimmed) else {
            continue;
        };
        current = Some(ListenerContract {
            name,
            methods: Vec::new(),
        });
    }

    Ok(contracts)
}

fn listener_interface_name(line: &str) -> Option<String> {
    let rest = line.strip_prefix("type ")?;
    let name = rest.strip_suffix(" interface {")?;
    let first = name.chars().next()?;
    first.is_ascii_uppercase().then(|| name.to_string())
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
    use std::path::PathBuf;

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

    #[test]
    fn auto_extracts_go_public_api_and_listener_surface_when_source_exists() {
        let Some(root) = available_go_sdk_root() else {
            eprintln!("skipping Go source extraction test: OpenIM Go SDK source is not available");
            return;
        };

        let extracted = extract_go_source_contract(&root).expect("extract Go SDK contract");
        let public_api_names = extracted
            .public_apis
            .iter()
            .map(|api| api.name.as_str())
            .collect::<BTreeSet<_>>();
        let listener_names = extracted
            .listener_contracts
            .iter()
            .map(|contract| contract.name.as_str())
            .collect::<BTreeSet<_>>();

        assert_eq!(
            extracted.public_apis.len(),
            134,
            "unexpected Go SDK public API count from {}",
            root.display()
        );
        assert_eq!(extracted.listener_contracts.len(), 14);

        for required in [
            "InitSDK",
            "Login",
            "Logout",
            "SetConversationListener",
            "SetAdvancedMsgListener",
            "SendMessage",
            "CreateTextMessage",
            "GetAdvancedHistoryMessageList",
            "MarkConversationMessageAsRead",
            "UploadFile",
        ] {
            assert!(
                public_api_names.contains(required),
                "missing extracted public API: {required}"
            );
        }

        for required in [
            "Base",
            "OnConnListener",
            "OnGroupListener",
            "OnFriendshipListener",
            "OnConversationListener",
            "OnAdvancedMsgListener",
            "OnSignalingListener",
            "UploadFileCallback",
        ] {
            assert!(
                listener_names.contains(required),
                "missing extracted listener: {required}"
            );
        }
    }

    #[test]
    fn fixture_seed_is_subset_of_auto_extracted_go_surface_when_source_exists() {
        let Some(root) = available_go_sdk_root() else {
            eprintln!("skipping fixture subset test: OpenIM Go SDK source is not available");
            return;
        };

        let fixture = load_phase0_contract_fixture();
        let extracted = extract_go_source_contract(root).expect("extract Go SDK contract");
        let public_api_names = extracted
            .public_apis
            .iter()
            .map(|api| api.name.as_str())
            .collect::<BTreeSet<_>>();
        let listener_names = extracted
            .listener_contracts
            .iter()
            .map(|contract| contract.name.as_str())
            .collect::<BTreeSet<_>>();

        for contract in &fixture.api_contracts {
            assert!(
                public_api_names.contains(contract.name.as_str()),
                "fixture API is not present in Go SDK source: {}",
                contract.name
            );
        }
        for contract in &fixture.listener_contracts {
            assert!(
                listener_names.contains(contract.name.as_str()),
                "fixture listener is not present in Go SDK source: {}",
                contract.name
            );
        }
    }

    fn available_go_sdk_root() -> Option<PathBuf> {
        std::env::var("OPENIM_GO_SDK_ROOT")
            .ok()
            .map(PathBuf::from)
            .filter(|path| path.join("open_im_sdk").exists())
            .or_else(|| {
                [
                    "/Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core",
                    "/Volumes/ssd - Data/Users/hj/Documents/code/github/openim/openim-sdk-core",
                ]
                .into_iter()
                .map(PathBuf::from)
                .find(|path| path.join("open_im_sdk").exists())
            })
    }
}
