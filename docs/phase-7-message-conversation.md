# Phase 7 Message And Conversation Report

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

Phase 7 已从单纯领域层推进到可离线验证的消息/会话集成边界：文本、图片、文件消息模型，发送状态流转，接收落库入口，消息撤回和已读状态，按会话消息同步，历史分页，消息搜索，会话列表同步，未读计数，草稿、置顶、服务端 proto 生成、SendMsg/PullMsg WebSocket 请求封包与响应解析、native 等待回包入口、SQLite 消息/会话 Repository、IndexedDB 消息/会话异步适配、Session 消息/会话监听事件、新会话事件、总未读变更事件、上传结果到图片/文件消息内容的领域桥接，以及本地 fake transport 串联上传结果、SendMsg、PullMsg、推送消息和本地会话更新均已落地并通过本地验证。当前仍不冒充完整端到端 SDK：真实 OpenIM 服务联调、真实 HTTP 上传客户端与真实 SendMsg 发送链路串联、撤回和已读服务端 HTTP API 调用、绑定层事件暴露仍属于后续集成 Gate。

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
<!-- code-ref: phase7-domain-services-session -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L319 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-protobuf-build">build.rs</code> 已通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-generated-mods">generated.rs</code> 从本地 protocol 仓库生成 wrapperspb、sdkws、conversation、msg 四组 protobuf 类型，并用 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-pb-exports">pb_sdkws/pb_msg</code> 对外导出，保留既有手写 GetMaxSeq 最小模型。
<!-- code-ref: phase7-protobuf-build -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-protocol/build.rs#L24 -->
<!-- code-ref: phase7-generated-mods -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-protocol/src/generated.rs#L1 -->
<!-- code-ref: phase7-pb-exports -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-protocol/src/lib.rs#L13 -->

- 长连消息请求已接入真实 msggateway 标识：<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-build-send-msg">build_send_msg_request</code> 封包裸 MsgData，<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-build-pull-range">build_pull_msg_by_range_request</code> 封包 PullMessageBySeqsReq，<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-build-pull-seq-list">build_pull_msg_by_seq_list_request</code> 封包 GetSeqMessageReq，并由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-decode-response-data">decode_response_data</code> 统一校验 errCode 后解析 protobuf data。
<!-- code-ref: phase7-build-send-msg -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport-core/src/lib.rs#L260 -->
<!-- code-ref: phase7-build-pull-range -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport-core/src/lib.rs#L267 -->
<!-- code-ref: phase7-build-pull-seq-list -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport-core/src/lib.rs#L280 -->
<!-- code-ref: phase7-decode-response-data -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport-core/src/lib.rs#L330 -->

- native WebSocket 客户端新增 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-native-wait-response">send_request_wait_response</code>，可以发送任意 GeneralWsReq 并等待同一 msgIncr 的响应；等待期间收到的非目标响应或推送会保存在内部缓冲，后续仍可通过 recv_event 消费。
<!-- code-ref: phase7-native-wait-response -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-transport-native/src/lib.rs#L133 -->

- SQLite 端已补消息/会话真实 Repository：<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-sqlite-conversation-table">local_conversations</code> 在 migrate 中创建，<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-sqlite-save-message">save_message</code> 会为每个会话创建 chat_logs 前缀动态表并 upsert 消息，<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-sqlite-load-history">load_history</code> 提供按 seq/send_time 倒序分页，<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-sqlite-save-conversation">save_conversation</code> 负责 owner 作用域会话 upsert。
<!-- code-ref: phase7-sqlite-conversation-table -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-sqlite/src/lib.rs#L60 -->
<!-- code-ref: phase7-sqlite-save-message -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-sqlite/src/lib.rs#L205 -->
<!-- code-ref: phase7-sqlite-load-history -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-sqlite/src/lib.rs#L292 -->
<!-- code-ref: phase7-sqlite-save-conversation -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-sqlite/src/lib.rs#L330 -->

- IndexedDB 端已补消息/会话异步适配：<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-indexeddb-stores">local_messages/local_message_histories/local_conversations/local_owner_conversations</code> 固定对象仓库命名，<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-indexeddb-save-message">save_message</code> 写入单消息和会话历史，<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-indexeddb-load-history">load_history</code> 按 seq/send_time 倒序分页，<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-indexeddb-save-conversation">save_conversation</code> 维护 owner 作用域会话列表。
<!-- code-ref: phase7-indexeddb-stores -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-indexeddb/src/lib.rs#L11 -->
<!-- code-ref: phase7-indexeddb-save-message -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-indexeddb/src/wasm.rs#L48 -->
<!-- code-ref: phase7-indexeddb-load-history -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-indexeddb/src/wasm.rs#L70 -->
<!-- code-ref: phase7-indexeddb-save-conversation -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-indexeddb/src/wasm.rs#L82 -->

- Session 监听器边界已补消息/会话事件：<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-session-new-messages-event">NewMessages</code>、<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-session-new-conversations-event">NewConversations</code>、<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-session-conversation-event">ConversationChanged</code> 和 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-session-total-unread-event">TotalUnreadCountChanged</code> 进入统一 SessionEvent，<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-session-dispatch-messages">dispatch_new_messages</code>、<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-session-dispatch-new-conversations">dispatch_new_conversations</code>、<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-session-dispatch-conversations">dispatch_conversation_changed</code> 和 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-session-dispatch-total-unread">dispatch_total_unread_count_changed</code> 要求登录态，其中消息和会话列表事件会跳过空批次。
<!-- code-ref: phase7-session-new-messages-event -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L150 -->
<!-- code-ref: phase7-session-new-conversations-event -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L153 -->
<!-- code-ref: phase7-session-conversation-event -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L156 -->
<!-- code-ref: phase7-session-total-unread-event -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L159 -->
<!-- code-ref: phase7-session-dispatch-messages -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L642 -->
<!-- code-ref: phase7-session-dispatch-new-conversations -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L661 -->
<!-- code-ref: phase7-session-dispatch-conversations -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L650 -->
<!-- code-ref: phase7-session-dispatch-total-unread -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L672 -->

- 上传结果到消息内容的领域桥接已补齐：<code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-picture-from-upload">picture_from_upload</code> 和 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-file-from-upload">file_from_upload</code> 将文件摘要与上传 URL 转成图片/文件 MessageContent，并复用现有消息内容校验。
<!-- code-ref: phase7-picture-from-upload -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/message.rs#L42 -->
<!-- code-ref: phase7-file-from-upload -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/message.rs#L63 -->

- Session 内本地消息传输边界通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-session-message-transport">SessionMessageTransport</code> 固定，fake transport 可同时覆盖 SendMsg、PullMsg 和推送消息入口。
<!-- code-ref: phase7-session-message-transport -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L449 -->

- Session 发送编排由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-session-send-message">send_message</code> 承接，会校验发送者属于当前登录用户，复用 MessageService 的发送状态流转，再把会话投影拆分成“新建会话”“已有会话变更”“总未读变更”三类事件。
<!-- code-ref: phase7-session-send-message -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L678 -->

- Session 拉取编排由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-session-pull-messages">pull_messages</code> 承接，会按会话 ID 执行本地消息区间合并、会话投影，并派发 NewMessages 以及按批拆分后的会话事件。
<!-- code-ref: phase7-session-pull-messages -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L693 -->

- 推送处理由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-session-receive-pushes">receive_transport_pushes</code> 承接，先校验消息仍在当前登录用户可见范围内，再落到消息服务、会话服务和监听事件。
<!-- code-ref: phase7-session-receive-pushes -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L718 -->

- 会话事件批处理由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-session-conversation-batch">dispatch_conversation_events</code> 和 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-session-apply-conversations">apply_messages_to_conversations</code> 统一：新建会话只进入 NewConversations，已有会话进入 ConversationChanged，总未读数只有在前后值变化时才派发 TotalUnreadCountChanged。
<!-- code-ref: phase7-session-conversation-batch -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L797 -->
<!-- code-ref: phase7-session-apply-conversations -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L806 -->

- 本地闭环测试 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase7-session-fake-transport-test">session_message_transport_sends_pulls_pushes_and_updates_conversations</code> 覆盖上传 URL 生成文件消息、SendMsg ack、PullMsg、Push、history、latest message、unread count、max seq 和事件派发计数。
<!-- code-ref: phase7-session-fake-transport-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-session/src/lib.rs#L1124 -->

## 验证命令

```bash
cargo fmt --all --check
```

```bash
cargo test -p openim-protocol
```

```bash
cargo test -p openim-transport-core
```

```bash
cargo test -p openim-transport-native
```

```bash
cargo test -p openim-storage-sqlite
```

```bash
cargo test -p openim-storage-indexeddb
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
cargo check -p openim-storage-indexeddb --target wasm32-unknown-unknown
```

```bash
cargo test --workspace
```

## Gate 状态

当前已完成：文本、图片、文件基础消息模型；上传结果生成图片/文件消息内容；消息发送前本地保存；发送成功回包状态推进；发送失败状态落地；消息接收入口；消息撤回；消息已读；会话内消息区间同步；历史分页；消息搜索；会话列表模型；latest message 投影；未读计数；单会话和全量已读；草稿；置顶；会话列表 owner 作用域同步；会话搜索；Session 领域服务聚合；Session 消息/会话监听事件派发入口；Session 新会话事件；Session 总未读变更事件；Session 本地 fake transport 发送、拉取、推送和会话投影闭环；上游 proto 生成 Rust 端消息/会话协议类型；SendMsg、PullMsgByRange、PullMsgBySeqList、会话 max/read seq、会话 last message 的 WebSocket protobuf 封包与响应解析；native WebSocket 指定 msgIncr 等待回包；SQLite 消息动态表和 local_conversations 会话表的领域 Repository 适配；IndexedDB 消息/会话对象仓库与 wasm32 异步适配；领域、协议、传输、SQLite、IndexedDB、Session 单元测试和 workspace 回归。

当前未完成：真实 OpenIM 服务端账号环境下的 SendMsg/PullMsg/推送回调端到端联调、图片和文件真实 HTTP 上传客户端到真实发送链路的完整联调、撤回和已读回执的 HTTP API 执行与服务端校验、绑定层对消息/会话事件的 FFI/wasm 暴露。因此 Phase 7 当前已具备离线可验证的领域、协议、传输、SQLite、IndexedDB 和 Session 事件集成边界，但完整跨平台端到端仍需要后续真实集成 Gate。
