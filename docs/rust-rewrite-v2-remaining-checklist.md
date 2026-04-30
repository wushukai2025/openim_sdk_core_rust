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

当前仓库的 Rust workspace 已落地核心 crate 列表；绑定层尚未落地，兼容测试 crate 已在 R2-00 继续补齐，并已具备 Go SDK 源码级 public API/listener surface 自动抽取和 replay transcript 校验入口。
<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-workspace-members">workspace members</code>
<!-- code-ref: v2-workspace-members -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/Cargo.toml#L2 -->

仓库规则要求 Phase 报告必须保留真实剩余 Gate，不能把离线验证说成端到端完成。
<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-agents-gate-rule">AGENTS Gate rule</code>
<!-- code-ref: v2-agents-gate-rule -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/AGENTS.md#L58 -->

## V2 剩余任务总表

| 编号 | 阶段 | 状态 | 剩余任务 | 完成标准 |
| --- | --- | --- | --- | --- |
| R2-00 | Phase 0 | 本轮继续推进 | 已补契约冻结报告、Golden Fixture、兼容测试骨架、Go SDK 源码级 public API/listener surface 自动抽取和 replay transcript 校验入口；仍需补真实 Go SDK 回放采集器、真实服务端 Golden Event、Rust 同场景采集和绑定层回调语义冻结 | 先以本地源码抽取、fixture 校验和 transcript validator 通过，后续扩成真实运行回放契约冻结 |
| R2-01 | Phase 1 | 外部 Gate | 用真实 OpenIM 服务端完成登录、请求响应和推送收包 | 真实账号下协议 POC 命令通过并更新报告 |
| R2-02 | Phase 4 | 外部 Gate | 真实服务端 native 和 wasm 兼容收发、前后台切换验证 | ignored 真实服务端测试被实际执行并留存结果 |
| R2-03 | Phase 5 | 可本地推进加外部 Gate | 补真实 HTTP 上传客户端边界和上传凭据流程 | 本地 mock 覆盖端点语义，真实端点再做端到端上传 |
| R2-04 | Phase 6 | 可本地推进 | 接入真实资源装配适配器框架，打开和关闭 storage，挂接 transport 任务边界 | Session 登录会真正创建资源句柄，并能在 Logout 和 UnInit 清理 |
| R2-05 | Phase 6 | 外部 Gate | 登录接口 HTTP 校验、平台线程切换、前后台生命周期回归 | iOS、Android、Web 或桌面最小场景可复现 |
| R2-06 | Phase 7 | 可本地推进加外部 Gate | 串联上传结果、SendMsg、PullMsg、推送消息和本地会话更新 | 本地 fake transport 先闭环，真实账号再端到端联调 |
| R2-07 | Phase 7 | 外部 Gate | 撤回和已读回执 HTTP API 服务端校验 | 服务端状态、对端回调和本地状态三者一致 |
| R2-08 | Phase 8 | 可本地推进 | 创建原生 C ABI 和 wasm 导出 crate 骨架 | 产出可编译的导出层、句柄模型和基础生命周期 API |
| R2-09 | Phase 8 | 平台 Gate | 产出 iOS、Android、桌面和 Web 示例工程 | 示例工程能 Init、Login、Logout、UnInit |
| R2-10 | Phase 9 | 外部 Gate | 建立 Go 与 Rust 双栈对比报告和差异清单 | 核心场景有对比结果、已知差异、灰度和回滚方案 |

## 现有未完成 Gate 引用

- Phase 0 已新增契约冻结骨架，并已补 Go SDK 源码级 public API/listener surface 自动抽取和 replay transcript 校验入口；真实 Go SDK 回放采集、真实服务端 Golden Event、Rust 同场景采集和绑定层回调语义冻结仍未完成。
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase0-report">Phase 0 contract freeze</code>
  <!-- code-ref: v2-phase0-report -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/docs/phase-0-contract-freeze.md#L1 -->

- Phase 0 的源码级 surface 自动抽取落在兼容测试 crate，当前本机 Go SDK 源码存在时会冻结 134 个 open_im_sdk 导出函数和 14 个 listener interface。
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase0-source-extractor">Go source contract extractor</code>
  <!-- code-ref: v2-phase0-source-extractor -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L120 -->

- Phase 0 的 replay transcript 校验入口已落在兼容测试 crate，真实 Go/Rust 回放采集文件后续需要喂给 ignored Gate 执行。
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase0-replay-validator">Replay transcript validator</code>
  <!-- code-ref: v2-phase0-replay-validator -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L229 -->

- Phase 1 真实协议联调仍未完成，需要真实服务端、有效用户和可触发推送。
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase1-open-gate">Phase 1 open Gate</code>
  <!-- code-ref: v2-phase1-open-gate -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/docs/phase-1-protocol-diff.md#L80 -->

- Phase 4 传输层本地 native 和 wasm 已验证，但真实 OpenIM server 兼容收发和前后台行为还没验证。
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase4-open-gate">Phase 4 open Gate</code>
  <!-- code-ref: v2-phase4-open-gate -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/docs/phase-4-transport-layer.md#L133 -->

- Phase 6 目前仍是资源装配骨架，真实 native 和 wasm transport 任务、真实 storage 打开关闭、同步任务和登录 HTTP 校验未接入。
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase6-open-gate">Phase 6 open Gate</code>
  <!-- code-ref: v2-phase6-open-gate -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/docs/phase-6-session-runtime.md#L113 -->

- Phase 7 已具备离线消息和会话集成边界，但真实 SendMsg、PullMsg、推送回调、上传到发送、撤回和已读回执仍未端到端完成。
  <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-phase7-open-gate">Phase 7 open Gate</code>
  <!-- code-ref: v2-phase7-open-gate -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/docs/phase-7-message-conversation.md#L163 -->

## 本地优先执行顺序

1. 继续补 Phase 0 真实回放契约：在现有源码级 surface 抽取和 transcript validator 基础上，补 Go SDK 自动回放采集器、真实服务端 Golden Event、Rust 同场景采集和绑定层回调语义冻结。
2. 再补 Session 真实资源适配器框架。现有边界已经通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-session-resource-adapter">SessionResourceAdapter</code> 留出入口。
   <!-- code-ref: v2-session-resource-adapter -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L261 -->
3. 再补文件 HTTP 上传客户端边界。当前领域层已有 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="v2-file-upload-boundary">FileUploadClient</code>，下一步应落真实 HTTP 请求语义和 mock 验证。
   <!-- code-ref: v2-file-upload-boundary -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L69 -->
4. 再补本地 fake transport 的消息发送、拉取、推送闭环，把 Phase 7 从单服务边界推进到 Session 内链路闭环。
5. 最后进入绑定层骨架、平台示例和双栈验证。绑定层可以先做编译和生命周期 API，真实平台示例必须后续单独验证。

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
- 新增 Phase 0 契约冻结文档、兼容测试骨架、Go SDK 源码级 public API/listener surface 自动抽取和 replay transcript 校验入口。
- 保持 Rust workspace 检查通过。

本轮不能宣称完成以下项：

- 真实服务端协议 Gate。
- 真实上传端到端 Gate。
- 平台绑定层交付 Gate。
- Go 与 Rust 双栈替换 Gate。
