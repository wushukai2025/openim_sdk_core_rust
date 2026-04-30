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
    load_phase0_contract_fixture, load_replay_events, validate_replay_transcript, ReplayEvent,
};

#[derive(Debug, Parser)]
#[command(version, about = "OpenIM Phase 0 replay transcript capture helper")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    CaptureCommand(CaptureCommandArgs),
    CaptureJsonl(CaptureJsonlArgs),
    Validate(ValidateArgs),
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
struct ValidateArgs {
    #[arg(long, env = "OPENIM_REPLAY_EVENTS")]
    events: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::CaptureCommand(args) => capture_command(args),
        Command::CaptureJsonl(args) => capture_jsonl(args),
        Command::Validate(args) => validate_transcript(args),
    }
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
}
