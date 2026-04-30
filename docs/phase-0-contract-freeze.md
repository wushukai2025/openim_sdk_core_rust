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

Phase 0 已补可本地验证的契约冻结骨架：公开生命周期 API 种子清单、监听器方法清单、核心事件顺序、文件上传进度事件和 Go SDK 错误码映射已经进入 Golden fixture，并新增 Rust 兼容测试 crate 固定这些最小契约。本轮继续补上 Go SDK 源码级 surface 自动抽取，当前能从 open_im_sdk 导出函数和 callback_client.go listener interface 自动生成全量 public API/listener 清单并校验 fixture 种子没有脱离 Go 源码。当前仍不是完整契约冻结：所有回调顺序的真实 Go SDK 回放、真实服务端场景 Golden Event、Rust 同场景事件输出对比和绑定层回调语义仍需继续补齐。

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

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-contract-fixture">phase0_contract_baseline.json</code> 固定第一批 API、监听器、事件场景和错误码。
<!-- code-ref: phase0-contract-fixture -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/fixtures/phase0_contract_baseline.json#L1 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-validate-fixture">validate_fixture</code> 统一校验 fixture 版本、Go SDK 来源、API 名、listener 名、scenario 名和 error 名不为空且不重复。
<!-- code-ref: phase0-validate-fixture -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L73 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-extract-go-source-contract">extract_go_source_contract</code> 从 Go SDK 源码自动抽取 open_im_sdk 导出函数和 callback_client.go listener interface，作为后续全量契约冻结的源码基线。
<!-- code-ref: phase0-extract-go-source-contract -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L106 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-error-code-test">go_error_codes_match_rust_error_constants</code> 将 Go SDK 错误码 fixture 和 Rust ErrorCode 常量逐项比对。
<!-- code-ref: phase0-error-code-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L321 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-event-order-test">event_sequences_preserve_critical_order</code> 固定登录同步场景中连接成功必须早于同步开始。
<!-- code-ref: phase0-event-order-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L337 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-auto-extract-test">auto_extracts_go_public_api_and_listener_surface_when_source_exists</code> 在本机 Go SDK 源码存在时冻结当前 134 个 open_im_sdk 导出函数和 14 个 listener interface 数量。
<!-- code-ref: phase0-auto-extract-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L360 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase0-fixture-subset-test">fixture_seed_is_subset_of_auto_extracted_go_surface_when_source_exists</code> 校验 Golden fixture 里的种子 API 和 listener 仍然能在 Go SDK 源码 surface 中找到。
<!-- code-ref: phase0-fixture-subset-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L422 -->

## 当前覆盖范围

- 公开 API 自动抽取：当前本机 Go SDK 源码下 open_im_sdk 导出函数数量固定为 134，并验证核心 API 种子仍在源码 surface 中。
- 监听器自动抽取：当前本机 callback_client.go listener interface 数量固定为 14，并验证核心 listener 种子仍在源码 surface 中。
- Golden fixture 公开 API 种子：InitSDK、Login、Logout、SetConversationListener、SetAdvancedMsgListener、SendMessage、UploadFile。
- Golden fixture 监听器种子：Base、SendMsgCallBack、OnConnListener、OnConversationListener、OnAdvancedMsgListener、UploadFileCallback、UploadLogProgress。
- 核心事件场景：connection_status、login_sync_message、logout、message_arrival、file_upload_progress。
- 错误码：Go SDK sdkerrs 中的 common、user、message、conversation、group 基础错误码。

## 验证命令

```bash
cargo test -p openim-compat-tests
```

## Gate 状态

当前已完成：Phase 0 第一版契约 fixture、兼容测试 crate、核心 listener surface 覆盖检查、核心 public API surface 覆盖检查、Go SDK 源码 public API 自动抽取、Go SDK 源码 listener 自动抽取、fixture 种子对 Go 源码 surface 子集检查、核心 scenario 覆盖检查、错误码数值对齐检查、登录同步顺序最小检查。

当前未完成：全量 listener 事件顺序、真实 Go SDK 自动回放、真实服务端 Golden Event 采集、Rust 端同场景事件输出对比、绑定层回调命名和线程语义冻结。因此 Phase 0 现在是可运行骨架，不是完整契约冻结终态。
