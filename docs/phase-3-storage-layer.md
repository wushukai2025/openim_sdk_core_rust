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

Phase 3 当前已补齐 storage-core、storage-sqlite、storage-indexeddb、SQLite fixture 工具和迁移样例。native 侧测试、SQLite 兼容测试、wasm32 编译检查和浏览器 IndexedDB CRUD 自动化执行均已通过。

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

- workspace 已继续加入 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-workspace-indexeddb-member">openim-storage-indexeddb</code> 和 db-fixture 工具，用于覆盖 Web 存储实现与迁移样例。
<!-- code-ref: phase3-workspace-indexeddb-member -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/Cargo.toml#L6 -->

- storage-core 定义 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-version-record">VersionRecord</code>，字段 JSON 命名对齐 Go 的 tableName、entityID、versionID、createTime 和 uidList。
<!-- code-ref: phase3-version-record -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-core/src/lib.rs#L25 -->

- storage-core 定义 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-storage-traits">StorageMigrator</code>、AppVersionStore 和 VersionStore，后续 SQLite 与 IndexedDB 都应实现同一组核心契约。
<!-- code-ref: phase3-storage-traits -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-core/src/lib.rs#L53 -->

- storage-core 额外定义 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-async-storage-traits">AsyncStorageMigrator</code>、AsyncAppVersionStore 和 AsyncVersionStore，用于承接浏览器 IndexedDB 的异步语义。
<!-- code-ref: phase3-async-storage-traits -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-core/src/lib.rs#L73 -->

- storage-core 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-openim-db-file">openim_db_file</code> 保持 Go 的数据库命名规则，并在相对路径输入下返回基于当前工作目录的绝对路径。
<!-- code-ref: phase3-openim-db-file -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-core/src/lib.rs#L68 -->

- storage-sqlite 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-sqlite-migrate">migrate</code> 创建 local_app_sdk_version 与 local_sync_version 两张兼容表。
<!-- code-ref: phase3-sqlite-migrate -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-sqlite/src/lib.rs#L33 -->

- storage-sqlite 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-set-version-sync">set_version_sync</code> 使用复合键 upsert，保持 table_name 与 entity_id 的参数透传闭环。
<!-- code-ref: phase3-set-version-sync -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-sqlite/src/lib.rs#L127 -->

- storage-sqlite 的 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-decode-uid-list">decode_uid_list</code> 同时接受 SQLite text 和 blob 两种形态，覆盖 Go StringArray 的 JSON 存储读兼容。
<!-- code-ref: phase3-decode-uid-list -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-sqlite/src/lib.rs#L199 -->

- storage-indexeddb 提供 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-indexeddb-storage">IndexedDbStorage</code>，按登录用户生成 OpenIM_v3 用户级数据库名。
<!-- code-ref: phase3-indexeddb-storage -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-indexeddb/src/lib.rs#L15 -->

- wasm 实现通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-indexeddb-migrate">migrate</code> 打开 IndexedDB 并创建 local_app_sdk_version 与 local_sync_version 对象仓。
<!-- code-ref: phase3-indexeddb-migrate -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-indexeddb/src/wasm.rs#L15 -->

- wasm 浏览器测试 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-indexeddb-crud-test">indexeddb_crud_round_trips_storage_records</code> 覆盖 AppSDKVersion 与 VersionSync 的写入、读取和删除。
<!-- code-ref: phase3-indexeddb-crud-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-indexeddb/src/wasm.rs#L195 -->

- SQLite 兼容测试 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-go-compat-test">reads_go_compatible_existing_version_database</code> 使用 Go 兼容 schema 预置数据，再由 Rust 存储层读取验证。
<!-- code-ref: phase3-go-compat-test -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/crates/openim-storage-sqlite/tests/go_compat.rs#L7 -->

- db-fixture 工具通过 <code style="background:#FFF4E5;color:#C2410C;padding:0 0.2em;border-radius:4px;" data-code-ref="phase3-db-fixture-commands">Command</code> 提供 write-sqlite、verify-sqlite 和 migrate-sqlite 三个入口。
<!-- code-ref: phase3-db-fixture-commands -> file:///Volumes/ssd/Users/hj/Documents/code/github/openim/openim-sdk-core-rust/tools/db-fixture/src/main.rs#L22 -->

## 验证命令

```bash
cargo fmt --all --check
```

```bash
cargo test --workspace
```

```bash
cargo check -p openim-storage-indexeddb --target wasm32-unknown-unknown
```

```bash
cargo test -p openim-storage-indexeddb --target wasm32-unknown-unknown
```

当前本机没有 chromedriver，Safari WebDriver 需要系统授权，因此浏览器自动化执行使用以下替代路径完成：

```bash
NO_HEADLESS=1 cargo test -p openim-storage-indexeddb --target wasm32-unknown-unknown
```

随后使用 Playwright 缓存中的 headless Chromium 打开测试服务器，并通过 DevTools Protocol 读取页面测试结果：

```text
test result: ok. 2 passed; 0 failed; 0 ignored; 0 filtered out
```

## Gate 状态

当前已完成：SQLite schema 创建、AppSDKVersion 单行读写更新、VersionSync 复合键读写删除、Go StringArray JSON blob/text 读取、DB 文件命名规则单测、Go 兼容 SQLite 测试库读取、迁移样例工具、IndexedDB wasm32 编译检查、浏览器 IndexedDB CRUD 自动化执行。

Phase 3 存储层 Gate 已通过。后续进入 Phase 4 前仍建议在 CI 中补标准 WebDriver runner，避免依赖本机 headless Chromium 路径。
