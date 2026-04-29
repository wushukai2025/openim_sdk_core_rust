# Phase 6 Session Runtime Report

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

Phase 6 已完成 Session 装配层的最小可验证骨架：生命周期状态、登录凭据、领域服务聚合、监听器注册表、任务监督器、登录态资源清理和单元测试均已落地。当前实现先建立运行时边界和清理语义，不冒充真实长连任务、数据库连接、后台任务或平台回调线程已经接入；这些真实资源会在后续集成 Gate 中继续补齐。

## Rust 落地点

- workspace 已加入 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-workspace-session-member">openim-session</code>，作为 Phase 6 生命周期和资源装配入口。
<!-- code-ref: phase6-workspace-session-member -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/Cargo.toml#L6 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-session-deps">openim-session</code> 只依赖 domain、errors 和 types，暂不直接持有 transport 或 storage 具体实现，避免在真实 Gate 前把平台资源耦合进 Session。
<!-- code-ref: phase6-session-deps -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/Cargo.toml#L9 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-session-config">SessionConfig</code> 记录 platform、api address、websocket address 和可选 data directory，并在创建 Session 时做基础参数校验。
<!-- code-ref: phase6-session-config -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L13 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-login-credentials">LoginCredentials</code> 明确登录态输入边界，user id 和 token 为空会被拒绝。
<!-- code-ref: phase6-login-credentials -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L50 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-session-state">SessionState</code> 显式表达 Created、Initialized、LoggedIn、LoggedOut 和 Uninitialized，后续真实资源接入时可以沿用同一状态机。
<!-- code-ref: phase6-session-state -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L70 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-session-event">SessionEvent</code> 先覆盖初始化、登录、登出、反初始化、监听器注册注销和任务启停事件。
<!-- code-ref: phase6-session-event -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L79 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-domain-services">DomainServices</code> 聚合 Phase 5 的用户、关系和群组服务，Session 可在登录态下统一访问领域服务。
<!-- code-ref: phase6-domain-services -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L91 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-task-supervisor">TaskSupervisor</code> 提供任务启动、停止、查询和枚举能力，用于先固定多任务资源的启停语义。
<!-- code-ref: phase6-task-supervisor -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L104 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-task-stop-all">stop_all</code> 会返回本次真正停止的任务名并清空任务表，确保 Logout 和 UnInit 清理幂等。
<!-- code-ref: phase6-task-stop-all -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L132 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-listener-registry">ListenerRegistry</code> 按 listener id 管理回调，注销后不再接收后续事件。
<!-- code-ref: phase6-listener-registry -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L160 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-openim-session">OpenImSession</code> 持有 config、state、login user、domain services、listener registry 和 task supervisor，是后续绑定层和真实资源接入的统一入口。
<!-- code-ref: phase6-openim-session -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L199 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-init">init</code> 支持 Created 和 Uninitialized 到 Initialized 的转换，重复初始化不会重复启动资源。
<!-- code-ref: phase6-init -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L221 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-login">login</code> 要求 Session 已初始化，登录成功后重置登录态领域服务并启动 transport 和 sync 两类任务占位。
<!-- code-ref: phase6-login -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L232 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-logout">logout</code> 会停止所有任务、清空 login user、重置领域服务并进入 LoggedOut，重复调用保持安全。
<!-- code-ref: phase6-logout -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L253 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-uninit">uninit</code> 会执行和登出同级别的资源清理，并进入 Uninitialized，作为 SDK 实例销毁的统一入口。
<!-- code-ref: phase6-uninit -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L266 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-register-listener">register_listener</code> 注册事件回调并广播注册事件。
<!-- code-ref: phase6-register-listener -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L275 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-unregister-listener">unregister_listener</code> 先移除回调再广播注销事件，确保被注销 listener 不会收到自己的注销事件。
<!-- code-ref: phase6-unregister-listener -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L284 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase6-domains-mut">domains_mut</code> 只允许 LoggedIn 状态访问可变领域服务，避免未登录或登出后继续写入登录态缓存。
<!-- code-ref: phase6-domains-mut -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L326 -->

## 验证命令

```bash
cargo fmt --all --check
```

```bash
cargo test -p openim-session
```

```bash
cargo check --workspace
```

```bash
cargo test --workspace
```

## Gate 状态

当前已完成：Init、Login、Logout、UnInit 状态转换；登录前置校验；登录态 user id 保存；登录态领域服务重置；登出和反初始化任务清理；监听器注册和注销；注销后回调隔离；任务启停事件；重复 Logout 幂等；Session crate 单元测试和 workspace check。

当前未完成：真实 transport 任务接入、真实 storage 打开和关闭、真实同步任务调度、平台线程切换策略、监听器类型细分、登录接口 HTTP 校验、真实前后台生命周期回归。因此 Phase 6 仍是可验证骨架，后续需要继续接入真实资源后再标记完整 Gate 通过。
