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

Phase 8 已先落地可本地编译的绑定层骨架：原生 C ABI crate、wasm-bindgen crate、句柄模型、基础生命周期 API、通用 session event listener 桥接、C header、desktop C、iOS Swift、Android Kotlin/JNI 和 Web TypeScript 生命周期示例源码已进入 workspace，并通过本地单元测试覆盖示例 API 漂移。当前已冻结已实现 Session event 到 Go 细分 listener 的第一批映射种子；本轮继续把 desktop C 示例从“只有源码”推进到“Darwin 本机可实际 clang 链接 `openim-ffi` staticlib 并运行生命周期 smoke”，并进一步把 native C ABI 创建路径接到 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-native-adapter">NativeSessionResourceAdapter</code>，支持通过 `openim_session_create_with_data_dir` 和 `OPENIM_DATA_DIR` 打开 SQLite storage 资源。仍不冒充平台交付：真实平台工程构建、真实平台打包产物、Go SDK 细分 listener 全量映射和真实服务端端到端验证仍属于后续 Gate。
<!-- code-ref: phase8-ffi-native-adapter -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L75 -->

## Rust 落地点

- workspace 已加入 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-workspace-ffi-member">openim-ffi</code> 和 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-workspace-wasm-member">openim-wasm</code>，绑定层从独立 crate 接入，不污染领域、传输和 session crate。
<!-- code-ref: phase8-workspace-ffi-member -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/Cargo.toml#L6 -->
<!-- code-ref: phase8-workspace-wasm-member -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/Cargo.toml#L19 -->

- 原生导出 crate 通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-crate-type">crate-type</code> 同时产出 cdylib、staticlib 和 rlib，便于后续平台打包与本地测试共用同一实现。
<!-- code-ref: phase8-ffi-crate-type -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/Cargo.toml#L8 -->

- C ABI 句柄由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-handle">OpenImFfiSession</code> 封装，内部持有 OpenImSession 和 last error 字符串，对外只暴露 opaque pointer。
<!-- code-ref: phase8-ffi-handle -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L26 -->

- C ABI 生命周期入口已覆盖 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-create">openim_session_create</code>、<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-create-with-data-dir">openim_session_create_with_data_dir</code>、<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-init">openim_session_init</code>、<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-login">openim_session_login</code>、<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-logout">openim_session_logout</code> 和 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-uninit">openim_session_uninit</code>。
<!-- code-ref: phase8-ffi-create -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L42 -->
<!-- code-ref: phase8-ffi-create-with-data-dir -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L51 -->
<!-- code-ref: phase8-ffi-init -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L95 -->
<!-- code-ref: phase8-ffi-login -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L100 -->
<!-- code-ref: phase8-ffi-logout -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L120 -->
<!-- code-ref: phase8-ffi-uninit -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L125 -->

- C ABI 状态和错误读取通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-state">openim_session_state</code> 与 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-last-error">openim_session_last_error</code> 固定，空句柄和非法输入会返回稳定错误码。
<!-- code-ref: phase8-ffi-state -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L130 -->
<!-- code-ref: phase8-ffi-last-error -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L138 -->

- 原生回调线程策略通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-native-callback-thread">openim_native_callback_thread_policy</code> 暴露，值与 Phase 0 固定的 sdk_serialized_callback_queue 契约一致。
<!-- code-ref: phase8-native-callback-thread -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L37 -->

- 原生通用 session event 回调通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-event-callback">OpenImFfiSessionEventCallback</code>、<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-register-listener">openim_session_register_listener</code> 和 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-unregister-listener">openim_session_unregister_listener</code> 暴露，事件名和 payload JSON 在回调调用期间有效。
<!-- code-ref: phase8-ffi-event-callback -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L23 -->
<!-- code-ref: phase8-ffi-register-listener -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L148 -->
<!-- code-ref: phase8-ffi-unregister-listener -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L179 -->

- wasm 导出 crate 通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-wasm-crate-type">crate-type</code> 产出 cdylib 和 rlib，并复用 workspace 的 wasm-bindgen 依赖。
<!-- code-ref: phase8-wasm-crate-type -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-wasm/Cargo.toml#L8 -->

- wasm 句柄由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-wasm-session">OpenImWasmSession</code> 封装，对 JS 暴露 constructor、init、login、logout、uninit、stateCode、loginUserId、callbackThreadPolicy、addListener、removeListener 和 listenerCount。
<!-- code-ref: phase8-wasm-session -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-wasm/src/lib.rs#L16 -->

- wasm 生命周期实现直接复用 OpenImSession：<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-wasm-new">new</code> 校验 platform id 并创建 session，<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-wasm-login">login</code> 复用 LoginCredentials，错误统一转换为 JsValue。
<!-- code-ref: phase8-wasm-new -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-wasm/src/lib.rs#L27 -->
<!-- code-ref: phase8-wasm-login -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-wasm/src/lib.rs#L53 -->

- wasm 通用 session event listener 由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-wasm-add-listener">addListener</code> 和 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-wasm-remove-listener">removeListener</code> 暴露，回调参数保持 event name 与 payload JSON 字符串。
<!-- code-ref: phase8-wasm-add-listener -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-wasm/src/lib.rs#L106 -->
<!-- code-ref: phase8-wasm-remove-listener -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-wasm/src/lib.rs#L119 -->

- 本地测试已固定 C ABI 和 wasm 基础生命周期：<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-lifecycle-test">c_abi_session_lifecycle_uses_opaque_handle</code> 覆盖 opaque pointer 状态流转，<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-wasm-lifecycle-test">wasm_session_lifecycle_exports_basic_state</code> 覆盖 wasm 包装状态流转。
<!-- code-ref: phase8-ffi-lifecycle-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L348 -->
<!-- code-ref: phase8-wasm-lifecycle-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-wasm/src/lib.rs#L176 -->

- 本地 C ABI listener 测试 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-listener-test">c_abi_listener_dispatches_lifecycle_events</code> 固定 listenerRegistered、initialized、task/resource 事件和 loggedIn/loggedOut 顺序；<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-storage-test">c_abi_create_with_data_dir_opens_native_storage_resource</code> 会额外验证 `data_dir` 打开 SQLite storage。
<!-- code-ref: phase8-ffi-listener-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L435 -->
<!-- code-ref: phase8-ffi-storage-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L501 -->

- 已实现 Session event 到 Go 细分 listener 的映射种子由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-session-listener-mapping">session_event_listener_mappings</code> 固定，当前只覆盖 NewMessages -> OnAdvancedMsgListener.OnRecvNewMessage 和 ConversationChanged -> OnConversationListener.OnConversationChanged；<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-session-listener-mapping-test">session_event_listener_mapping_covers_current_session_events</code> 会对 fixture 校验，源码存在时还会对 Go callback_client.go 自动抽取结果校验。
<!-- code-ref: phase8-session-listener-mapping -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L435 -->
<!-- code-ref: phase8-session-listener-mapping-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-compat-tests/src/lib.rs#L815 -->

- C ABI 对外声明已补 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ffi-header">openim_ffi.h</code>，固定平台 ID、错误码、opaque handle，并新增带 `data_dir` 的 native 创建入口。
<!-- code-ref: phase8-ffi-header -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/include/openim_ffi.h#L1 -->

- desktop 示例源码 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-desktop-example">openim_desktop_lifecycle.c</code> 直接调用 C ABI，固定 `OPENIM_PLATFORM_MACOS`，支持通过 `OPENIM_API_ADDR`、`OPENIM_WS_ADDR`、`OPENIM_USER_ID`、`OPENIM_TOKEN` 和可选 `OPENIM_DATA_DIR` 覆盖默认值，并通过 `openim_session_create_with_data_dir` 进入 native 资源路径。
<!-- code-ref: phase8-desktop-example -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/examples/desktop-c/openim_desktop_lifecycle.c#L1 -->

- iOS 示例源码 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-ios-example">OpenIMLifecycleExample.swift</code> 通过 bridging header 使用 C ABI，注册通用 session event callback，并在 deinit 中清理 session handle。
<!-- code-ref: phase8-ios-example -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/examples/ios-swift/OpenIMLifecycleExample.swift#L18 -->

- Android 示例源码由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-android-kotlin-example">OpenIMLifecycleExample.kt</code> 和 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-android-jni-example">openim_jni_lifecycle.cc</code> 组成，Kotlin 侧走 external 方法并注册 OpenIMSessionEventListener，JNI 侧转发 lifecycle 与 listener 注册/注销到 C ABI。
<!-- code-ref: phase8-android-kotlin-example -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/examples/android-kotlin/OpenIMLifecycleExample.kt#L3 -->
<!-- code-ref: phase8-android-jni-example -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/examples/android-kotlin/openim_jni_lifecycle.cc#L1 -->

- Web 示例源码 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-web-example">openim_lifecycle.ts</code> 使用 wasm-bindgen 生成的 OpenImWasmSession，覆盖 addListener、init、login、logout、uninit、removeListener 和 stateCode。
<!-- code-ref: phase8-web-example -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/examples/web/openim_lifecycle.ts#L1 -->

- 示例 API 漂移检查已进入单元测试：<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-native-example-test">native_header_and_examples_cover_lifecycle_exports</code> 校验 C header、desktop、iOS 和 Android 示例，<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-web-example-test">web_example_uses_wasm_lifecycle_exports</code> 校验 Web 示例。
<!-- code-ref: phase8-native-example-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L530 -->
<!-- code-ref: phase8-web-example-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-wasm/src/lib.rs#L203 -->

- Darwin 本地新增 desktop C 示例实际构建运行 smoke：测试会调用 `cargo build -p openim-ffi` 生成最新 staticlib，再用 `clang` 把 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase8-desktop-example-smoke">openim_desktop_lifecycle.c</code> 链接到 `openim-ffi` staticlib，并运行 listener 注册、Init、Login、Logout、UnInit 和 native resource 事件最小回归。
<!-- code-ref: phase8-desktop-example-smoke -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-ffi/src/lib.rs#L574 -->

## 验证命令

```bash
cargo fmt --all --check
```

```bash
cargo test -p openim-compat-tests
```

```bash
cargo test -p openim-ffi
```

```bash
cargo build -p openim-ffi
```

```bash
clang -I crates/openim-ffi/include examples/desktop-c/openim_desktop_lifecycle.c target/debug/libopenim_ffi.a -o /tmp/openim_desktop_lifecycle
```

```bash
/tmp/openim_desktop_lifecycle
```

```bash
OPENIM_DATA_DIR=/tmp/openim-sdk-db /tmp/openim_desktop_lifecycle
```

```bash
OPENIM_DATA_DIR=/tmp/openim-sdk-db OPENIM_API_ADDR=... OPENIM_WS_ADDR=... OPENIM_USER_ID=... OPENIM_TOKEN=... /tmp/openim_desktop_lifecycle
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

当前已完成：C ABI crate 骨架、wasm-bindgen crate 骨架、opaque 句柄模型、基础生命周期 API、状态码读取、last error 读取、native 和 wasm 回调线程策略常量、通用 session event listener 注册/注销、native C ABI 通过 `NativeSessionResourceAdapter` 进入 storage/transport/sync 资源路径、`openim_session_create_with_data_dir` 和 `OPENIM_DATA_DIR` 入口、C header、desktop C 示例源码、Darwin 本地 staticlib 构建链接运行 smoke、iOS Swift 示例源码、Android Kotlin/JNI 生命周期与 listener 示例源码、Web TypeScript 示例源码、已实现 Session event 到 Go 细分 listener 的映射种子、本地 C ABI 单元测试、本地 wasm 单元测试、listener 生命周期派发测试、示例 API 漂移检查、wasm32 编译检查和 workspace 回归。

当前未完成：iOS、Android 和 Web 示例工程的真实平台构建；Swift/Kotlin/TypeScript 包装产物发布；Go SDK 细分 listener 全量映射和平台包装层派发；真实平台线程切换；macOS desktop C 示例在真实服务端环境下的 Init、Login、Logout、UnInit 端到端验证。这些能力继续留在 R2-09 平台 Gate 和后续真实集成 Gate。
