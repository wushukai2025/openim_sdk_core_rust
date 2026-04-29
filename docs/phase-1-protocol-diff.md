# Phase 1 Protocol Diff Report

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

第一阶段 POC 选择复用服务端已有 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="server-sdk-type-check">sdkType=js</code> 分支，而不是实现 Gob。服务端当前只接受空值、go、js 三种 SDK 类型；当 SDK 类型不是 go 时，会进入 JSON encoder 路径。
<!-- code-ref: server-sdk-type-check -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/open-im-server/internal/msggateway/context.go#L173 -->

Rust POC 已在 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="rust-client-config">ClientConfig</code> 中默认设置 js、gzip、isMsgResp，并按服务端所需 query 参数组装连接地址。
<!-- code-ref: rust-client-config -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport/src/lib.rs#L32 -->

## 已确认协议事实

- 服务端连接参数从 query 读取 sendID、token、platformID、operationID、compression、sdkType、sdkVersion 和 isMsgResp；其中 isMsgResp 为 true 时会在 WebSocket 建连后发送一次文本成功响应。

- 服务端 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="server-encoder-select">Encoder</code> 选择逻辑为 go 使用 Gob，其余合法 SDK 类型使用 JSON。
<!-- code-ref: server-encoder-select -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/open-im-server/internal/msggateway/client.go#L105 -->

- 服务端 JSON 请求壳字段为 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="server-req-envelope">Req</code>，响应壳字段为 Resp；Data 字段是 Go 字节切片，JSON 表现为 base64 字符串。
<!-- code-ref: server-req-envelope -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/open-im-server/internal/msggateway/message_handler.go#L43 -->

- 现有 Go SDK 原生路径拼接长连地址时没有传 sdkType，因此默认走 go/Gob；Rust POC 必须显式传 js，不能照抄 Go SDK 原生连接参数。

- 服务端认证成功后在 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="server-initial-response">ShouldSendResp</code> 分支发送首帧文本响应；Rust POC 已在连接后先消费该首帧。
<!-- code-ref: server-initial-response -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/open-im-server/internal/msggateway/ws_server.go#L529 -->

## Rust POC 落地点

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="rust-envelope">GeneralWsReq</code> 和 GeneralWsResp 对齐服务端 JSON envelope 字段，并对 Data 使用 base64 序列化以匹配 Go JSON 字节切片语义。
<!-- code-ref: rust-envelope -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-protocol/src/envelope.rs#L5 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="rust-gzip">gzip_compress</code> 和 gzip_decompress 对齐当前长连 compression=gzip 路径。
<!-- code-ref: rust-gzip -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-protocol/src/codec.rs#L24 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="rust-get-max-seq">GetMaxSeqReq</code> 和 GetMaxSeqResp 只覆盖 Phase 1 所需的最小同步请求，不扩展消息域。
<!-- code-ref: rust-get-max-seq -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-protocol/src/sdkws.rs#L5 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="rust-send-get-newest-seq">send_get_newest_seq</code> 发送 reqIdentifier 1001，并按现有 msgIncr 关联响应。
<!-- code-ref: rust-send-get-newest-seq -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport/src/lib.rs#L102 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="rust-spike-cli">protocol-spike</code> 提供 connect、get-newest-seq、listen-push 三个第一阶段验证入口。
<!-- code-ref: rust-spike-cli -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/tools/protocol-spike/src/main.rs#L17 -->

## 验证命令

```bash
cargo test --workspace
```

```bash
OPENIM_WS_ADDR="ws://127.0.0.1:10001/msg_gateway" \
OPENIM_USER_ID="user_id" \
OPENIM_TOKEN="token" \
cargo run -p protocol-spike -- get-newest-seq --platform-id 5 --wait-push-seconds 30
```

```bash
OPENIM_WS_ADDR="ws://127.0.0.1:10001/msg_gateway" \
OPENIM_USER_ID="user_id" \
OPENIM_TOKEN="token" \
cargo run -p protocol-spike -- listen-push --platform-id 5 --timeout-seconds 60
```

## Gate 状态

当前已完成离线 Gate：JSON envelope、Data base64 语义、gzip 往返、GetMaxSeq protobuf 字段、连接 query 组装均有单元测试覆盖。

当前未完成联调 Gate：需要真实 OpenIM 服务端、有效 userID、token 和可触发的推送消息后，才能确认登录成功、一个请求响应成功、至少一种推送消息成功接收。
