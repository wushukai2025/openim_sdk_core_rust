# Phase 2 Foundation Layer Report

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

Phase 2 已完成基础能力层的可复用骨架：错误码、公共类型、协议 envelope 与版本同步算法都已独立成包，并通过 workspace 单元测试。该阶段不引入领域服务，也不把消息、会话、登录态提前接入，避免基础包被上层生命周期绑死。

## 已完成范围

- 错误体系以 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase2-error-code">ErrorCode</code> 保留 Go SDK 的错误码分段，并用 OpenImError 统一错误对象。
<!-- code-ref: phase2-error-code -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-errors/src/lib.rs#L5 -->

- 公共类型层通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase2-numeric-enum">numeric_i32_enum</code> 保证平台、会话类型、消息类型和消息状态按数字序列化，而不是按 Rust 枚举名序列化。
<!-- code-ref: phase2-numeric-enum -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-types/src/lib.rs#L3 -->

- 分页和版本快照落在 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase2-version-state">VersionState</code>，为后续存储层和同步层共享同一份版本状态模型。
<!-- code-ref: phase2-version-state -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-types/src/lib.rs#L153 -->

- 协议 envelope 以 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase2-general-ws-req">GeneralWsReq</code> 和 GeneralWsResp 表达 JSON 外壳，Data 字段沿用 Go JSON 字节切片的 base64 语义。
<!-- code-ref: phase2-general-ws-req -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-protocol/src/envelope.rs#L5 -->

- 最小 protobuf 模型覆盖 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase2-get-max-seq-req">GetMaxSeqReq</code>、GetMaxSeqResp 和 RequestPagination，服务于协议 POC 与同步基础验证。
<!-- code-ref: phase2-get-max-seq-req -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-protocol/src/sdkws.rs#L5 -->

- 同步层提供 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase2-diff-by">diff_by</code> 做通用列表差异规划，并提供 plan_version_sync 处理版本号、versionID、全量和增量分支。
<!-- code-ref: phase2-diff-by -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-sync/src/lib.rs#L63 -->

## 验证命令

```bash
cargo fmt --all --check
```

```bash
cargo test --workspace
```

## Gate 状态

Phase 2 离线 Gate 已通过：基础包都可独立单测，协议 JSON envelope、gzip、protobuf 字段号、错误码分类、数字枚举序列化、分页默认值和版本同步分支均有测试覆盖。

未纳入 Phase 2 的内容：完整 API 契约、领域服务、会话生命周期、真实服务端联调和平台绑定。这些继续留到后续阶段处理。

