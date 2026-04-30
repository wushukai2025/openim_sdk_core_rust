use std::{
    collections::BTreeSet,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
};

use anyhow::{anyhow, Context, Result};
use clap::{Args, Parser, Subcommand};
use openim_compat_tests::{
    compare_replay_scenario, load_phase0_contract_fixture, load_replay_events,
    validate_replay_transcript, ReplayEvent,
};
use openim_session::{LoginCredentials, OpenImSession, SessionConfig, SessionEvent};
use openim_types::Platform;
use serde_json::json;

#[derive(Debug, Parser)]
#[command(version, about = "OpenIM Phase 0 replay transcript capture helper")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Compare(CompareArgs),
    CaptureCommand(CaptureCommandArgs),
    CaptureJsonl(CaptureJsonlArgs),
    CaptureRustSession(CaptureRustSessionArgs),
    CheckRealGate(CheckRealGateArgs),
    Validate(ValidateArgs),
}

const REAL_GATE_REQUIRED_ENV: &[&str] = &[
    "OPENIM_API_ADDR",
    "OPENIM_WS_ADDR",
    "OPENIM_USER_ID",
    "OPENIM_TOKEN",
];

const REAL_GATE_TRANSCRIPT_ENV: &[&str] = &["OPENIM_GO_REPLAY_EVENTS", "OPENIM_RUST_REPLAY_EVENTS"];

#[derive(Debug, Args)]
struct CompareArgs {
    #[arg(long, env = "OPENIM_GO_REPLAY_EVENTS")]
    go_events: PathBuf,
    #[arg(long, env = "OPENIM_RUST_REPLAY_EVENTS")]
    rust_events: PathBuf,
}

#[derive(Debug, Args)]
struct CaptureCommandArgs {
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
    command: Vec<String>,
}

#[derive(Debug, Args)]
struct CaptureJsonlArgs {
    #[arg(long)]
    input: Option<PathBuf>,
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct CaptureRustSessionArgs {
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long, default_value = "rust_session_lifecycle")]
    scenario: String,
    #[arg(long, default_value = "https://api.openim.test")]
    api_addr: String,
    #[arg(long, default_value = "wss://ws.openim.test")]
    ws_addr: String,
    #[arg(long, default_value = "u1")]
    user_id: String,
    #[arg(long, default_value = "token")]
    token: String,
    #[arg(long, default_value_t = Platform::Web.as_i32())]
    platform_id: i32,
}

#[derive(Debug, Args)]
struct CheckRealGateArgs {
    #[arg(long, default_value = "tools/go-phase0-replay")]
    go_harness: PathBuf,
    #[arg(long)]
    require_transcripts: bool,
}

#[derive(Debug, Args)]
struct ValidateArgs {
    #[arg(long, env = "OPENIM_REPLAY_EVENTS")]
    events: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Compare(args) => compare_transcripts(args),
        Command::CaptureCommand(args) => capture_command(args),
        Command::CaptureJsonl(args) => capture_jsonl(args),
        Command::CaptureRustSession(args) => capture_rust_session(args),
        Command::CheckRealGate(args) => check_real_gate(args),
        Command::Validate(args) => validate_transcript(args),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct RealGateStatus {
    go_harness_exists: bool,
    go_tool_available: bool,
    missing_required_env: Vec<&'static str>,
    missing_transcript_env: Vec<&'static str>,
    require_transcripts: bool,
}

impl RealGateStatus {
    fn is_ready(&self) -> bool {
        self.go_harness_exists
            && self.go_tool_available
            && self.missing_required_env.is_empty()
            && (!self.require_transcripts || self.missing_transcript_env.is_empty())
    }
}

fn check_real_gate(args: CheckRealGateArgs) -> Result<()> {
    let status = real_gate_status(
        &args.go_harness,
        args.require_transcripts,
        |name| std::env::var(name).ok(),
        command_available("go"),
    );

    println!("phase0_real_gate_ready={}", status.is_ready());
    println!(
        "go_harness={} path={}",
        status_label(status.go_harness_exists),
        args.go_harness.display()
    );
    println!("go_tool={}", status_label(status.go_tool_available));
    println!(
        "required_env_missing={}",
        env_list(&status.missing_required_env)
    );
    if args.require_transcripts {
        println!(
            "transcript_env_missing={}",
            env_list(&status.missing_transcript_env)
        );
    } else {
        println!("transcript_env_missing=not_required");
    }

    if !status.is_ready() {
        return Err(anyhow!("phase0 real gate is not ready"));
    }
    Ok(())
}

fn real_gate_status<F>(
    go_harness: &Path,
    require_transcripts: bool,
    lookup: F,
    go_tool_available: bool,
) -> RealGateStatus
where
    F: Fn(&str) -> Option<String>,
{
    let missing_required_env = missing_env(REAL_GATE_REQUIRED_ENV, &lookup);
    let missing_transcript_env = if require_transcripts {
        missing_env(REAL_GATE_TRANSCRIPT_ENV, &lookup)
    } else {
        Vec::new()
    };

    RealGateStatus {
        go_harness_exists: go_harness.exists(),
        go_tool_available,
        missing_required_env,
        missing_transcript_env,
        require_transcripts,
    }
}

fn missing_env<F>(names: &'static [&'static str], lookup: &F) -> Vec<&'static str>
where
    F: Fn(&str) -> Option<String>,
{
    names
        .iter()
        .copied()
        .filter(|name| {
            lookup(name)
                .map(|value| value.trim().is_empty())
                .unwrap_or(true)
        })
        .collect()
}

fn command_available(program: &str) -> bool {
    ProcessCommand::new(program)
        .arg("version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn status_label(ok: bool) -> &'static str {
    if ok {
        "ok"
    } else {
        "missing"
    }
}

fn env_list(values: &[&str]) -> String {
    if values.is_empty() {
        return "none".to_string();
    }
    values.join(",")
}

fn compare_transcripts(args: CompareArgs) -> Result<()> {
    let fixture = load_phase0_contract_fixture();
    let go_events = load_replay_events(&args.go_events).with_context(|| {
        format!(
            "load Go replay transcript failed: {}",
            args.go_events.display()
        )
    })?;
    let rust_events = load_replay_events(&args.rust_events).with_context(|| {
        format!(
            "load Rust replay transcript failed: {}",
            args.rust_events.display()
        )
    })?;

    validate_replay_transcript(&fixture, &go_events)
        .map_err(|err| anyhow!("invalid Go replay transcript: {err}"))?;
    validate_replay_transcript(&fixture, &rust_events)
        .map_err(|err| anyhow!("invalid Rust replay transcript: {err}"))?;
    for scenario in &fixture.required_scenarios {
        compare_replay_scenario(scenario, &go_events, &rust_events)
            .map_err(|err| anyhow!("replay transcript mismatch: {err}"))?;
    }

    println!(
        "matching replay transcripts scenarios={} go_events={} rust_events={}",
        fixture.required_scenarios.len(),
        go_events.len(),
        rust_events.len()
    );
    Ok(())
}

fn capture_command(args: CaptureCommandArgs) -> Result<()> {
    let (program, command_args) = args
        .command
        .split_first()
        .ok_or_else(|| anyhow!("capture command is empty"))?;
    let output = ProcessCommand::new(program)
        .args(command_args)
        .output()
        .with_context(|| format!("run replay command failed: {program}"))?;
    if !output.status.success() {
        return Err(anyhow!(
            "replay command exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8(output.stdout).context("replay command stdout is not UTF-8")?;
    let events = parse_jsonl_events(&stdout)?;
    let transcript = serde_json::to_string_pretty(&events)? + "\n";
    write_output(args.output.as_deref(), &transcript)
}

fn capture_jsonl(args: CaptureJsonlArgs) -> Result<()> {
    let input = read_input(args.input.as_deref())?;
    let events = parse_jsonl_events(&input)?;
    let output = serde_json::to_string_pretty(&events)? + "\n";
    write_output(args.output.as_deref(), &output)?;
    Ok(())
}

fn capture_rust_session(args: CaptureRustSessionArgs) -> Result<()> {
    let events = capture_rust_session_events(&args)?;
    let transcript = serde_json::to_string_pretty(&events)? + "\n";
    write_output(args.output.as_deref(), &transcript)
}

fn validate_transcript(args: ValidateArgs) -> Result<()> {
    let fixture = load_phase0_contract_fixture();
    let events = load_replay_events(&args.events)
        .with_context(|| format!("load replay transcript failed: {}", args.events.display()))?;

    validate_replay_transcript(&fixture, &events)
        .map_err(|err| anyhow!("invalid replay transcript: {err}"))?;

    println!(
        "valid replay transcript events={} scenarios={}",
        events.len(),
        scenario_count(&events)
    );
    Ok(())
}

fn read_input(path: Option<&Path>) -> Result<String> {
    if let Some(path) = path {
        return fs::read_to_string(path)
            .with_context(|| format!("read replay jsonl failed: {}", path.display()));
    }

    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .context("read replay jsonl from stdin failed")?;
    Ok(input)
}

fn write_output(path: Option<&Path>, output: &str) -> Result<()> {
    if let Some(path) = path {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create output dir failed: {}", parent.display()))?;
        }
        return fs::write(path, output)
            .with_context(|| format!("write replay transcript failed: {}", path.display()));
    }

    io::stdout()
        .write_all(output.as_bytes())
        .context("write replay transcript to stdout failed")
}

fn parse_jsonl_events(input: &str) -> Result<Vec<ReplayEvent>> {
    let mut events = Vec::new();
    for (line_index, line) in input.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let event = serde_json::from_str::<ReplayEvent>(line)
            .with_context(|| format!("invalid replay event json at line {}", line_index + 1))?;
        events.push(event);
    }
    if events.is_empty() {
        return Err(anyhow!("replay jsonl contains no events"));
    }
    Ok(events)
}

fn scenario_count(events: &[ReplayEvent]) -> usize {
    events
        .iter()
        .map(|event| event.scenario.as_str())
        .collect::<BTreeSet<_>>()
        .len()
}

fn capture_rust_session_events(args: &CaptureRustSessionArgs) -> Result<Vec<ReplayEvent>> {
    let platform = Platform::from_i32(args.platform_id)
        .ok_or_else(|| anyhow!("invalid platform_id: {}", args.platform_id))?;
    let config = SessionConfig::new(platform, args.api_addr.clone(), args.ws_addr.clone());
    let credentials = LoginCredentials::new(args.user_id.clone(), args.token.clone());
    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::<ReplayEvent>::new()));
    let captured = events.clone();
    let scenario = args.scenario.clone();
    let mut session = OpenImSession::new(config)?;

    session.register_listener(move |event| {
        captured
            .lock()
            .expect("capture mutex")
            .push(session_event_to_replay_event(&scenario, event));
    });
    session.init()?;
    session.login(credentials)?;
    session.logout()?;
    session.uninit()?;

    let events = events.lock().expect("capture mutex").clone();
    if events.is_empty() {
        return Err(anyhow!("rust session emitted no events"));
    }
    Ok(events)
}

fn session_event_to_replay_event(scenario: &str, event: &SessionEvent) -> ReplayEvent {
    let (method, payload) = match event {
        SessionEvent::Initialized => ("Initialized", serde_json::Value::Null),
        SessionEvent::LoggedIn { user_id } => ("LoggedIn", json!({ "userID": user_id })),
        SessionEvent::LoggedOut { user_id } => ("LoggedOut", json!({ "userID": user_id })),
        SessionEvent::Uninitialized => ("Uninitialized", serde_json::Value::Null),
        SessionEvent::ListenerRegistered { listener_id } => {
            ("ListenerRegistered", json!({ "listenerID": listener_id }))
        }
        SessionEvent::ListenerUnregistered { listener_id } => {
            ("ListenerUnregistered", json!({ "listenerID": listener_id }))
        }
        SessionEvent::TaskStarted { name } => ("TaskStarted", json!({ "name": name })),
        SessionEvent::TaskStopped { name } => ("TaskStopped", json!({ "name": name })),
        SessionEvent::NewMessages { messages } => {
            ("NewMessages", json!({ "count": messages.len() }))
        }
        SessionEvent::ConversationChanged { conversations } => (
            "ConversationChanged",
            json!({ "count": conversations.len() }),
        ),
    };

    ReplayEvent {
        scenario: scenario.to_string(),
        listener: "RustSession".to_string(),
        method: method.to_string(),
        payload,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openim_compat_tests::validate_replay_scenario;

    #[test]
    fn parses_jsonl_events_and_counts_scenarios() {
        let events = parse_jsonl_events(
            r#"
{"scenario":"connection_status","listener":"OnConnListener","method":"OnConnecting"}
{"scenario":"connection_status","listener":"OnConnListener","method":"OnConnectSuccess"}
            "#,
        )
        .expect("parse replay jsonl");

        assert_eq!(events.len(), 2);
        assert_eq!(scenario_count(&events), 1);
    }

    #[test]
    fn parsed_events_validate_against_phase0_scenario() {
        let fixture = load_phase0_contract_fixture();
        let events = parse_jsonl_events(
            r#"
{"scenario":"connection_status","listener":"OnConnListener","method":"OnConnecting"}
{"scenario":"connection_status","listener":"OnConnListener","method":"OnConnectSuccess"}
            "#,
        )
        .expect("parse replay jsonl");

        validate_replay_scenario(&fixture, "connection_status", &events)
            .expect("valid connection_status replay");
    }

    #[test]
    fn rejects_empty_jsonl() {
        let err = parse_jsonl_events("\n\n").expect_err("empty jsonl should fail");

        assert!(err.to_string().contains("contains no events"));
    }

    #[test]
    fn captures_rust_session_lifecycle_events() {
        let args = CaptureRustSessionArgs {
            output: None,
            scenario: "rust_session_lifecycle".to_string(),
            api_addr: "https://api.openim.test".to_string(),
            ws_addr: "wss://ws.openim.test".to_string(),
            user_id: "u1".to_string(),
            token: "token".to_string(),
            platform_id: Platform::Web.as_i32(),
        };

        let events = capture_rust_session_events(&args).expect("capture rust session");
        let methods = events
            .iter()
            .map(|event| event.method.as_str())
            .collect::<Vec<_>>();

        assert!(methods.contains(&"Initialized"));
        assert!(methods.contains(&"LoggedIn"));
        assert!(methods.contains(&"TaskStarted"));
        assert!(methods.contains(&"TaskStopped"));
        assert!(methods.contains(&"LoggedOut"));
        assert!(methods.contains(&"Uninitialized"));
        assert!(events
            .iter()
            .all(|event| event.scenario == "rust_session_lifecycle"
                && event.listener == "RustSession"));
    }

    #[test]
    fn compares_matching_phase0_transcript_files() {
        let fixture = load_phase0_contract_fixture();
        let events = minimal_required_events(&fixture);
        let go_path = temp_path("go-events");
        let rust_path = temp_path("rust-events");
        fs::write(&go_path, serde_json::to_string(&events).unwrap()).unwrap();
        fs::write(&rust_path, serde_json::to_string(&events).unwrap()).unwrap();

        compare_transcripts(CompareArgs {
            go_events: go_path.clone(),
            rust_events: rust_path.clone(),
        })
        .expect("compare matching transcripts");

        let _ = fs::remove_file(go_path);
        let _ = fs::remove_file(rust_path);
    }

    #[test]
    fn real_gate_status_reports_missing_inputs() {
        let status = real_gate_status(&temp_path("missing-harness"), true, |_| None, false);

        assert!(!status.is_ready());
        assert!(!status.go_harness_exists);
        assert!(!status.go_tool_available);
        assert_eq!(status.missing_required_env, REAL_GATE_REQUIRED_ENV);
        assert_eq!(status.missing_transcript_env, REAL_GATE_TRANSCRIPT_ENV);
    }

    #[test]
    fn real_gate_status_accepts_required_inputs() {
        let harness_path = temp_dir("go-harness");
        fs::create_dir_all(&harness_path).unwrap();
        let env = |name: &str| match name {
            "OPENIM_API_ADDR" => Some("https://api.openim.test".to_string()),
            "OPENIM_WS_ADDR" => Some("wss://ws.openim.test".to_string()),
            "OPENIM_USER_ID" => Some("u1".to_string()),
            "OPENIM_TOKEN" => Some("token".to_string()),
            "OPENIM_GO_REPLAY_EVENTS" => Some("go-events.json".to_string()),
            "OPENIM_RUST_REPLAY_EVENTS" => Some("rust-events.json".to_string()),
            _ => None,
        };

        let status = real_gate_status(&harness_path, true, env, true);

        assert!(status.is_ready());
        assert!(status.missing_required_env.is_empty());
        assert!(status.missing_transcript_env.is_empty());
        let _ = fs::remove_dir_all(harness_path);
    }

    fn minimal_required_events(fixture: &openim_compat_tests::ContractFixture) -> Vec<ReplayEvent> {
        let mut events = Vec::new();
        for scenario in &fixture.event_scenarios {
            for method in &scenario.required_order {
                let (listener, method) = method
                    .strip_prefix("Base.")
                    .map(|method| ("Base", method))
                    .unwrap_or(("RecordedListener", method.as_str()));
                events.push(ReplayEvent {
                    scenario: scenario.name.clone(),
                    listener: listener.to_string(),
                    method: method.to_string(),
                    payload: serde_json::Value::Null,
                });
            }
        }
        events
    }

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "openim-replay-capture-{name}-{}.json",
            std::process::id()
        ))
    }

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "openim-replay-capture-{name}-{}",
            std::process::id()
        ))
    }
}
