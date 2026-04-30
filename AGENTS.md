# AGENTS.md

## Scope

This file applies to the whole Rust workspace under this repository.
Deeper AGENTS.md files may override these rules for their subtrees.

## Mission

This repository is the Rust-native rewrite of the OpenIM SDK core located at:

/Volumes/ssd - Data/Users/hj/Documents/code/github/openim/openim-sdk-core

Use that Go repository as the product behavior, protocol, storage, and API
compatibility reference. Do not treat it as code to mechanically translate.
The goal is a complete Rust implementation with equivalent SDK behavior,
clean Rust crate boundaries, and verifiable cross-platform integration.

## Rewrite Rules

- Implement functionality in Rust. Do not shell out to Go code or depend on the
  Go SDK runtime as the implementation path.
- Preserve OpenIM behavior, request and response semantics, storage rules,
  callback expectations, and error meaning unless a documented Rust design
  decision says otherwise.
- Prefer Rust ownership, typed errors, traits, async boundaries, and platform
  adapters over one-to-one Go structure copying.
- Keep generated protocol bindings reproducible from source schemas or checked
  build inputs. Do not hand-edit generated code.
- Use the Go SDK tests, integration fixtures, docs, and implementation only as
  evidence for behavior parity.
- Never modify the Go source repository unless the user explicitly asks.

## Current Workspace Shape

- Protocol and wire models live in crates/openim-protocol.
- Shared data models live in crates/openim-types.
- Domain services live in crates/openim-domain.
- Session orchestration lives in crates/openim-session.
- Storage traits and adapters live in crates/openim-storage-core,
  crates/openim-storage-sqlite, and crates/openim-storage-indexeddb.
- Transport traits and adapters live in crates/openim-transport-core,
  crates/openim-transport, crates/openim-transport-native, and
  crates/openim-transport-wasm.
- Sync support lives in crates/openim-sync.
- Shared error handling lives in crates/openim-errors.
- Local verification helpers live under tools.
- Phase reports live under docs and are the migration status record.

## Phase Workflow

- Before starting a phase, read the relevant docs phase report and compare it
  with the Go SDK source paths for that feature.
- If a phase report lists unfinished Gate items, finish those before moving to
  a later phase unless the user explicitly changes priority.
- When continuing after an interruption, inspect git status and the latest phase
  report before assuming what remains.
- Each phase should end with an updated report in docs that states completed
  behavior, Rust landing points, verification commands, and remaining Gate
  items.
- Do not mark a Gate complete unless the implementation has a local verification
  command or a clearly documented reason why it cannot be run locally.

## Implementation Standards

- Keep diffs small, reviewable, and tied to the requested migration slice.
- Read the relevant Go behavior and existing Rust code before editing.
- Prefer existing crate patterns and public APIs before adding abstractions.
- Keep platform-specific code behind adapter crates or target-specific modules.
- Preserve owner, user, group, conversation, message, sequence, and login context
  boundaries. Do not rely on global current state where a data owner is needed.
- Avoid deleting behavior, callbacks, fields, request parameters, persistence
  side effects, or event dispatch paths without explicit user confirmation.
- Add tests close to the crate that owns the behavior. Use fixtures when parity
  with Go encoding, protocol, or storage behavior matters.

## Verification Defaults

Run the smallest command that proves the change. Prefer package-scoped checks
while iterating, then broader checks before reporting a completed Gate.

Common verification commands:

- cargo fmt --all --check
- cargo test -p openim-protocol
- cargo test -p openim-domain
- cargo test -p openim-session
- cargo test -p openim-storage-sqlite
- cargo test -p openim-storage-indexeddb
- cargo test -p openim-transport-core
- cargo test -p openim-transport-native
- cargo check --workspace
- cargo check -p openim-storage-indexeddb --target wasm32-unknown-unknown
- cargo test --workspace

Read command output before claiming success. If a command cannot be run, report
the blocker and the residual risk.

## Documentation Rules

- Keep phase documentation current with implementation changes.
- Document behavior parity decisions and remaining gaps concretely.
- Do not claim real OpenIM service compatibility from offline unit tests alone.
- When adding Markdown status reports with code references, follow the repository
  data-code-ref convention already used in docs.

## Git Rules

- Do not run git stash.
- Do not change the user's staging state unless explicitly asked.
- Do not use destructive git commands unless the user explicitly asks.
- Before generating a commit message, inspect status, staged diff, unstaged diff,
  and recent commit style.
