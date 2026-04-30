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

V2 从“Phase 7 离线核心边界已具备”继续推进到“可替换跨平台 SDK”。当前优先级不是继续扩写离线领域模型，而是先收敛到 macOS 桌面平台跑通：优先完成 macOS 下的 native 资源装配、C ABI 产物、desktop C 示例构建运行、真实服务端 Init/Login/Logout/UnInit 生命周期和基础消息链路验证。iOS、Android、Web、wasm 平台工程、包装产物发布和双栈替换先暂停。所有真实服务端相关 Gate 都必须使用有效 OpenIM 服务端地址、账号、token、上传端点和可触发推送的测试场景验证，不能由本地 fixture 冒充完成。

当前仓库的 Rust workspace 已落地核心 crate 列表；兼容测试 crate 已在 R2-00 继续补齐，并已具备 Go SDK 源码级 public API/listener surface 自动抽取、replay transcript 校验入口、绑定回调命名/线程语义冻结、replay-capture transcript 采集工具、Rust 本地 session lifecycle 采集入口、Rust 真实 transport probe 采集入口、Go SDK 真实场景回放 harness 源码入口、Go harness 本地编译检查和真实 Gate 就绪检查入口。由于暂时没有真实环境，Phase 0 本地骨架先告一段落，真实回放转为外部 Gate 暂挂；R2-04 已继续推进 native Session 资源句柄闭环；R2-03 已补齐本地可测的对象上传 API、签名 PUT 请求和 mock 上传边界；R2-06 已补 Session 内本地 fake transport 发送、拉取、推送和会话更新闭环；R2-08 已补原生 C ABI 和 wasm 导出 crate 骨架；R2-09 已补 desktop C、iOS Swift、Android Kotlin/JNI、Web TypeScript 生命周期示例源码、通用 session event listener 桥接、Android listener 示例桥接、已实现 Session event 到 Go 细分 listener 的映射种子和示例 API 漂移检查，并已让 native C ABI 创建路径接入 `NativeSessionResourceAdapter`、新增 `openim_session_create_with_data_dir` 和 `OPENIM_DATA_DIR` 入口，且在 Darwin 本机实际用 `clang` 链接 `openim-ffi` staticlib 运行带 storage/transport/sync 资源事件的 desktop C 生命周期 smoke。下一轮仍优先推进 macOS 桌面平台真实服务端验证；其他平台、真实上传端点、Go SDK 细分 listener 全量映射和双栈替换暂不推进。
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
| R2-01 | Phase 1 | macOS 优先 Gate | 先在 macOS 真实 OpenIM 服务端环境完成登录、请求响应和推送收包；其他平台暂不推进 | macOS 真实账号下协议 POC 命令通过并更新报告 |
| R2-02 | Phase 4 | macOS 优先 Gate | 先验证 macOS native 真实服务端兼容收发；wasm 和移动端前后台切换暂停 | macOS native ignored 真实服务端测试被实际执行并留存结果 |
| R2-03 | Phase 5 | 本地已推进，外部 Gate 暂挂 | 已补对象上传 API 边界、上传凭据流程、签名分片 PUT 请求和 mock 上传验证；真实 HTTP 上传端点端到端执行等待真实环境 | 本地 mock 覆盖端点语义并通过 openim-domain 测试，真实端点再做端到端上传 |
| R2-04 | Phase 6 | 本地已推进 | 已接入 SessionRuntimeResources、ResourceOpened/ResourceClosed 事件和 native Session resource adapter；native 登录会打开 SQLite storage、执行迁移，并挂接 transport/sync 任务句柄；wasm IndexedDB 运行时装配和真实 WebSocket 长连接任务仍待后续 Gate | Session 登录会创建资源句柄，并能在 Logout 和 UnInit 清理；native SQLite 打开和迁移已有单元测试覆盖 |
| R2-05 | Phase 6 | macOS 优先 Gate | 先完成 macOS desktop 登录接口 HTTP 校验、回调线程策略和生命周期回归；iOS、Android、Web 暂停 | macOS desktop 最小场景可复现并留存命令、环境变量和结果 |
| R2-06 | Phase 7 | 本地已推进，外部 Gate 暂挂 | 已串联上传结果、SendMsg、PullMsg、推送消息和本地会话更新；真实账号端到端联调等待真实环境 | 本地 fake transport 闭环已通过 openim-session 测试，真实账号再端到端联调 |
| R2-07 | Phase 7 | 外部 Gate | 撤回和已读回执 HTTP API 服务端校验 | 服务端状态、对端回调和本地状态三者一致 |
| R2-08 | Phase 8 | 本地已推进 | 已创建原生 C ABI 和 wasm 导出 crate 骨架，覆盖句柄模型和基础生命周期 API | openim-ffi 和 openim-wasm 可编译，基础生命周期测试和 wasm32 check 通过 |
| R2-09 | Phase 8 | 本地已推进，外部 Gate 暂挂 | 已补 desktop C、iOS Swift、Android Kotlin/JNI、Web TypeScript 生命周期示例源码、通用 session event listener 注册/注销、Android listener 示例桥接和已实现 Session event 到 Go 细分 listener 的映射种子；native C ABI 创建路径现已接入 `NativeSessionResourceAdapter`，desktop C 示例已固定 `OPENIM_PLATFORM_MACOS`，支持 `OPENIM_API_ADDR`、`OPENIM_WS_ADDR`、`OPENIM_USER_ID`、`OPENIM_TOKEN` 和可选 `OPENIM_DATA_DIR` 覆盖默认值，并已在 Darwin 本机实际链接本地 `openim-ffi` staticlib 跑通 listener 注册、Init、Login、Logout、UnInit、listener 注销以及 storage/transport/sync 资源事件 smoke；真实 OpenIM 服务端执行等待环境 | macOS desktop C 示例可在本地构建运行，并在真实环境下复用同一可执行文件跑通 listener 注册、Init、Login、Logout、UnInit、listener 注销与 native 资源装配；其他平台不纳入本轮验收 |
| R2-10 | Phase 9 | 暂停 | Go 与 Rust 双栈对比报告和灰度替换方案等待 macOS 单平台跑通后再恢复 | macOS 单平台真实 Gate 通过后重新拆分双栈对比范围 |

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
  <!-- code-ref: v2-phase0-source-extractor -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L142 -->

- Phase 0 的 replay transcript 校验入口已落在兼容测试 crate，真实 Go/Rust 回放采集文件后续需要喂给 ignored Gate 执行。
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase0-replay-validator">Replay transcript validator</code>
  <!-- code-ref: v2-phase0-replay-validator -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L251 -->

- Phase 0 的绑定回调命名和线程语义已落在兼容测试 crate，Phase 8 创建 C ABI/wasm 导出层时应复用该契约。
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase0-binding-callbacks">Binding callback contract</code>
  <!-- code-ref: v2-phase0-binding-callbacks -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L351 -->

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

- Phase 8 绑定层骨架已落地，当前 workspace 已加入 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-openim-ffi-member">openim-ffi</code> 和 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-openim-wasm-member">openim-wasm</code>；C ABI 侧由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-openim-ffi-handle">OpenImFfiSession</code> 固定 opaque handle，wasm 侧由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-openim-wasm-session">OpenImWasmSession</code> 暴露基础生命周期。R2-09 已补 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase8-ffi-header">openim_ffi.h</code>、<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase8-desktop-example">openim_desktop_lifecycle.c</code>、<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase8-ios-example">OpenIMLifecycleExample.swift</code>、<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase8-android-kotlin-example">OpenIMLifecycleExample.kt</code>、<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase8-android-jni-example">openim_jni_lifecycle.cc</code> 和 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase8-web-example">openim_lifecycle.ts</code> 生命周期示例源码；native C ABI 创建路径已通过 `openim_session_create_with_data_dir` 接入 `NativeSessionResourceAdapter`；通用 session event listener 由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase8-ffi-register-listener">openim_session_register_listener</code> 与 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase8-wasm-add-listener">addListener</code> 暴露；已实现 Session event 到 Go 细分 listener 的映射种子由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase8-session-listener-mapping">session_event_listener_mappings</code> 固定；示例 API 漂移由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase8-native-example-test">native_header_and_examples_cover_lifecycle_exports</code> 与 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase8-web-example-test">web_example_uses_wasm_lifecycle_exports</code> 固定。真实平台工程构建、平台包装产物和 Go SDK 细分 listener 全量映射仍未完成。
  <!-- code-ref: v2-openim-ffi-member -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/Cargo.toml#L6 -->
  <!-- code-ref: v2-openim-wasm-member -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/Cargo.toml#L19 -->
  <!-- code-ref: v2-openim-ffi-handle -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L26 -->
  <!-- code-ref: v2-openim-wasm-session -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-wasm/src/lib.rs#L16 -->
  <!-- code-ref: v2-phase8-ffi-header -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/include/openim_ffi.h#L1 -->
  <!-- code-ref: v2-phase8-desktop-example -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/examples/desktop-c/openim_desktop_lifecycle.c#L1 -->
  <!-- code-ref: v2-phase8-ios-example -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/examples/ios-swift/OpenIMLifecycleExample.swift#L18 -->
  <!-- code-ref: v2-phase8-android-kotlin-example -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/examples/android-kotlin/OpenIMLifecycleExample.kt#L3 -->
  <!-- code-ref: v2-phase8-android-jni-example -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/examples/android-kotlin/openim_jni_lifecycle.cc#L1 -->
  <!-- code-ref: v2-phase8-web-example -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/examples/web/openim_lifecycle.ts#L1 -->
  <!-- code-ref: v2-phase8-ffi-register-listener -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L148 -->
  <!-- code-ref: v2-phase8-wasm-add-listener -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-wasm/src/lib.rs#L106 -->
  <!-- code-ref: v2-phase8-session-listener-mapping -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L435 -->
  <!-- code-ref: v2-phase8-native-example-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L530 -->
  <!-- code-ref: v2-phase8-web-example-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-wasm/src/lib.rs#L203 -->
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase8-report">Phase 8 bindings report</code>
  <!-- code-ref: v2-phase8-report -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/docs/phase-8-bindings.md#L1 -->

## 本地优先执行顺序

1. macOS desktop C 示例的本地 staticlib 构建、编译和离线生命周期 smoke 已在 Darwin 本机跑通；当前示例已能经由 `openim_session_create_with_data_dir` 和 `OPENIM_DATA_DIR` 打开 native storage/transport/sync 资源。下一步直接复用同一示例和 `OPENIM_*` 环境变量接入真实 OpenIM 环境，执行 listener 注册、Init、Login、Logout、UnInit 和 listener 注销。
2. 在 macOS native 上补真实服务端协议与 transport 验证：登录、GetNewestSeq、至少一种推送或可复现的收包场景；wasm、iOS、Android 暂停。
3. Phase 8 报告和本清单已补本地命令、环境变量入口和产物路径；拿到真实环境后只补真实服务端结果与剩余风险。
4. Phase 0 真实回放契约仍暂挂；除非 macOS 单平台需要对照 Go SDK，否则不恢复全量 Go/Rust 双栈回放。
5. Session native 资源适配器已落地，现有资源句柄边界通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-session-runtime-resources">SessionRuntimeResources</code> 持有并清理。
   <!-- code-ref: v2-session-runtime-resources -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L216 -->
6. 文件 HTTP 上传客户端边界已推进到可 mock 验证，当前领域层已有 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-file-upload-boundary">FileUploadClient</code> 和签名 PUT 请求边界；真实 endpoint 本轮暂停，除非 macOS 端到端消息发送必须依赖真实上传。
   <!-- code-ref: v2-file-upload-boundary -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L276 -->
7. 本地 fake transport 的消息发送、拉取、推送闭环已落地，Phase 7 已从单服务边界推进到 Session 内链路闭环。
8. 平台示例源码骨架、通用 session event listener 桥接、Android listener 示例桥接和已实现 Session event 到 Go 细分 listener 的映射种子已落地；macOS desktop C 示例的本地 staticlib 构建运行 smoke 已完成，且已开始消费 native storage/transport/sync 资源路径，iOS、Android、Web 平台工程构建和包装产物先暂停。

## 外部环境需求

以下项没有外部环境不能标记完成：

- 真实 OpenIM WebSocket 地址、HTTP API 地址、userID、token 和平台 ID。
- 可触发服务端推送的测试账号或自动化触发脚本。
- 图片和文件上传所需的真实凭据、签名或上传 endpoint。
- macOS 桌面 C 示例构建环境：clang、可链接的 openim-ffi macOS 动态或静态库、真实 OpenIM 服务端环境变量。
- iOS、Android、Web、wasm 示例工程构建环境暂不需要，本轮暂停。
- Go SDK 与 Rust SDK 同场景运行的双栈测试环境暂不需要，本轮暂停。

## V2 本轮验收口径

本轮可以先完成以下本地项：

- 新增本清单文档并通过 Markdown code-ref 校验。
- 新增 Phase 0 契约冻结文档、兼容测试骨架、Go SDK 源码级 public API/listener surface 自动抽取、replay transcript 校验入口、绑定回调契约、replay-capture 工具、Rust 本地 session lifecycle 采集入口、Rust 真实 transport probe 采集入口、Go SDK 真实场景回放 harness 源码入口、Go harness 本地编译检查和真实 Gate 就绪检查入口。
- 新增 Phase 5 对象上传 API、签名 PUT 请求和 mock 上传验证。
- 新增 Phase 7 Session 内本地 fake transport 消息发送、拉取、推送和会话更新闭环。
- 新增 Phase 8 原生 C ABI 和 wasm 导出 crate 骨架、句柄模型和基础生命周期 API。
- 新增 Phase 8 平台示例源码骨架、通用 session event listener 桥接、Android listener 示例桥接、已实现 Session event 到 Go 细分 listener 的映射种子和示例 API 漂移检查；并已在 Darwin 本机完成 macOS desktop C 示例 staticlib 构建、链接和生命周期运行 smoke，且 desktop C 路径已接入 native resource adapter，下一轮只补真实服务端验证。
- 保持 Rust workspace 检查通过。

本轮不能宣称完成以下项：

- 真实服务端协议 Gate。
- 真实上传端到端 Gate。
- 非 macOS 平台示例构建、打包和交付 Gate。
- Go SDK 细分 listener 全量映射和平台包装层派发 Gate。
- Go 与 Rust 双栈替换 Gate。
