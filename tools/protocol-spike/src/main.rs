use std::time::Duration;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use openim_protocol::{GetMaxSeqResp, WsReqIdentifier};
use openim_transport::{ClientConfig, OpenImWsClient};
use prost::Message;
use tokio::time::timeout;

#[derive(Debug, Parser)]
#[command(version, about = "OpenIM Rust phase-1 protocol POC")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Connect(CommonArgs),
    GetNewestSeq(GetNewestSeqArgs),
    ListenPush(ListenPushArgs),
}

#[derive(Debug, Args)]
struct CommonArgs {
    #[arg(long, env = "OPENIM_WS_ADDR")]
    ws_addr: String,
    #[arg(long, env = "OPENIM_USER_ID")]
    user_id: String,
    #[arg(long, env = "OPENIM_TOKEN")]
    token: String,
    #[arg(long, env = "OPENIM_PLATFORM_ID", default_value_t = 5)]
    platform_id: i32,
    #[arg(long, env = "OPENIM_OPERATION_ID")]
    operation_id: Option<String>,
    #[arg(long, default_value_t = false)]
    no_compression: bool,
}

#[derive(Debug, Args)]
struct GetNewestSeqArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(long, default_value_t = 0)]
    wait_push_seconds: u64,
}

#[derive(Debug, Args)]
struct ListenPushArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(long, default_value_t = 60)]
    timeout_seconds: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Connect(args) => {
            let client = OpenImWsClient::connect(config_from_args(args)).await?;
            println!(
                "connected user_id={} platform_id={} sdk_type={}",
                client.config().user_id,
                client.config().platform_id,
                client.config().sdk_type
            );
        }
        Command::GetNewestSeq(args) => run_get_newest_seq(args).await?,
        Command::ListenPush(args) => run_listen_push(args).await?,
    }

    Ok(())
}

async fn run_get_newest_seq(args: GetNewestSeqArgs) -> Result<()> {
    let mut client = OpenImWsClient::connect(config_from_args(args.common)).await?;
    let msg_incr = client.send_get_newest_seq().await?;
    println!("sent GetNewestSeq msg_incr={msg_incr}");

    loop {
        let resp = client.recv_envelope().await?;
        match resp.req_identifier {
            id if id == WsReqIdentifier::GetNewestSeq.as_i32() && resp.msg_incr == msg_incr => {
                if resp.err_code != 0 {
                    println!(
                        "GetNewestSeq failed err_code={} err_msg={}",
                        resp.err_code, resp.err_msg
                    );
                    break;
                }
                let decoded = GetMaxSeqResp::decode(resp.data.as_slice())?;
                println!(
                    "GetNewestSeq success max_seqs={:?} min_seqs={:?}",
                    decoded.max_seqs, decoded.min_seqs
                );
                break;
            }
            id if id == WsReqIdentifier::PushMsg.as_i32() => {
                println!("received push before response data_len={}", resp.data.len());
            }
            other => {
                println!(
                    "received unrelated envelope req_identifier={} msg_incr={} data_len={}",
                    other,
                    resp.msg_incr,
                    resp.data.len()
                );
            }
        }
    }

    if args.wait_push_seconds > 0 {
        wait_for_push(&mut client, Duration::from_secs(args.wait_push_seconds)).await?;
    }

    Ok(())
}

async fn run_listen_push(args: ListenPushArgs) -> Result<()> {
    let mut client = OpenImWsClient::connect(config_from_args(args.common)).await?;
    wait_for_push(&mut client, Duration::from_secs(args.timeout_seconds)).await
}

async fn wait_for_push(client: &mut OpenImWsClient, duration: Duration) -> Result<()> {
    match timeout(duration, async {
        loop {
            let resp = client.recv_envelope().await?;
            if resp.req_identifier == WsReqIdentifier::PushMsg.as_i32() {
                println!(
                    "received push operation_id={} data_len={}",
                    resp.operation_id,
                    resp.data.len()
                );
                return Ok::<_, anyhow::Error>(());
            }
            println!(
                "received envelope req_identifier={} msg_incr={} data_len={}",
                resp.req_identifier,
                resp.msg_incr,
                resp.data.len()
            );
        }
    })
    .await
    {
        Ok(result) => result,
        Err(_) => {
            println!("push wait timed out after {}s", duration.as_secs());
            Ok(())
        }
    }
}

fn config_from_args(args: CommonArgs) -> ClientConfig {
    let mut config = ClientConfig::new(args.ws_addr, args.user_id, args.token, args.platform_id);
    if let Some(operation_id) = args.operation_id {
        config.operation_id = operation_id;
    }
    config.compression = !args.no_compression;
    config
}
