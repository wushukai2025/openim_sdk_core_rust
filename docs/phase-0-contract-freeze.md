# Phase 0 Contract Freeze Report

更新时间：2026-04-30

<style>
code[data-code-ref],
a code[data-code-ref],
a:has(code[data-code-ref]) {
  text-decoration: none !important;
  text-decoration-line: none !important;
  border-bottom: none !important;
  box-shadow: none !important;
}
</style>

## 结论

Phase 0 已补可本地验证的契约冻结骨架：公开生命周期 API 种子清单、监听器方法清单、核心事件顺序、文件上传进度事件和 Go SDK 错误码映射已经进入 Golden fixture，并新增 Rust 兼容测试 crate 固定这些最小契约。本轮继续补上 Go SDK 源码级 surface 自动抽取、replay transcript 校验入口、绑定层回调命名/线程语义冻结、真实回放 transcript 采集工具和 Rust 本地 session lifecycle 采集入口。当前能从 open_im_sdk 导出函数和 callback_client.go listener interface 自动生成全量 public API/listener 清单，也能把真实回放器或 Rust 本地 session 输出归一成 transcript。当前仍不是完整契约冻结：Go SDK 真实场景回放 harness、真实服务端场景 Golden Event、Rust 真实服务端同场景事件输出采集和真实 transcript 对比执行仍需继续补齐。

## Go 契约来源

- 生命周期入口先以 Go SDK 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-go-init-login">InitSDK/Login/Logout</code> 为种子，固定初始化返回值、登录回调和登出回调边界。
<!-- code-ref: phase0-go-init-login -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core/open_im_sdk/init_login.go#L43 -->

- 监听器契约来自 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-go-callback-client">callback_client.go</code>，当前覆盖 Base、SendMsgCallBack、OnConnListener、OnConversationListener、OnAdvancedMsgListener、UploadFileCallback 和 UploadLogProgress。
<!-- code-ref: phase0-go-callback-client -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core/open_im_sdk_callback/callback_client.go#L17 -->

- 错误码契约来自 Go SDK 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-go-sdkerrs-code">sdkerrs code</code>，Rust 侧测试会校验这些数值没有漂移。
<!-- code-ref: phase0-go-sdkerrs-code -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core/pkg/sdkerrs/code.go#L18 -->

## Rust 落地点

- workspace 新增 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-workspace-compat-member">openim-compat-tests</code>，后续所有 Go/Rust 契约 fixture 和双栈对比都应优先放到这个 crate。
<!-- code-ref: phase0-workspace-compat-member -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/Cargo.toml#L4 -->

- workspace 新增 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-workspace-replay-capture-member">replay-capture</code>，用于捕获真实回放器 JSONL 输出、生成 transcript 并复用 Phase 0 校验入口。
<!-- code-ref: phase0-workspace-replay-capture-member -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/Cargo.toml#L19 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-contract-fixture">phase0_contract_baseline.json</code> 固定第一批 API、监听器、事件场景和错误码。
<!-- code-ref: phase0-contract-fixture -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/fixtures/phase0_contract_baseline.json#L1 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-replay-event">ReplayEvent</code> 是真实回放 transcript 的统一事件格式，包含 scenario、listener、method 和可选 payload。
<!-- code-ref: phase0-replay-event -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L55 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-validate-fixture">validate_fixture</code> 统一校验 fixture 版本、Go SDK 来源、API 名、listener 名、scenario 名和 error 名不为空且不重复。
<!-- code-ref: phase0-validate-fixture -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L100 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-extract-go-source-contract">extract_go_source_contract</code> 从 Go SDK 源码自动抽取 open_im_sdk 导出函数和 callback_client.go listener interface，作为后续全量契约冻结的源码基线。
<!-- code-ref: phase0-extract-go-source-contract -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L133 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-validate-replay-transcript">validate_replay_transcript</code> 校验真实回放 transcript 是否覆盖 fixture 中所有 required scenario 和 required event 顺序。
<!-- code-ref: phase0-validate-replay-transcript -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L242 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-compare-replay-scenario">compare_replay_scenario</code> 对比 Go 与 Rust 同一 scenario 的事件序列，作为后续双栈回放 Gate 的本地断言入口。
<!-- code-ref: phase0-compare-replay-scenario -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L299 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-binding-callback-contracts">binding_callback_contracts</code> 根据 listener surface 生成 native C ABI 和 wasm 回调名，并冻结 native 串行 SDK 回调队列与 wasm host event loop 线程语义。
<!-- code-ref: phase0-binding-callback-contracts -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L342 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-validate-binding-callbacks">validate_binding_callback_contracts</code> 校验 native 回调名唯一、openim_ 前缀、wasm lowerCamelCase 命名和线程策略不漂移。
<!-- code-ref: phase0-validate-binding-callbacks -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L373 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-error-code-test">go_error_codes_match_rust_error_constants</code> 将 Go SDK 错误码 fixture 和 Rust ErrorCode 常量逐项比对。
<!-- code-ref: phase0-error-code-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L567 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-event-order-test">event_sequences_preserve_critical_order</code> 固定登录同步场景中连接成功必须早于同步开始。
<!-- code-ref: phase0-event-order-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L583 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-real-go-replay-test">real_go_sdk_replay_transcript_matches_phase0_contract</code> 是真实 Go SDK 回放 transcript 的 ignored Gate，需要 OPENIM_GO_REPLAY_EVENTS 指向真实采集文件后执行。
<!-- code-ref: phase0-real-go-replay-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L643 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-real-compare-replay-test">real_go_and_rust_replay_transcripts_match_phase0_sequences</code> 是 Go/Rust 双栈真实回放对比 ignored Gate，需要同时提供 Go 与 Rust transcript。
<!-- code-ref: phase0-real-compare-replay-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L671 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-binding-seed-test">binding_callback_contract_freezes_seed_names_and_threads</code> 固定核心 seed listener 的 native/wasm 回调名和线程策略。
<!-- code-ref: phase0-binding-seed-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L709 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-replay-capture-command">capture_command</code> 运行外部真实回放器命令，捕获 stdout JSONL 并输出标准 transcript。
<!-- code-ref: phase0-replay-capture-command -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/tools/replay-capture/src/main.rs#L128 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-replay-capture-jsonl">capture_jsonl</code> 支持从文件或 stdin 读取 JSONL 事件并生成 transcript，方便 Go/Rust 回放器用管道接入。
<!-- code-ref: phase0-replay-capture-jsonl -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/tools/replay-capture/src/main.rs#L151 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-replay-capture-rust-session">capture_rust_session</code> 输出 Rust 本地 session lifecycle transcript，先覆盖 init、login、task start/stop、logout 和 uninit 事件采集。
<!-- code-ref: phase0-replay-capture-rust-session -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/tools/replay-capture/src/main.rs#L159 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-replay-capture-validate">validate_transcript</code> 复用 Phase 0 fixture 校验 transcript，作为采集后立即验收的 CLI 入口。
<!-- code-ref: phase0-replay-capture-validate -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/tools/replay-capture/src/main.rs#L165 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-replay-capture-compare">compare_transcripts</code> 读取 Go/Rust transcript，先按 Phase 0 fixture 校验两边完整性，再逐 required scenario 对比事件序列。
<!-- code-ref: phase0-replay-capture-compare -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/tools/replay-capture/src/main.rs#L95 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-replay-rust-session-events">capture_rust_session_events</code> 通过 OpenImSession 监听器捕获 Rust SessionEvent 并转换成统一 ReplayEvent。
<!-- code-ref: phase0-replay-rust-session-events -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/tools/replay-capture/src/main.rs#L234 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-auto-extract-test">auto_extracts_go_public_api_and_listener_surface_when_source_exists</code> 在本机 Go SDK 源码存在时冻结当前 134 个 open_im_sdk 导出函数和 14 个 listener interface 数量。
<!-- code-ref: phase0-auto-extract-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L763 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-fixture-subset-test">fixture_seed_is_subset_of_auto_extracted_go_surface_when_source_exists</code> 校验 Golden fixture 里的种子 API 和 listener 仍然能在 Go SDK 源码 surface 中找到。
<!-- code-ref: phase0-fixture-subset-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L825 -->

## 当前覆盖范围

- 公开 API 自动抽取：当前本机 Go SDK 源码下 open_im_sdk 导出函数数量固定为 134，并验证核心 API 种子仍在源码 surface 中。
- 监听器自动抽取：当前本机 callback_client.go listener interface 数量固定为 14，并验证核心 listener 种子仍在源码 surface 中。
- Golden fixture 公开 API 种子：InitSDK、Login、Logout、SetConversationListener、SetAdvancedMsgListener、SendMessage、UploadFile。
- Golden fixture 监听器种子：Base、SendMsgCallBack、OnConnListener、OnConversationListener、OnAdvancedMsgListener、UploadFileCallback、UploadLogProgress。
- 核心事件场景：connection_status、login_sync_message、logout、message_arrival、file_upload_progress。
- 错误码：Go SDK sdkerrs 中的 common、user、message、conversation、group 基础错误码。
- replay transcript 格式：JSON 数组，每条事件包含 scenario、listener、method 和可选 payload；ignored Gate 通过 OPENIM_GO_REPLAY_EVENTS 和 OPENIM_RUST_REPLAY_EVENTS 读取真实采集文件。
- 绑定回调命名：native C ABI 使用 openim_{listener_snake}_{method_snake}，wasm 使用 method lowerCamelCase；native 回调线程语义为 sdk_serialized_callback_queue，wasm 为 host_event_loop。
- replay 采集工具：支持捕获外部命令 stdout JSONL、读取 JSONL 文件或 stdin、输出 Rust 本地 session lifecycle transcript，并用 Phase 0 fixture 校验和对比真实回放 transcript。

## 验证命令

```bash
cargo test -p openim-compat-tests
cargo test -p replay-capture
cargo run -p replay-capture -- capture-rust-session --output rust-local-session.json
cargo run -p replay-capture -- capture-command --output go-events.json -- /path/to/go-phase0-replay --scenario phase0
cargo run -p replay-capture -- validate --events go-events.json
cargo run -p replay-capture -- compare --go-events go-events.json --rust-events rust-events.json
```

真实回放 transcript Gate 入口如下，当前本机未提供真实 Go/Rust transcript，因此该项没有执行：

```bash
OPENIM_GO_REPLAY_EVENTS=go-events.json cargo test -p openim-compat-tests -- --ignored real_go_sdk_replay_transcript_matches_phase0_contract
OPENIM_RUST_REPLAY_EVENTS=rust-events.json cargo test -p openim-compat-tests -- --ignored real_rust_replay_transcript_matches_phase0_contract
OPENIM_GO_REPLAY_EVENTS=go-events.json OPENIM_RUST_REPLAY_EVENTS=rust-events.json cargo test -p openim-compat-tests -- --ignored real_go_and_rust_replay_transcripts_match_phase0_sequences
```

## Gate 状态

当前已完成：Phase 0 第一版契约 fixture、兼容测试 crate、replay-capture 工具、核心 listener surface 覆盖检查、核心 public API surface 覆盖检查、Go SDK 源码 public API 自动抽取、Go SDK 源码 listener 自动抽取、fixture 种子对 Go 源码 surface 子集检查、replay transcript 顺序校验入口、Go/Rust replay transcript 序列对比入口、真实 transcript ignored Gate、真实回放 JSONL 采集与归一化入口、Rust 本地 session lifecycle transcript 采集入口、真实 transcript CLI 对比入口、绑定层回调命名冻结、绑定层回调线程语义冻结、核心 scenario 覆盖检查、错误码数值对齐检查、登录同步顺序最小检查。

当前未完成：Go SDK 真实场景回放 harness、真实服务端 Golden Event 采集、Rust 真实服务端同场景事件输出采集、真实 transcript 对比执行。因此 Phase 0 现在是可运行骨架，不是完整契约冻结终态。
