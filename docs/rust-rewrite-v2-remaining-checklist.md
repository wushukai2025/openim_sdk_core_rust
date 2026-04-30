# Rust Rewrite V2 Remaining Checklist

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

V2 从“Phase 7 离线核心边界已具备”继续推进到“可替换跨平台 SDK”。当前优先级不是继续扩写离线领域模型，而是补齐契约冻结、真实服务端联调、真实资源装配、绑定层产物和双栈验证。所有真实服务端相关 Gate 都必须使用有效 OpenIM 服务端地址、账号、token、上传端点和可触发推送的测试场景验证，不能由本地 fixture 冒充完成。

当前仓库的 Rust workspace 已落地核心 crate 列表；兼容测试 crate 已在 R2-00 继续补齐，并已具备 Go SDK 源码级 public API/listener surface 自动抽取、replay transcript 校验入口、绑定回调命名/线程语义冻结、replay-capture transcript 采集工具、Rust 本地 session lifecycle 采集入口、Rust 真实 transport probe 采集入口、Go SDK 真实场景回放 harness 源码入口、Go harness 本地编译检查和真实 Gate 就绪检查入口。由于暂时没有真实环境，Phase 0 本地骨架先告一段落，真实回放转为外部 Gate 暂挂；R2-04 已继续推进 native Session 资源句柄闭环；R2-03 已补齐本地可测的对象上传 API、签名 PUT 请求和 mock 上传边界；R2-06 已补 Session 内本地 fake transport 发送、拉取、推送和会话更新闭环；R2-08 已补原生 C ABI 和 wasm 导出 crate 骨架。真实上传端点、真实消息端到端执行和平台示例仍是外部 Gate。
<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-workspace-members">workspace members</code>
<!-- code-ref: v2-workspace-members -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/Cargo.toml#L2 -->

仓库规则要求 Phase 报告必须保留真实剩余 Gate，不能把离线验证说成端到端完成。
<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-agents-gate-rule">AGENTS Gate rule</code>
<!-- code-ref: v2-agents-gate-rule -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/AGENTS.md#L58 -->

## 外部 Go 仓库依赖处理原则

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-protocol-go-module">protocol</code> 是协议契约来源，不是 Rust SDK 运行时依赖。Rust 侧继续由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-openim-protocol-dir">OPENIM_PROTOCOL_DIR</code> 或本地默认 protocol 目录读取 schema，并通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-prost-build">prost-build</code> 与 vendored protoc 生成 Rust protobuf 类型；不要手改 Go 生成文件，也不要把 Go runtime 作为实现路径。
  <!-- code-ref: v2-protocol-go-module -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/protocol/go.mod#L1 -->
  <!-- code-ref: v2-openim-protocol-dir -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-protocol/build.rs#L6 -->
  <!-- code-ref: v2-prost-build -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-protocol/Cargo.toml#L17 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-tools-go-module">tools</code> 是 Go 工程工具箱，不是整库重写目标。SDK 所需行为只按实际使用面迁移：错误码和统一错误已经落到 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-error-code">ErrorCode</code> 与 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-openim-error">OpenImError</code>，JSON、base64、gzip 等通用能力优先使用 Rust 生态和 workspace 依赖；MQ、Redis、Mongo、Gin/gRPC 中间件、服务发现等服务端工具不进入 SDK 迁移范围，除非后续有明确 SDK 行为依赖。
  <!-- code-ref: v2-tools-go-module -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/tools/go.mod#L1 -->
  <!-- code-ref: v2-error-code -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-errors/src/lib.rs#L5 -->
  <!-- code-ref: v2-openim-error -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-errors/src/lib.rs#L77 -->

- 该原则不新增独立 Gate；若后续要求 Rust 仓库脱离旁边 Go checkout 独立构建，应新增的是必要 protocol schema 的 snapshot/vendor 同步与再生成验证任务，而不是扩大成 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-protocol-build-input">openim-protocol</code> 与 Go 工具库的整库重写。
  <!-- code-ref: v2-protocol-build-input -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-protocol/build.rs#L16 -->

## V2 剩余任务总表

| 编号 | 阶段 | 状态 | 剩余任务 | 完成标准 |
| --- | --- | --- | --- | --- |
| R2-00 | Phase 0 | 外部 Gate 暂挂 | 已补契约冻结报告、Golden Fixture、兼容测试骨架、Go SDK 源码级 public API/listener surface 自动抽取、replay transcript 校验入口、绑定回调命名/线程语义冻结、replay-capture 工具、Rust 本地 session lifecycle 采集入口、Rust 真实 transport probe 采集入口、Go SDK 真实场景回放 harness 源码入口、Go harness 本地编译检查和真实 Gate 就绪检查入口；真实 Go SDK harness 执行、真实服务端 Golden Event 和 Rust 真实服务端完整同场景采集等待真实环境 | 本地源码抽取、fixture 校验、transcript validator、绑定回调契约、采集工具、Go harness 本地编译和 real gate readiness 检查通过；后续有真实环境后扩成真实运行回放契约冻结 |
| R2-01 | Phase 1 | 外部 Gate | 用真实 OpenIM 服务端完成登录、请求响应和推送收包 | 真实账号下协议 POC 命令通过并更新报告 |
| R2-02 | Phase 4 | 外部 Gate | 真实服务端 native 和 wasm 兼容收发、前后台切换验证 | ignored 真实服务端测试被实际执行并留存结果 |
| R2-03 | Phase 5 | 本地已推进，外部 Gate 暂挂 | 已补对象上传 API 边界、上传凭据流程、签名分片 PUT 请求和 mock 上传验证；真实 HTTP 上传端点端到端执行等待真实环境 | 本地 mock 覆盖端点语义并通过 openim-domain 测试，真实端点再做端到端上传 |
| R2-04 | Phase 6 | 本地已推进 | 已接入 SessionRuntimeResources、ResourceOpened/ResourceClosed 事件和 native Session resource adapter；native 登录会打开 SQLite storage、执行迁移，并挂接 transport/sync 任务句柄；wasm IndexedDB 运行时装配和真实 WebSocket 长连接任务仍待后续 Gate | Session 登录会创建资源句柄，并能在 Logout 和 UnInit 清理；native SQLite 打开和迁移已有单元测试覆盖 |
| R2-05 | Phase 6 | 外部 Gate | 登录接口 HTTP 校验、平台线程切换、前后台生命周期回归 | iOS、Android、Web 或桌面最小场景可复现 |
| R2-06 | Phase 7 | 本地已推进，外部 Gate 暂挂 | 已串联上传结果、SendMsg、PullMsg、推送消息和本地会话更新；真实账号端到端联调等待真实环境 | 本地 fake transport 闭环已通过 openim-session 测试，真实账号再端到端联调 |
| R2-07 | Phase 7 | 外部 Gate | 撤回和已读回执 HTTP API 服务端校验 | 服务端状态、对端回调和本地状态三者一致 |
| R2-08 | Phase 8 | 本地已推进 | 已创建原生 C ABI 和 wasm 导出 crate 骨架，覆盖句柄模型和基础生命周期 API | openim-ffi 和 openim-wasm 可编译，基础生命周期测试和 wasm32 check 通过 |
| R2-09 | Phase 8 | 平台 Gate | 产出 iOS、Android、桌面和 Web 示例工程 | 示例工程能 Init、Login、Logout、UnInit |
| R2-10 | Phase 9 | 外部 Gate | 建立 Go 与 Rust 双栈对比报告和差异清单 | 核心场景有对比结果、已知差异、灰度和回滚方案 |

## 现有未完成 Gate 引用

- Phase 0 已新增契约冻结骨架，并已补 Go SDK 源码级 public API/listener surface 自动抽取、replay transcript 校验入口、绑定回调命名/线程语义冻结、replay-capture 工具、Rust 本地 session lifecycle 采集入口和 Rust 真实 transport probe 采集入口；Go SDK 真实场景回放 harness、真实服务端 Golden Event 和 Rust 真实服务端完整同场景采集仍未完成。
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase0-report">Phase 0 contract freeze</code>
  <!-- code-ref: v2-phase0-report -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/docs/phase-0-contract-freeze.md#L1 -->

- Phase 0 的 replay-capture 工具已加入 workspace，后续真实 Go/Rust 回放器可以通过 stdout JSONL 或 JSONL 文件生成标准 transcript，并用 compare 子命令执行双栈 transcript 对比。
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase0-replay-capture-tool">Replay capture tool</code>
  <!-- code-ref: v2-phase0-replay-capture-tool -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/tools/replay-capture/src/main.rs#L264 -->

- Phase 0 已新增真实 Gate 就绪检查入口，当前本机检查结果为 go_tool=ok，但真实 OpenIM 地址、账号、token 环境变量缺失，因此不能执行真实回放 Gate。
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase0-real-gate-check">Real gate readiness check</code>
  <!-- code-ref: v2-phase0-real-gate-check -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/tools/replay-capture/src/main.rs#L167 -->

- Phase 0 已新增 Go SDK 真实场景回放 harness 源码入口和依赖校验文件，当前本机 go test ./... 已通过；后续提供真实服务端地址、账号、token 后可输出 JSONL transcript。
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase0-go-replay-harness">Go replay harness</code>
  <!-- code-ref: v2-phase0-go-replay-harness -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/tools/go-phase0-replay/main.go#L402 -->

- Phase 0 的 Rust 本地 session lifecycle 采集入口已落在 replay-capture，当前覆盖 init、login、task start/stop、logout 和 uninit。
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase0-rust-session-capture">Rust session capture</code>
  <!-- code-ref: v2-phase0-rust-session-capture -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/tools/replay-capture/src/main.rs#L328 -->

- Phase 0 的 Rust 真实 transport probe 采集入口已落在 replay-capture，当前覆盖真实 WebSocket 连接、GetNewestSeq 请求响应和可选 PushMsg 归一化；完整 required transcript 仍等待 session 同步、文件上传和真实消息触发闭环。
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase0-rust-transport-capture">Rust transport capture</code>
  <!-- code-ref: v2-phase0-rust-transport-capture -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/tools/replay-capture/src/main.rs#L334 -->

- Phase 0 的源码级 surface 自动抽取落在兼容测试 crate，当前本机 Go SDK 源码存在时会冻结 134 个 open_im_sdk 导出函数和 14 个 listener interface。
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase0-source-extractor">Go source contract extractor</code>
  <!-- code-ref: v2-phase0-source-extractor -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L133 -->

- Phase 0 的 replay transcript 校验入口已落在兼容测试 crate，真实 Go/Rust 回放采集文件后续需要喂给 ignored Gate 执行。
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase0-replay-validator">Replay transcript validator</code>
  <!-- code-ref: v2-phase0-replay-validator -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L242 -->

- Phase 0 的绑定回调命名和线程语义已落在兼容测试 crate，Phase 8 创建 C ABI/wasm 导出层时应复用该契约。
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase0-binding-callbacks">Binding callback contract</code>
  <!-- code-ref: v2-phase0-binding-callbacks -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L342 -->

- Phase 1 真实协议联调仍未完成，需要真实服务端、有效用户和可触发推送。
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase1-open-gate">Phase 1 open Gate</code>
  <!-- code-ref: v2-phase1-open-gate -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/docs/phase-1-protocol-diff.md#L80 -->

- Phase 4 传输层本地 native 和 wasm 已验证，但真实 OpenIM server 兼容收发和前后台行为还没验证。
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase4-open-gate">Phase 4 open Gate</code>
  <!-- code-ref: v2-phase4-open-gate -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/docs/phase-4-transport-layer.md#L133 -->

- Phase 5 领域层已补对象上传 API 和签名上传边界，当前由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-object-storage-api">ObjectStorageApi</code> 对接服务端 object API，由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-signed-multipart-upload-client">SignedMultipartUploadClient</code> 对接签名 PUT 上传；真实 HTTP 上传端点仍需后续环境验证。
  <!-- code-ref: v2-object-storage-api -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L259 -->
  <!-- code-ref: v2-signed-multipart-upload-client -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L376 -->

- Phase 6 已从资源装配骨架推进到 native 本地资源句柄闭环；真实 native WebSocket 长连接任务、wasm IndexedDB 运行时装配、同步任务和登录 HTTP 校验仍未接入。
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase6-open-gate">Phase 6 open Gate</code>
  <!-- code-ref: v2-phase6-open-gate -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/docs/phase-6-session-runtime.md#L135 -->

- Phase 7 已具备离线消息和会话集成边界，并已补 Session 内本地 fake transport 闭环；当前由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-session-message-transport">SessionMessageTransport</code> 覆盖发送、拉取和推送，由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-session-message-flow-test">session_message_transport_sends_pulls_pushes_and_updates_conversations</code> 固定上传结果到发送、PullMsg、Push 和会话投影的本地闭环；真实 SendMsg、PullMsg、推送回调、上传到发送、撤回和已读回执仍未端到端完成。
  <!-- code-ref: v2-session-message-transport -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L436 -->
  <!-- code-ref: v2-session-message-flow-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L1050 -->
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase7-open-gate">Phase 7 open Gate</code>
  <!-- code-ref: v2-phase7-open-gate -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/docs/phase-7-message-conversation.md#L178 -->

- Phase 8 绑定层骨架已落地，当前 workspace 已加入 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-openim-ffi-member">openim-ffi</code> 和 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-openim-wasm-member">openim-wasm</code>；C ABI 侧由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-openim-ffi-handle">OpenImFfiSession</code> 固定 opaque handle，wasm 侧由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-openim-wasm-session">OpenImWasmSession</code> 暴露基础生命周期。平台示例和完整 listener 回调派发仍未完成。
  <!-- code-ref: v2-openim-ffi-member -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/Cargo.toml#L6 -->
  <!-- code-ref: v2-openim-wasm-member -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/Cargo.toml#L19 -->
  <!-- code-ref: v2-openim-ffi-handle -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L18 -->
  <!-- code-ref: v2-openim-wasm-session -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-wasm/src/lib.rs#L8 -->
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase8-report">Phase 8 bindings report</code>
  <!-- code-ref: v2-phase8-report -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/docs/phase-8-bindings.md#L86 -->

## 本地优先执行顺序

1. Phase 0 真实回放契约暂挂到外部 Gate；后续有真实环境后再执行 Go harness 真实运行、真实服务端 Golden Event 和 Rust 完整同场景采集。
2. Session native 资源适配器已落地，现有资源句柄边界通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-session-runtime-resources">SessionRuntimeResources</code> 持有并清理。
   <!-- code-ref: v2-session-runtime-resources -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L216 -->
3. 文件 HTTP 上传客户端边界已推进到可 mock 验证，当前领域层已有 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-file-upload-boundary">FileUploadClient</code> 和签名 PUT 请求边界；真实 endpoint 后续跟随外部 Gate 验证。
   <!-- code-ref: v2-file-upload-boundary -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L276 -->
4. 本地 fake transport 的消息发送、拉取、推送闭环已落地，Phase 7 已从单服务边界推进到 Session 内链路闭环。
5. 绑定层骨架已落地，下一步进入平台示例和双栈验证。平台示例必须后续单独验证，不能用绑定 crate 单元测试替代。

## 外部环境需求

以下项没有外部环境不能标记完成：

- 真实 OpenIM WebSocket 地址、HTTP API 地址、userID、token 和平台 ID。
- 可触发服务端推送的测试账号或自动化触发脚本。
- 图片和文件上传所需的真实凭据、签名或上传 endpoint。
- iOS、Android、Web、桌面示例工程构建环境。
- Go SDK 与 Rust SDK 同场景运行的双栈测试环境。

## V2 本轮验收口径

本轮可以先完成以下本地项：

- 新增本清单文档并通过 Markdown code-ref 校验。
- 新增 Phase 0 契约冻结文档、兼容测试骨架、Go SDK 源码级 public API/listener surface 自动抽取、replay transcript 校验入口、绑定回调契约、replay-capture 工具、Rust 本地 session lifecycle 采集入口、Rust 真实 transport probe 采集入口、Go SDK 真实场景回放 harness 源码入口、Go harness 本地编译检查和真实 Gate 就绪检查入口。
- 新增 Phase 5 对象上传 API、签名 PUT 请求和 mock 上传验证。
- 新增 Phase 7 Session 内本地 fake transport 消息发送、拉取、推送和会话更新闭环。
- 新增 Phase 8 原生 C ABI 和 wasm 导出 crate 骨架、句柄模型和基础生命周期 API。
- 保持 Rust workspace 检查通过。

本轮不能宣称完成以下项：

- 真实服务端协议 Gate。
- 真实上传端到端 Gate。
- 平台示例交付 Gate。
- Go 与 Rust 双栈替换 Gate。
