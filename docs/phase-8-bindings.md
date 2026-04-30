# Phase 8 Bindings Report

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

Phase 8 已先落地可本地编译的绑定层骨架：原生 C ABI crate、wasm-bindgen crate、句柄模型和基础生命周期 API 已进入 workspace，并通过本地单元测试。当前仍不冒充平台交付：iOS、Android、桌面和 Web 示例工程，真实平台打包产物，监听器完整回调派发和真实服务端端到端验证仍属于后续 Gate。

## Rust 落地点

- workspace 已加入 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-workspace-ffi-member">openim-ffi</code> 和 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-workspace-wasm-member">openim-wasm</code>，绑定层从独立 crate 接入，不污染领域、传输和 session crate。
<!-- code-ref: phase8-workspace-ffi-member -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/Cargo.toml#L6 -->
<!-- code-ref: phase8-workspace-wasm-member -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/Cargo.toml#L19 -->

- 原生导出 crate 通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-crate-type">crate-type</code> 同时产出 cdylib、staticlib 和 rlib，便于后续平台打包与本地测试共用同一实现。
<!-- code-ref: phase8-ffi-crate-type -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/Cargo.toml#L8 -->

- C ABI 句柄由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-handle">OpenImFfiSession</code> 封装，内部持有 OpenImSession 和 last error 字符串，对外只暴露 opaque pointer。
<!-- code-ref: phase8-ffi-handle -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L18 -->

- C ABI 生命周期入口已覆盖 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-create">openim_session_create</code>、<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-init">openim_session_init</code>、<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-login">openim_session_login</code>、<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-logout">openim_session_logout</code> 和 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-uninit">openim_session_uninit</code>。
<!-- code-ref: phase8-ffi-create -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L34 -->
<!-- code-ref: phase8-ffi-init -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L67 -->
<!-- code-ref: phase8-ffi-login -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L72 -->
<!-- code-ref: phase8-ffi-logout -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L92 -->
<!-- code-ref: phase8-ffi-uninit -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L97 -->

- C ABI 状态和错误读取通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-state">openim_session_state</code> 与 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-last-error">openim_session_last_error</code> 固定，空句柄和非法输入会返回稳定错误码。
<!-- code-ref: phase8-ffi-state -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L102 -->
<!-- code-ref: phase8-ffi-last-error -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L110 -->

- 原生回调线程策略通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-native-callback-thread">openim_native_callback_thread_policy</code> 暴露，值与 Phase 0 固定的 sdk_serialized_callback_queue 契约一致。
<!-- code-ref: phase8-native-callback-thread -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L29 -->

- wasm 导出 crate 通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-wasm-crate-type">crate-type</code> 产出 cdylib 和 rlib，并复用 workspace 的 wasm-bindgen 依赖。
<!-- code-ref: phase8-wasm-crate-type -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-wasm/Cargo.toml#L8 -->

- wasm 句柄由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-wasm-session">OpenImWasmSession</code> 封装，对 JS 暴露 constructor、init、login、logout、uninit、stateCode、loginUserId 和 callbackThreadPolicy。
<!-- code-ref: phase8-wasm-session -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-wasm/src/lib.rs#L8 -->

- wasm 生命周期实现直接复用 OpenImSession：<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-wasm-new">new</code> 校验 platform id 并创建 session，<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-wasm-login">login</code> 复用 LoginCredentials，错误统一转换为 JsValue。
<!-- code-ref: phase8-wasm-new -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-wasm/src/lib.rs#L15 -->
<!-- code-ref: phase8-wasm-login -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-wasm/src/lib.rs#L27 -->

- 本地测试已固定 C ABI 和 wasm 基础生命周期：<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-lifecycle-test">c_abi_session_lifecycle_uses_opaque_handle</code> 覆盖 opaque pointer 状态流转，<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-wasm-lifecycle-test">wasm_session_lifecycle_exports_basic_state</code> 覆盖 wasm 包装状态流转。
<!-- code-ref: phase8-ffi-lifecycle-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L186 -->
<!-- code-ref: phase8-wasm-lifecycle-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-wasm/src/lib.rs#L76 -->

## 验证命令

```bash
cargo fmt --all --check
```

```bash
cargo test -p openim-ffi
```

```bash
cargo test -p openim-wasm
```

```bash
cargo check -p openim-wasm --target wasm32-unknown-unknown
```

```bash
cargo check --workspace
```

```bash
cargo test --workspace
```

## Gate 状态

当前已完成：C ABI crate 骨架、wasm-bindgen crate 骨架、opaque 句柄模型、基础生命周期 API、状态码读取、last error 读取、native 和 wasm 回调线程策略常量、本地 C ABI 单元测试、本地 wasm 单元测试、wasm32 编译检查和 workspace 回归。

当前未完成：iOS、Android、桌面和 Web 示例工程；C header、Swift/Kotlin/TypeScript 包装产物；完整 listener 注册和回调派发；真实平台线程切换；真实服务端 Init、Login、Logout、UnInit 端到端验证。这些能力继续留在 R2-09 和后续平台 Gate。
