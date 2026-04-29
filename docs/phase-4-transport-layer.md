# Phase 4 Transport Layer Report

更新时间：2026-04-29

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

Phase 4 已启动并完成第一批传输层拆包：workspace 增加 transport-core、transport-native、transport-wasm 三个 crate，原有 transport crate 改为兼容 facade。当前已通过 native workspace 回归和 wasm32 编译检查，但浏览器真实 WebSocket 断线、重连、前后台切换自动化 Gate 尚未执行，因此 Phase 4 Gate 不能标记为全部通过。

## Rust 落地点

- workspace 已加入 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase4-workspace-transport-members">openim-transport-core</code>、openim-transport-native 和 openim-transport-wasm，保持传输核心、原生实现和浏览器实现的物理边界。
<!-- code-ref: phase4-workspace-transport-members -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/Cargo.toml#L10 -->

- facade 继续导出 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase4-facade-export">OpenImWsClient</code> 与 ClientConfig，保证 protocol-spike 的现有导入路径不变。
<!-- code-ref: phase4-facade-export -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport/src/lib.rs#L1 -->

- transport-core 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase4-transport-config">TransportConfig</code> 保留 Phase 1 连接参数和 URL query 生成规则，包括 sendID、token、platformID、operationID、sdkType、compression 和 isMsgResp。
<!-- code-ref: phase4-transport-config -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport-core/src/lib.rs#L17 -->

- transport-core 新增 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase4-transport-event">TransportEvent</code>，统一表达响应、推送、心跳、断线、重连计划和请求超时。
<!-- code-ref: phase4-transport-event -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport-core/src/lib.rs#L78 -->

- transport-core 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase4-reconnect-policy">ReconnectPolicy</code> 提供指数退避与最大延迟封顶，用于 native 和后续 wasm 重连调度复用。
<!-- code-ref: phase4-reconnect-policy -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport-core/src/lib.rs#L105 -->

- transport-core 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase4-pending-requests">PendingRequests</code> 负责 msgIncr 请求登记、响应消解和超时移除。
<!-- code-ref: phase4-pending-requests -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport-core/src/lib.rs#L141 -->

- transport-core 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase4-text-heartbeat">text_heartbeat_frame</code> 统一处理 OpenIM 文本 ping 到 pong 的心跳语义，同时识别文本 pong。
<!-- code-ref: phase4-text-heartbeat -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport-core/src/lib.rs#L204 -->

- transport-core 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase4-encode-request-payload">encode_request_payload</code> 与 decode_response_payload 统一 JSON envelope 和 gzip 编解码入口。
<!-- code-ref: phase4-encode-request-payload -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport-core/src/lib.rs#L233 -->

- transport-core 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase4-route-envelope">route_envelope</code> 先按 PushMsg 转推送，再按 msgIncr 命中 pending 请求来区分响应与非请求事件。
<!-- code-ref: phase4-route-envelope -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport-core/src/lib.rs#L272 -->

- transport-native 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase4-native-client">NativeWsClient</code> 承接原 tokio-tungstenite 实现，并继续提供 OpenImWsClient 类型别名。
<!-- code-ref: phase4-native-client -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport-native/src/lib.rs#L23 -->

- transport-native 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase4-native-reconnect">reconnect</code> 按相同配置重建连接，鉴权响应通过后清空 pending 请求，避免旧请求在新连接上误配响应。
<!-- code-ref: phase4-native-reconnect -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport-native/src/lib.rs#L82 -->

- transport-native 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase4-native-send-request">send_request</code> 发送前复用核心 payload 编码，发送后登记 msgIncr，用于后续响应关联。
<!-- code-ref: phase4-native-send-request -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport-native/src/lib.rs#L121 -->

- transport-native 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase4-native-timeout">recv_event_with_timeout</code> 将接收等待超时映射为 RequestTimeout 事件，覆盖 native 侧基础超时路径。
<!-- code-ref: phase4-native-timeout -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport-native/src/lib.rs#L177 -->

- transport-wasm 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase4-wasm-client">WasmWsClient</code> 基于 web-sys WebSocket 建立浏览器适配，支持二进制帧、文本心跳、关闭事件和错误事件进入统一事件流。
<!-- code-ref: phase4-wasm-client -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport-wasm/src/wasm.rs#L27 -->

- transport-wasm 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase4-wasm-handlers">install_handlers</code> 持有 WebSocket 回调闭包，避免浏览器事件处理器被 Rust 提前释放。
<!-- code-ref: phase4-wasm-handlers -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport-wasm/src/wasm.rs#L181 -->

- native 集成测试 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase4-native-route-test">native_client_routes_response_push_and_text_heartbeat</code> 启动本地 WebSocket fixture，覆盖文本心跳、请求响应关联和推送转发。
<!-- code-ref: phase4-native-route-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport-native/src/lib.rs#L246 -->

- native 集成测试 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase4-native-reconnect-test">native_client_reconnects_after_disconnect</code> 覆盖首连接断开后通过 reconnect 重新建连并完成请求响应。
<!-- code-ref: phase4-native-reconnect-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport-native/src/lib.rs#L286 -->

## 验证命令

```bash
cargo fmt --all --check
```

```bash
cargo test -p openim-transport-native
```

```bash
cargo test --workspace
```

```bash
cargo check -p openim-transport-wasm --target wasm32-unknown-unknown
```

## Gate 状态

当前已完成：传输层三 crate 拆分、旧 facade 兼容导出、连接 URL 行为保持、JSON 与 gzip payload 复用、文本心跳 ping 到 pong、native 请求登记与响应关联、native 推送转发、native 接收超时事件、native 重连入口、本地 WebSocket 断线重连测试、wasm WebSocket 编译通过、workspace 回归通过。

当前未完成：浏览器真实 WebSocket send 和 receive 自动化执行、wasm 断线重连策略接入、前后台切换基础场景测试、真实 OpenIM server 兼容收发回归。因此 Phase 4 仍处于进行中，不能标记 Gate 全部通过。
