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

#[derive(Debug, Clone, Deserialize)]
pub struct ReplayEvent {
    pub scenario: String,
    pub listener: String,
    pub method: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

pub const NATIVE_CALLBACK_THREAD: &str = "sdk_serialized_callback_queue";
pub const WASM_CALLBACK_THREAD: &str = "host_event_loop";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingCallbackContract {
    pub listener: String,
    pub method: String,
    pub native_c_abi: String,
    pub wasm: String,
    pub native_thread: &'static str,
    pub wasm_thread: &'static str,
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

pub fn load_replay_events(path: impl AsRef<Path>) -> io::Result<Vec<ReplayEvent>> {
    let text = fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
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

pub fn validate_replay_transcript(
    fixture: &ContractFixture,
    events: &[ReplayEvent],
) -> Result<(), String> {
    for scenario in &fixture.required_scenarios {
        validate_replay_scenario(fixture, scenario, events)?;
    }
    Ok(())
}

pub fn validate_replay_scenario(
    fixture: &ContractFixture,
    scenario_name: &str,
    events: &[ReplayEvent],
) -> Result<(), String> {
    let scenario = fixture
        .event_scenarios
        .iter()
        .find(|scenario| scenario.name == scenario_name)
        .ok_or_else(|| format!("unknown replay scenario: {scenario_name}"))?;
    let observed = replay_event_names(events, scenario_name);
    if observed.is_empty() {
        return Err(format!(
            "missing replay events for scenario: {scenario_name}"
        ));
    }

    let allowed = scenario
        .required_order
        .iter()
        .chain(scenario.optional_events.iter())
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    for event in &observed {
        if !allowed.contains(event.as_str()) {
            return Err(format!(
                "unexpected event {event} in replay scenario {scenario_name}"
            ));
        }
    }

    let mut cursor = 0;
    for required in &scenario.required_order {
        let Some(offset) = observed[cursor..]
            .iter()
            .position(|event| event == required)
        else {
            return Err(format!(
                "missing required event {required} in replay scenario {scenario_name}"
            ));
        };
        cursor += offset + 1;
    }

    Ok(())
}

pub fn compare_replay_scenario(
    scenario_name: &str,
    go_events: &[ReplayEvent],
    rust_events: &[ReplayEvent],
) -> Result<(), String> {
    let go_order = replay_event_names(go_events, scenario_name);
    let rust_order = replay_event_names(rust_events, scenario_name);
    if go_order.is_empty() {
        return Err(format!(
            "missing Go replay events for scenario: {scenario_name}"
        ));
    }
    if rust_order.is_empty() {
        return Err(format!(
            "missing Rust replay events for scenario: {scenario_name}"
        ));
    }
    if go_order != rust_order {
        return Err(format!(
            "replay event order mismatch for {scenario_name}: go={go_order:?} rust={rust_order:?}"
        ));
    }
    Ok(())
}

fn replay_event_names(events: &[ReplayEvent], scenario_name: &str) -> Vec<String> {
    events
        .iter()
        .filter(|event| event.scenario == scenario_name)
        .map(replay_event_name)
        .collect()
}

fn replay_event_name(event: &ReplayEvent) -> String {
    if event.method.contains('.') {
        return event.method.clone();
    }
    if event.listener == "Base" {
        return format!("Base.{}", event.method);
    }
    event.method.clone()
}

pub fn binding_callback_contracts(
    listener_contracts: &[ListenerContract],
) -> Vec<BindingCallbackContract> {
    let mut callbacks = Vec::new();
    for listener in listener_contracts {
        for signature in &listener.methods {
            let Some(method) = listener_method_name(signature) else {
                continue;
            };
            callbacks.push(BindingCallbackContract {
                native_c_abi: format!(
                    "openim_{}_{}",
                    to_snake_case(&listener.name),
                    to_snake_case(method)
                ),
                wasm: to_lower_camel(method),
                listener: listener.name.clone(),
                method: method.to_string(),
                native_thread: NATIVE_CALLBACK_THREAD,
                wasm_thread: WASM_CALLBACK_THREAD,
            });
        }
    }
    callbacks.sort_by(|left, right| {
        left.listener
            .cmp(&right.listener)
            .then_with(|| left.method.cmp(&right.method))
    });
    callbacks
}

pub fn validate_binding_callback_contracts(
    callbacks: &[BindingCallbackContract],
) -> Result<(), String> {
    if callbacks.is_empty() {
        return Err("binding callback contract list is empty".to_string());
    }

    let mut native_names = BTreeSet::new();
    for callback in callbacks {
        if callback.listener.is_empty() || callback.method.is_empty() {
            return Err("binding callback contains empty listener or method".to_string());
        }
        if !callback.native_c_abi.starts_with("openim_") {
            return Err(format!(
                "native callback name must use openim_ prefix: {}",
                callback.native_c_abi
            ));
        }
        if !native_names.insert(callback.native_c_abi.as_str()) {
            return Err(format!(
                "duplicate native callback name: {}",
                callback.native_c_abi
            ));
        }
        if callback.wasm.is_empty()
            || !callback
                .wasm
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_lowercase())
        {
            return Err(format!(
                "wasm callback name must be lowerCamelCase: {}",
                callback.wasm
            ));
        }
        if callback.native_thread != NATIVE_CALLBACK_THREAD {
            return Err(format!(
                "unexpected native callback thread policy: {}",
                callback.native_thread
            ));
        }
        if callback.wasm_thread != WASM_CALLBACK_THREAD {
            return Err(format!(
                "unexpected wasm callback thread policy: {}",
                callback.wasm_thread
            ));
        }
    }

    Ok(())
}

fn listener_method_name(signature: &str) -> Option<&str> {
    let end = signature.find('(')?;
    let name = signature[..end].trim();
    (!name.is_empty()).then_some(name)
}

fn to_lower_camel(name: &str) -> String {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    first.to_ascii_lowercase().to_string() + chars.as_str()
}

fn to_snake_case(name: &str) -> String {
    let mut out = String::new();
    let mut prev_was_lower_or_digit = false;
    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            if prev_was_lower_or_digit {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            prev_was_lower_or_digit = false;
        } else if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_was_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else if !out.ends_with('_') {
            out.push('_');
            prev_was_lower_or_digit = false;
        }
    }
    out.trim_matches('_').to_string()
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
    fn replay_transcript_validates_fixture_required_order() {
        let fixture = load_phase0_contract_fixture();
        let events = minimal_required_replay_events(&fixture);

        validate_replay_transcript(&fixture, &events).expect("valid replay transcript");
    }

    #[test]
    fn replay_transcript_rejects_missing_required_event() {
        let fixture = load_phase0_contract_fixture();
        let mut events = minimal_required_replay_events(&fixture);
        events.retain(|event| {
            !(event.scenario == "login_sync_message" && event.method == "OnConnectSuccess")
        });

        let err = validate_replay_transcript(&fixture, &events)
            .expect_err("missing required event should fail");
        assert!(
            err.contains("OnConnectSuccess"),
            "unexpected validation error: {err}"
        );
    }

    #[test]
    fn go_and_rust_replay_sequences_can_be_compared() {
        let fixture = load_phase0_contract_fixture();
        let go_events = minimal_required_replay_events(&fixture);
        let rust_events = go_events.clone();

        for scenario in &fixture.required_scenarios {
            compare_replay_scenario(scenario, &go_events, &rust_events)
                .expect("matching replay sequence");
        }
    }

    #[test]
    #[ignore = "requires OPENIM_GO_REPLAY_EVENTS captured from a real Go SDK run"]
    fn real_go_sdk_replay_transcript_matches_phase0_contract() {
        let Some(path) = replay_events_path("OPENIM_GO_REPLAY_EVENTS") else {
            eprintln!("skipping Go replay transcript test: OPENIM_GO_REPLAY_EVENTS is not set");
            return;
        };

        let fixture = load_phase0_contract_fixture();
        let events = load_replay_events(path).expect("load Go replay transcript");

        validate_replay_transcript(&fixture, &events).expect("valid Go replay transcript");
    }

    #[test]
    #[ignore = "requires OPENIM_RUST_REPLAY_EVENTS captured from a real Rust SDK run"]
    fn real_rust_replay_transcript_matches_phase0_contract() {
        let Some(path) = replay_events_path("OPENIM_RUST_REPLAY_EVENTS") else {
            eprintln!("skipping Rust replay transcript test: OPENIM_RUST_REPLAY_EVENTS is not set");
            return;
        };

        let fixture = load_phase0_contract_fixture();
        let events = load_replay_events(path).expect("load Rust replay transcript");

        validate_replay_transcript(&fixture, &events).expect("valid Rust replay transcript");
    }

    #[test]
    #[ignore = "requires OPENIM_GO_REPLAY_EVENTS and OPENIM_RUST_REPLAY_EVENTS"]
    fn real_go_and_rust_replay_transcripts_match_phase0_sequences() {
        let Some(go_path) = replay_events_path("OPENIM_GO_REPLAY_EVENTS") else {
            eprintln!("skipping replay comparison test: OPENIM_GO_REPLAY_EVENTS is not set");
            return;
        };
        let Some(rust_path) = replay_events_path("OPENIM_RUST_REPLAY_EVENTS") else {
            eprintln!("skipping replay comparison test: OPENIM_RUST_REPLAY_EVENTS is not set");
            return;
        };

        let fixture = load_phase0_contract_fixture();
        let go_events = load_replay_events(go_path).expect("load Go replay transcript");
        let rust_events = load_replay_events(rust_path).expect("load Rust replay transcript");

        validate_replay_transcript(&fixture, &go_events).expect("valid Go replay transcript");
        validate_replay_transcript(&fixture, &rust_events).expect("valid Rust replay transcript");
        for scenario in &fixture.required_scenarios {
            compare_replay_scenario(scenario, &go_events, &rust_events)
                .expect("matching Go/Rust replay sequence");
        }
    }

    #[test]
    fn binding_callback_contract_covers_fixture_listener_methods() {
        let fixture = load_phase0_contract_fixture();
        let callbacks = binding_callback_contracts(&fixture.listener_contracts);
        let expected_count = fixture
            .listener_contracts
            .iter()
            .flat_map(|listener| &listener.methods)
            .filter(|method| method.contains('('))
            .count();

        assert_eq!(callbacks.len(), expected_count);
        validate_binding_callback_contracts(&callbacks).expect("valid binding callback contract");
    }

    #[test]
    fn binding_callback_contract_freezes_seed_names_and_threads() {
        let fixture = load_phase0_contract_fixture();
        let callbacks = binding_callback_contracts(&fixture.listener_contracts);

        assert_binding_callback(
            &callbacks,
            "OnConnListener",
            "OnConnectSuccess",
            "openim_on_conn_listener_on_connect_success",
            "onConnectSuccess",
        );
        assert_binding_callback(
            &callbacks,
            "Base",
            "OnError",
            "openim_base_on_error",
            "onError",
        );
        assert_binding_callback(
            &callbacks,
            "UploadFileCallback",
            "UploadPartComplete",
            "openim_upload_file_callback_upload_part_complete",
            "uploadPartComplete",
        );
    }

    #[test]
    fn binding_callback_contract_covers_auto_extracted_listener_methods_when_source_exists() {
        let Some(root) = available_go_sdk_root() else {
            eprintln!(
                "skipping binding callback extraction test: OpenIM Go SDK source is not available"
            );
            return;
        };

        let extracted = extract_go_source_contract(root).expect("extract Go SDK contract");
        let callbacks = binding_callback_contracts(&extracted.listener_contracts);

        assert_eq!(callbacks.len(), 71);
        validate_binding_callback_contracts(&callbacks).expect("valid binding callback contract");
        assert!(
            callbacks
                .iter()
                .any(|callback| callback.listener == "OnSignalingListener"
                    && callback.method == "OnReceiveNewInvitation"
                    && callback.native_c_abi
                        == "openim_on_signaling_listener_on_receive_new_invitation"
                    && callback.wasm == "onReceiveNewInvitation"),
            "missing signaling callback binding contract"
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

    fn minimal_required_replay_events(fixture: &ContractFixture) -> Vec<ReplayEvent> {
        let mut events = Vec::new();
        for scenario in &fixture.event_scenarios {
            for method in &scenario.required_order {
                let (listener, method) = method
                    .strip_prefix("Base.")
                    .map(|method| ("Base", method))
                    .unwrap_or(("RecordedListener", method.as_str()));
                events.push(replay_event(&scenario.name, listener, method));
            }
        }
        events
    }

    fn replay_event(scenario: &str, listener: &str, method: &str) -> ReplayEvent {
        ReplayEvent {
            scenario: scenario.to_string(),
            listener: listener.to_string(),
            method: method.to_string(),
            payload: serde_json::Value::Null,
        }
    }

    fn replay_events_path(name: &str) -> Option<PathBuf> {
        std::env::var_os(name).map(PathBuf::from)
    }

    fn assert_binding_callback(
        callbacks: &[BindingCallbackContract],
        listener: &str,
        method: &str,
        native_c_abi: &str,
        wasm: &str,
    ) {
        let callback = callbacks
            .iter()
            .find(|callback| callback.listener == listener && callback.method == method)
            .unwrap_or_else(|| panic!("missing binding callback: {listener}.{method}"));

        assert_eq!(callback.native_c_abi, native_c_abi);
        assert_eq!(callback.wasm, wasm);
        assert_eq!(callback.native_thread, NATIVE_CALLBACK_THREAD);
        assert_eq!(callback.wasm_thread, WASM_CALLBACK_THREAD);
    }
}
