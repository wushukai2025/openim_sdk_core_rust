# Phase 7 Message And Conversation Report

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

Phase 7 已完成消息与会话域的可验证领域层：文本、图片、文件消息模型，发送状态流转，接收落库入口，消息撤回和已读状态，按会话消息同步，历史分页，消息搜索，会话列表同步，未读计数，草稿和置顶均已落地并通过单元测试。当前实现仍保持领域层边界，不冒充真实 HTTP/WebSocket 发送、服务端拉取、SQLite/IndexedDB 真实表适配或监听器回调派发已经完成；这些真实资源接入继续归入后续集成 Gate。

## Rust 落地点

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-domain-message-module">message</code> 和 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-domain-conversation-module">conversation</code> 已加入领域 crate，继续沿用 Phase 5 的单 crate 聚合方式，避免过早拆分消息和会话 crate。
<!-- code-ref: phase7-domain-message-module -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/lib.rs#L4 -->
<!-- code-ref: phase7-domain-conversation-module -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/lib.rs#L1 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-message-content">MessageContent</code> 覆盖文本、图片和文件三类基础消息，并提供 content type 映射、摘要和参数校验。
<!-- code-ref: phase7-message-content -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/message.rs#L35 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-chat-message">ChatMessage</code> 保留 client/server msg id、conversation id、send/recv/group 路由、session type、content type、已读、状态、seq、时间和撤回标记，作为本地消息主模型。
<!-- code-ref: phase7-chat-message -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/message.rs#L86 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-message-sender">MessageSender</code> 固定真实发送边界，领域层只处理发送前保存、回包确认和失败状态落地，不直接依赖具体 HTTP/WebSocket 实现。
<!-- code-ref: phase7-message-sender -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/message.rs#L205 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-send-message">send_message</code> 会先把消息置为 Sending 并保存，再根据 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-send-ack">SendMessageAck</code> 推进为 SendSuccess，发送失败时落为 SendFailed。
<!-- code-ref: phase7-send-message -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/message.rs#L239 -->
<!-- code-ref: phase7-send-ack -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/message.rs#L198 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-receive-message">receive_message</code> 是服务端推送或同步消息进入本地消息表的统一入口，保持消息路由和 content type 校验。
<!-- code-ref: phase7-receive-message -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/message.rs#L258 -->

- 消息状态操作已覆盖 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-mark-read">mark_read</code> 和 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-revoke-message">revoke_message</code>，撤回时同时标记 revoked 并进入 HasDeleted 状态。
<!-- code-ref: phase7-mark-read -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/message.rs#L292 -->
<!-- code-ref: phase7-revoke-message -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/message.rs#L298 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-sync-message-range">sync_message_range</code> 以会话为作用域合并服务端消息区间，插入和更新服务端消息，但不会删除未出现在当前区间里的历史消息。
<!-- code-ref: phase7-sync-message-range -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/message.rs#L372 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-history">history</code> 和 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-message-search">search</code> 提供消息历史分页和消息内容搜索。
<!-- code-ref: phase7-history -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/message.rs#L334 -->
<!-- code-ref: phase7-message-search -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/message.rs#L344 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-conversation-id">conversation_id_by_session_type</code> 复用 Go SDK 的 si、g、sg、sn 会话 ID 前缀规则，并用测试固定单聊排序和群聊前缀。
<!-- code-ref: phase7-conversation-id -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/message.rs#L454 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-conversation-info">ConversationInfo</code> 覆盖 owner、conversation id、单聊用户、群 id、展示名、未读、latest message、草稿、置顶、max/min seq 和扩展字段。
<!-- code-ref: phase7-conversation-info -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/conversation.rs#L13 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-apply-message">apply_message</code> 将消息投影到会话列表：创建缺失会话、更新 latest message、推进 max seq，并仅对非本人未读消息增加 unread count。
<!-- code-ref: phase7-apply-message -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/conversation.rs#L125 -->

- 会话已读能力通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-mark-conversation-read">mark_conversation_read</code> 和 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-mark-all-read">mark_all_read</code> 落地，并可通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-total-unread">total_unread_count</code> 汇总 owner 维度未读数。
<!-- code-ref: phase7-mark-conversation-read -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/conversation.rs#L157 -->
<!-- code-ref: phase7-mark-all-read -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/conversation.rs#L168 -->
<!-- code-ref: phase7-total-unread -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/conversation.rs#L204 -->

- 草稿和置顶分别由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-set-draft">set_draft</code> 与 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-set-pinned">set_pinned</code> 管理，会话列表排序会优先展示置顶会话，再按 latest message 时间倒序。
<!-- code-ref: phase7-set-draft -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/conversation.rs#L180 -->
<!-- code-ref: phase7-set-pinned -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/conversation.rs#L193 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-sync-conversations">sync_conversations</code> 按 owner 作用域同步会话列表，避免不同登录用户会话串写。
<!-- code-ref: phase7-sync-conversations -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/conversation.rs#L231 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-domain-services-session">DomainServices</code> 已挂载 messages 和 conversations，使登录态 Session 可以统一访问 Phase 7 领域服务。
<!-- code-ref: phase7-domain-services-session -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L129 -->

## 验证命令

```bash
cargo fmt --all --check
```

```bash
cargo test -p openim-domain
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

当前已完成：文本、图片、文件基础消息模型；消息发送前本地保存；发送成功回包状态推进；发送失败状态落地；消息接收入口；消息撤回；消息已读；会话内消息区间同步；历史分页；消息搜索；会话列表模型；latest message 投影；未读计数；单会话和全量已读；草稿；置顶；会话列表 owner 作用域同步；会话搜索；Session 领域服务聚合；领域层单元测试和全 workspace 回归。

当前未完成：真实 SendMsg 请求封包和 WebSocket/HTTP 执行、服务端 PullMsgByRange/PullMsgBySeqList 拉取器、消息 SQLite/IndexedDB 表适配、消息监听器回调派发、图片和文件真实上传结果与消息发送的端到端串联、撤回和已读回执的真实服务端 API 校验。因此 Phase 7 当前完成的是可验证领域层，后续仍需要真实集成 Gate。
