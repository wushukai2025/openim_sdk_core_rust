# Phase 6 Session Runtime Report

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

Phase 6 已从 Session 装配骨架推进到 native 资源句柄闭环：生命周期状态、登录凭据、transport 配置生成、storage target 生成、领域服务聚合、监听器注册表、任务监督器、资源适配器边界、登录态资源清理、Session 运行时资源句柄、native SQLite 打开与迁移、transport/sync 任务句柄和单元测试均已落地。本轮继续把 native transport 从“只有 connect_url 的占位句柄”推进到“登录时真实创建 `NativeWsClient` 连接资源并在退出时主动 close”，并补上基于 `/auth/parse_token` 的本地 HTTP 登录校验，在 fixture 下验证 token、userID 和 platformID 对齐。当前实现仍不冒充完整长连任务：wasm IndexedDB 运行时装配、native 收包与重连后台任务、平台回调线程和真实 API endpoint 联调仍属于后续 Gate。

## Rust 落地点

- workspace 已加入 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-workspace-session-member">openim-session</code>，作为 Phase 6 生命周期和资源装配入口。
<!-- code-ref: phase6-workspace-session-member -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/Cargo.toml#L6 -->

- workspace 新增 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-workspace-session-native-member">openim-session-native</code>，先承接 native SQLite 资源打开、迁移和 transport/sync 任务句柄装配。
<!-- code-ref: phase6-workspace-session-native-member -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/Cargo.toml#L8 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-session-deps">openim-session</code> 依赖 domain、errors、storage-core、transport-core 和 types，只接入配置与 trait 边界，不直接持有 native 或 wasm 具体实现。
<!-- code-ref: phase6-session-deps -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/Cargo.toml#L9 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-session-config">SessionConfig</code> 记录 platform、api address、websocket address 和可选 data directory，并在创建 Session 时做基础参数校验。
<!-- code-ref: phase6-session-config -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L21 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-transport-config">transport_config</code> 从 Session 配置和登录凭据生成 transport-core 的连接配置，确保 ws address、user id、token 和 platform id 透传到传输层。
<!-- code-ref: phase6-transport-config -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L57 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-storage-target-fn">storage_target</code> 根据 platform 和 login user 生成 SQLite 文件路径或 IndexedDB 名称，复用 storage-core 的命名规则。
<!-- code-ref: phase6-storage-target-fn -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L67 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-login-credentials">LoginCredentials</code> 明确登录态输入边界，user id 和 token 为空会被拒绝。
<!-- code-ref: phase6-login-credentials -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L84 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-session-state">SessionState</code> 显式表达 Created、Initialized、LoggedIn、LoggedOut 和 Uninitialized，后续真实资源接入时可以沿用同一状态机。
<!-- code-ref: phase6-session-state -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L104 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-session-event">SessionEvent</code> 先覆盖初始化、登录、登出、反初始化、监听器注册注销和任务启停事件。
<!-- code-ref: phase6-session-event -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L120 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-resource-kind">SessionResourceKind</code> 和 ResourceOpened/ResourceClosed 事件把 storage、transport、sync 资源边界显式暴露给监听器。
<!-- code-ref: phase6-resource-kind -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L113 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-storage-target">StorageTarget</code> 统一表达未配置、本地 SQLite 和 Web IndexedDB 三种存储目标。
<!-- code-ref: phase6-storage-target -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L158 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-runtime-resources">SessionRuntimeResources</code> 保存登录态 user、transport config、storage target 和 adapter 返回的资源句柄，并在清理时反向关闭句柄。
<!-- code-ref: phase6-runtime-resources -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L216 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-domain-services">DomainServices</code> 聚合 Phase 5 的用户、关系和群组服务，Session 可在登录态下统一访问领域服务。
<!-- code-ref: phase6-domain-services -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L315 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-task-supervisor">TaskSupervisor</code> 提供任务启动、停止、查询和枚举能力，用于先固定多任务资源的启停语义。
<!-- code-ref: phase6-task-supervisor -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L330 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-task-stop-all">stop_all</code> 会返回本次真正停止的任务名并清空任务表，确保 Logout 和 UnInit 清理幂等。
<!-- code-ref: phase6-task-stop-all -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L359 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-listener-registry">ListenerRegistry</code> 按 listener id 管理回调，注销后不再接收后续事件。
<!-- code-ref: phase6-listener-registry -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L386 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-resource-adapter">SessionResourceAdapter</code> 固定真实资源接入点，Init、Login、Logout 和 UnInit 会把配置、凭据、transport config 与 storage target 传递给适配器；Login 现在必须返回 SessionRuntimeResources，避免资源只在 adapter 内部隐式存在。
<!-- code-ref: phase6-resource-adapter -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L423 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-openim-session">OpenImSession</code> 持有 config、state、login user、domain services、listener registry、task supervisor 和 runtime resources，是后续绑定层和真实资源接入的统一入口。
<!-- code-ref: phase6-openim-session -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L467 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-with-resource-adapter">with_resource_adapter</code> 允许测试和后续平台绑定注入真实资源适配器，同时保留默认 no-op 适配器。
<!-- code-ref: phase6-with-resource-adapter -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L483 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-init">init</code> 支持 Created 和 Uninitialized 到 Initialized 的转换，重复初始化不会重复启动资源。
<!-- code-ref: phase6-init -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L503 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-login">login</code> 要求 Session 已初始化，登录成功后生成 transport config 和 storage target，调用资源适配器取得 runtime resources，重置登录态领域服务并启动 transport 和 sync 两类任务占位。
<!-- code-ref: phase6-login -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L512 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-logout">logout</code> 会关闭 runtime resources、停止所有任务、清空 login user、重置领域服务并进入 LoggedOut，重复调用保持安全。
<!-- code-ref: phase6-logout -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L547 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-uninit">uninit</code> 会执行和登出同级别的 runtime resources 清理，并进入 Uninitialized，作为 SDK 实例销毁的统一入口。
<!-- code-ref: phase6-uninit -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L568 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-register-listener">register_listener</code> 注册事件回调并广播注册事件。
<!-- code-ref: phase6-register-listener -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L585 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-unregister-listener">unregister_listener</code> 先移除回调再广播注销事件，确保被注销 listener 不会收到自己的注销事件。
<!-- code-ref: phase6-unregister-listener -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L594 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-domains-mut">domains_mut</code> 只允许 LoggedIn 状态访问可变领域服务，避免未登录或登出后继续写入登录态缓存。
<!-- code-ref: phase6-domains-mut -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L655 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-native-resource-adapter">NativeSessionResourceAdapter</code> 在 native target 下打开 SQLite storage、执行迁移，并创建真实 `NativeWsClient` transport 资源与 sync 任务资源句柄。
<!-- code-ref: phase6-native-resource-adapter -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session-native/src/lib.rs#L17 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-open-sqlite-storage">open_sqlite_storage</code> 会创建 SQLite 父目录、打开数据库并执行 storage migrator，作为 native 登录资源打开的本地 Gate。
<!-- code-ref: phase6-open-sqlite-storage -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session-native/src/lib.rs#L92 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-http-login-validate">validate_http_login</code> 会向 `/auth/parse_token` 发送 HTTP 请求，校验 token 对应的 userID 与 platformID 是否和当前登录凭据一致；服务端 errCode 会原样映射为 Rust 错误，避免 desktop/native 在无效 token 下继续建立 transport 资源。
<!-- code-ref: phase6-http-login-validate -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session-native/src/lib.rs#L104 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-native-transport-open">open_native_transport_client</code> 会创建 current-thread Tokio runtime，并在登录阶段直接建立 `NativeWsClient` 连接；连接失败会按 network error 返回，避免 session 把无效 transport 当作已登录资源。
<!-- code-ref: phase6-native-transport-open -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session-native/src/lib.rs#L179 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-native-transport-close">NativeTransportTaskResource::close</code> 会在资源清理阶段主动调用 `NativeWsClient::close`，作为 Logout 和 UnInit 的本地 transport 关闭边界。
<!-- code-ref: phase6-native-transport-close -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session-native/src/lib.rs#L202 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-native-resource-test">native_adapter_opens_sqlite_storage_and_closes_resources_on_logout</code>、<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-http-login-test">native_adapter_rejects_parse_token_user_mismatch</code>、<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-native-transport-fixture">spawn_transport_server</code> 和 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-http-login-fixture">spawn_parse_token_server</code> 一起验证登录时 SQLite 文件真实创建、本地 WebSocket 连接成功建立、HTTP 登录校验会拦截错误 userID，以及 Logout 后 runtime resources 被清空。
<!-- code-ref: phase6-native-resource-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session-native/src/lib.rs#L308 -->
<!-- code-ref: phase6-http-login-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session-native/src/lib.rs#L350 -->
<!-- code-ref: phase6-native-transport-fixture -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session-native/src/lib.rs#L371 -->
<!-- code-ref: phase6-http-login-fixture -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session-native/src/lib.rs#L398 -->

## 验证命令

```bash
cargo fmt --all --check
```

```bash
cargo test -p openim-session
```

```bash
cargo test -p openim-session-native
```

```bash
cargo test -p openim-transport-native
```

```bash
cargo check --workspace
```

```bash
cargo test --workspace
```

## Gate 状态

当前已完成：Init、Login、Logout、UnInit 状态转换；登录前置校验；transport config 生成；SQLite 与 IndexedDB storage target 生成；登录态 user id 保存；登录态领域服务重置；资源适配器注入；Init/Login/Logout/UnInit 资源边界回调；SessionRuntimeResources 句柄持有；ResourceOpened/ResourceClosed 事件；Logout 和 UnInit 关闭资源句柄；native SQLite 父目录创建、数据库打开和 migration；`/auth/parse_token` HTTP 登录校验；native `NativeWsClient` 连接资源创建与 close；sync 任务资源句柄；登出和反初始化任务清理；监听器注册和注销；注销后回调隔离；任务启停事件；重复 Logout 幂等；Session crate 单元测试、native session adapter 单元测试、transport-native 单元测试和 workspace check。

当前未完成：native WebSocket 收包/重连后台任务编排、wasm IndexedDB 运行时打开和关闭、真实同步任务调度、平台线程切换策略、监听器类型细分、真实 API endpoint 联调和前后台生命周期回归。因此 Phase 6 已具备 native 本地资源句柄闭环、本地 transport 连接资源闭环和本地 HTTP 登录校验闭环，但完整跨平台 Session Gate 仍需后续真实资源和平台运行器验证。
