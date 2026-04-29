# Phase 3 Storage Layer Report

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

Phase 3 当前完成第一段：已新增 storage-core 契约和 storage-sqlite 原生实现，覆盖 Go 侧 AppSDKVersion 与 VersionSync 的兼容读写。完整 Phase 3 Gate 尚未通过，因为 IndexedDB、浏览器 CRUD 自动化和历史迁移 fixture 还没有实现。

## Go 兼容依据

- Go 原生数据库文件名来自 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-go-db-path">initDB</code>，格式为 OpenIM_v3 加登录用户 ID，并会转成绝对路径后打开 SQLite 文件。
<!-- code-ref: phase3-go-db-path -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core/pkg/db/db_init.go#L121 -->

- Go 侧 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-go-local-version-sync">LocalVersionSync</code> 使用 table_name 与 entity_id 作为复合主键，id_list 通过 StringArray 以 JSON 数组存储。
<!-- code-ref: phase3-go-local-version-sync -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core/pkg/db/model_struct/data_model_struct.go#L301 -->

- Go 侧 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-go-local-app-sdk-version">LocalAppSDKVersion</code> 使用 local_app_sdk_version 表记录 SDK 数据版本和 installed 标记。
<!-- code-ref: phase3-go-local-app-sdk-version -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core/pkg/db/model_struct/data_model_struct.go#L314 -->

- Go 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-go-set-version-sync">SetVersionSync</code> 行为是先按复合键查找，缺失则创建，存在则更新。
<!-- code-ref: phase3-go-set-version-sync -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core/pkg/db/version_sync.go#L44 -->

## Rust 落地点

- workspace 已加入 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-workspace-storage-members">openim-storage-core</code> 和 openim-storage-sqlite，并新增 bundled rusqlite 依赖。
<!-- code-ref: phase3-workspace-storage-members -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/Cargo.toml#L5 -->

- storage-core 定义 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-version-record">VersionRecord</code>，字段 JSON 命名对齐 Go 的 tableName、entityID、versionID、createTime 和 uidList。
<!-- code-ref: phase3-version-record -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-core/src/lib.rs#L25 -->

- storage-core 定义 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-storage-traits">StorageMigrator</code>、AppVersionStore 和 VersionStore，后续 SQLite 与 IndexedDB 都应实现同一组核心契约。
<!-- code-ref: phase3-storage-traits -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-core/src/lib.rs#L53 -->

- storage-core 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-openim-db-file">openim_db_file</code> 保持 Go 的数据库命名规则，并在相对路径输入下返回基于当前工作目录的绝对路径。
<!-- code-ref: phase3-openim-db-file -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-core/src/lib.rs#L68 -->

- storage-sqlite 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-sqlite-migrate">migrate</code> 创建 local_app_sdk_version 与 local_sync_version 两张兼容表。
<!-- code-ref: phase3-sqlite-migrate -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-sqlite/src/lib.rs#L33 -->

- storage-sqlite 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-set-version-sync">set_version_sync</code> 使用复合键 upsert，保持 table_name 与 entity_id 的参数透传闭环。
<!-- code-ref: phase3-set-version-sync -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-sqlite/src/lib.rs#L127 -->

- storage-sqlite 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-decode-uid-list">decode_uid_list</code> 同时接受 SQLite text 和 blob 两种形态，覆盖 Go StringArray 的 JSON 存储读兼容。
<!-- code-ref: phase3-decode-uid-list -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-sqlite/src/lib.rs#L199 -->

## 验证命令

```bash
cargo fmt --all --check
```

```bash
cargo test --workspace
```

## Gate 状态

当前已完成：SQLite schema 创建、AppSDKVersion 单行读写更新、VersionSync 复合键读写删除、Go StringArray JSON blob/text 读取、DB 文件命名规则单测。

当前未完成：storage-indexeddb、浏览器自动化 CRUD、现有真实测试数据库读写 fixture、历史版本迁移回放工具。Phase 3 完整 Gate 必须等这些补齐后再标记通过。

