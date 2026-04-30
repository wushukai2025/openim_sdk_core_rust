# Phase 5 Domain Services Report

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

Phase 5 已完成低耦合领域服务重构：用户资料查询与更新、好友与黑名单同步、群组与群成员同步、领域仓储边界、文件摘要、分片、断点续传状态、对象上传请求契约、签名分片 PUT 边界、上传执行抽象和上传进度计算均已落地并通过单元测试。Phase 4 的真实 OpenIM server Gate 仍保持未通过状态；真实服务端收发、真实 HTTP 上传端点端到端执行、SQLite 与 IndexedDB 具体表适配、Session 生命周期装配不在本阶段冒充完成，会继续留到后续真实集成 Gate。

## Rust 落地点

- workspace 已加入 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-workspace-domain-member">openim-domain</code>，领域服务先按模块聚合到一个 crate，避免过早拆分 user、relation、group 和 file crate。
<!-- code-ref: phase5-workspace-domain-member -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/Cargo.toml#L4 -->

- <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-domain-sync-summary">DomainSyncSummary</code> 统一统计领域同步中的插入、更新、删除和未变化数量，复用 Phase 2 的同步动作状态。
<!-- code-ref: phase5-domain-sync-summary -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/lib.rs#L9 -->

- 用户域通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-user-service">UserService</code> 提供用户资料内存态管理，保持查询、批量查询和局部更新的低耦合 API。
<!-- code-ref: phase5-user-service -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/user.rs#L32 -->

- 用户仓储边界通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-user-profile-repository">UserProfileRepository</code> 固定，当前由 UserService 提供内存实现，后续 SQLite 与 IndexedDB 适配可接入同一接口。
<!-- code-ref: phase5-user-profile-repository -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/user.rs#L25 -->

- 用户资料更新由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-user-update-profile">update_profile</code> 承接，空字段不会覆盖旧值，更新目标不存在时返回参数错误。
<!-- code-ref: phase5-user-update-profile -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/user.rs#L47 -->

- 批量用户查询由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-user-get-profiles">get_profiles</code> 承接，按调用方请求顺序返回已存在资料并跳过缺失项。
<!-- code-ref: phase5-user-get-profiles -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/user.rs#L75 -->

- 关系域通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-relation-service">RelationService</code> 管理好友和黑名单缓存，键空间显式包含 owner user，避免不同登录用户的数据串写。
<!-- code-ref: phase5-relation-service -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/relation.rs#L44 -->

- 关系仓储边界通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-friend-repository">FriendRepository</code> 和 BlacklistRepository 固定，覆盖保存、删除和按 owner 读取。
<!-- code-ref: phase5-friend-repository -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/relation.rs#L31 -->

- 好友同步由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-sync-friends">sync_friends</code> 承接，先校验 owner，再按 friend user 做插入、更新、删除和未变化合并。
<!-- code-ref: phase5-sync-friends -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/relation.rs#L91 -->

- 黑名单同步由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-sync-blacklist">sync_blacklist</code> 承接，和好友同步共用 owner 作用域校验与同步摘要语义。
<!-- code-ref: phase5-sync-blacklist -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/relation.rs#L153 -->

- 群组域通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-group-service">GroupService</code> 管理群资料和群成员，成员键空间由 group 和 user 共同确定。
<!-- code-ref: phase5-group-service -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/group.rs#L45 -->

- 群组仓储边界通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-group-repository">GroupRepository</code> 和 GroupMemberRepository 固定，覆盖群资料与群成员的保存、删除和读取。
<!-- code-ref: phase5-group-repository -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/group.rs#L32 -->

- 群组同步由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-sync-groups">sync_groups</code> 承接，覆盖群插入、更新和删除。
<!-- code-ref: phase5-sync-groups -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/group.rs#L80 -->

- 群删除时同步清理成员缓存，避免出现 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-group-member-cleanup">群不存在但成员仍残留</code> 的半残状态。
<!-- code-ref: phase5-group-member-cleanup -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/group.rs#L66 -->

- 群成员同步由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-sync-group-members">sync_group_members</code> 承接，先校验每个服务端成员归属当前 group，再按 user 合并。
<!-- code-ref: phase5-sync-group-members -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/group.rs#L136 -->

- 文件域以 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-file-digest">FileDigest</code> 表达文件摘要，后续真实上传端点可以直接复用 file name、size、content type 和 sha256。
<!-- code-ref: phase5-file-digest -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L15 -->

- 对象上传 HTTP 路径已在领域层固定为 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-object-part-limit-path">OBJECT_PART_LIMIT_PATH</code> 同组常量，覆盖分片限制、初始化 multipart、签名和完成上传四个 OpenIM object API。
<!-- code-ref: phase5-object-part-limit-path -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L7 -->

- 服务端分片限制由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-object-part-limit">ObjectPartLimit</code> 承接，按 Go SDK 的 min part、max part 和 max part count 规则计算 part size。
<!-- code-ref: phase5-object-part-limit -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L78 -->

- 上传对象参数由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-upload-object-request">UploadObjectRequest</code> 承接，会校验 login user 和 name，并按 Go SDK 规则给对象名补 login user 前缀。
<!-- code-ref: phase5-upload-object-request -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L124 -->

- 初始化和完成上传请求分别由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-initiate-multipart-request">InitiateMultipartUploadRequest</code> 与 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-complete-multipart-request">CompleteMultipartUploadRequest</code> 固定，保留 hash、size、part size、max parts、cause、name、content type 和 url prefix 字段。
<!-- code-ref: phase5-initiate-multipart-request -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L162 -->
<!-- code-ref: phase5-complete-multipart-request -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L198 -->

- 对象存储后端通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-object-storage-api">ObjectStorageApi</code> 抽象，覆盖 part limit、initiate、auth sign 和 complete；真实 HTTP 端点后续只需要实现该 trait。
<!-- code-ref: phase5-object-storage-api -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L259 -->

- 签名后的分片 PUT 请求由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-signed-upload-part-request">SignedUploadPartRequest</code> 表达，真正发起网络写入的边界收敛到 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-http-upload-client">HttpUploadClient</code>。
<!-- code-ref: phase5-signed-upload-part-request -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L251 -->
<!-- code-ref: phase5-http-upload-client -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L272 -->

- 分片上传会话由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-multipart-upload-session">MultipartUploadSession</code> 持有 upload id、part size、过期时间和已缓存签名；缺失 part sign 时按批量数量向后续 part number 续签。
<!-- code-ref: phase5-multipart-upload-session -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L285 -->

- 签名上传执行器 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-signed-multipart-upload-client">SignedMultipartUploadClient</code> 复用已有 FileUploadClient 流程，把领域分片转换为签名 PUT 请求并校验回包 part number。
<!-- code-ref: phase5-signed-multipart-upload-client -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L376 -->

- 签名 URL 构建由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-build-signed-put-request">build_signed_put_request</code> 负责，按 Go SDK 行为合并 base URL、sign query、part query、sign header 和 part header，part 级凭据优先。
<!-- code-ref: phase5-build-signed-put-request -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L616 -->

- 文件分片入口 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-plan-multipart">plan_multipart</code> 根据文件大小和 part size 生成稳定分片列表，并拒绝空文件名和零分片大小。
<!-- code-ref: phase5-plan-multipart -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L422 -->

- 断点续传入口 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-resume-plan">resume_plan</code> 根据已上传分片号恢复上传状态。
<!-- code-ref: phase5-resume-plan -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L515 -->

- 上传状态推进由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-mark-uploaded">mark_uploaded</code> 负责，未知分片号会返回参数错误，避免进度被错误推进。
<!-- code-ref: phase5-mark-uploaded -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L526 -->

- 文件上传执行边界通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-file-upload-client">FileUploadClient</code> 固定，真实或 mock 上传客户端只需要实现 upload_part。
<!-- code-ref: phase5-file-upload-client -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L276 -->

- 缺失分片上传由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-upload-missing-parts">upload_missing_parts</code> 执行，会跳过已恢复分片、校验回包 part number，并在每个分片成功后推进本地进度。
<!-- code-ref: phase5-upload-missing-parts -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L540 -->

- 上传进度由 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase5-upload-progress">progress</code> 汇总 uploaded bytes、total bytes、uploaded parts 和 total parts，并通过 UploadProgress 计算百分比和完成态。
<!-- code-ref: phase5-upload-progress -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-domain/src/file.rs#L568 -->

## 验证命令

```bash
cargo fmt --all --check
```

```bash
cargo test -p openim-domain
```

```bash
cargo check --workspace
```

```bash
cargo test --workspace
```

## Gate 状态

当前已完成：用户资料查询与更新、批量用户资料查询、用户仓储边界、好友 owner 作用域同步、黑名单 owner 作用域同步、关系仓储边界、群组同步、群成员 group 作用域同步、群组与群成员仓储边界、群删除时成员清理、文件摘要模型、分片计划、服务端 part limit 规则、对象名 login user 作用域、initiate 和 complete 请求契约、对象存储 API 边界、签名分片请求构建、签名缺失时批量续签、断点续传状态恢复、上传执行抽象、上传缺失分片流程、上传回包 part number 校验、上传分片状态推进、上传进度计算、领域层单元测试和全工作区回归。

当前未纳入 Phase 5 领域层完成声明：真实 OpenIM server 兼容收发、真实上传 HTTP 端点端到端执行、SQLite 与 IndexedDB 具体表结构适配、Session 生命周期装配、监听器回调派发。这些能力需要在 Phase 6 及后续真实集成 Gate 中继续验证。
