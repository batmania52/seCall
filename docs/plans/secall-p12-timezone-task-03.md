---
type: task
plan: secall-p12-timezone
task_number: 3
title: 보조 렌더링 위치 적용
depends_on: [1]
parallel_group: B
status: draft
updated_at: 2026-04-08
---

# Task 03: 보조 렌더링 위치 적용

## Changed files

| 파일 | 라인 | 변경 유형 | 설명 |
|------|------|----------|------|
| `crates/secall-core/src/vault/index.rs`:7,38 | 수정 | `update_index()` 시그니처에 `tz` 추가, 시간 포맷 변환 |
| `crates/secall-core/src/vault/log.rs`:7,10 | 수정 | `append_log()` 시그니처에 `tz` 추가, 날짜 포맷 변환 |
| `crates/secall-core/src/search/chunker.rs`:15,23 | 수정 | `chunk_session()` 시그니처에 `tz` 추가, 날짜 포맷 변환 |
| `crates/secall-core/src/hooks/mod.rs`:54 | 수정 | `run_post_ingest_hook()` 시그니처에 `tz` 추가, `SECALL_DATE` 환경변수 변환 |
| `crates/secall-core/src/vault/init.rs`:85,111,127,141 | 수정 | `Utc::now()` → `Utc::now().with_timezone(&tz)` (4곳) |
| `crates/secall-core/src/vault/mod.rs`:57-58 | 수정 | `update_index()`, `append_log()` 호출 시 tz 전달 |
| `crates/secall/src/commands/ingest.rs` | 수정 | `run_post_ingest_hook()` 호출 시 tz 전달 |
| `crates/secall/src/commands/embed.rs` | 수정 | `chunk_session()` 호출 시 tz 전달 |

## Change description

### 1단계: index.rs — update_index()

```rust
// 변경 전 (line 7)
pub fn update_index(vault_path: &Path, session: &Session, md_path: &Path) -> Result<()> {
// 변경 후
pub fn update_index(vault_path: &Path, session: &Session, md_path: &Path, tz: chrono_tz::Tz) -> Result<()> {
```

Line 38:
```rust
// 변경 전
let time_str = session.start_time.format("%H:%M").to_string();
// 변경 후
let time_str = session.start_time.with_timezone(&tz).format("%H:%M").to_string();
```

### 2단계: log.rs — append_log()

```rust
// 변경 전 (line 7)
pub fn append_log(vault_path: &Path, session: &Session, md_path: &Path) -> Result<()> {
// 변경 후
pub fn append_log(vault_path: &Path, session: &Session, md_path: &Path, tz: chrono_tz::Tz) -> Result<()> {
```

Line 10:
```rust
// 변경 전
let date = session.start_time.format("%Y-%m-%d").to_string();
// 변경 후
let date = session.start_time.with_timezone(&tz).format("%Y-%m-%d").to_string();
```

### 3단계: chunker.rs — chunk_session()

```rust
// 변경 전 (line 15)
pub fn chunk_session(session: &Session) -> Vec<Chunk> {
// 변경 후
pub fn chunk_session(session: &Session, tz: chrono_tz::Tz) -> Vec<Chunk> {
```

Line 23:
```rust
// 변경 전
session.start_time.format("%Y-%m-%d"),
// 변경 후
session.start_time.with_timezone(&tz).format("%Y-%m-%d"),
```

### 4단계: hooks/mod.rs — run_post_ingest_hook()

시그니처에 `tz: chrono_tz::Tz` 추가.

Line 54:
```rust
// 변경 전
.env("SECALL_DATE", session.start_time.format("%Y-%m-%d").to_string())
// 변경 후
.env("SECALL_DATE", session.start_time.with_timezone(&tz).format("%Y-%m-%d").to_string())
```

### 5단계: init.rs — 초기화 템플릿

4곳의 `chrono::Utc::now().format("%Y-%m-%d")` → `chrono::Utc::now().with_timezone(&tz).format("%Y-%m-%d")`.

`Vault::init()` 또는 init 함수에 tz 파라미터를 추가하거나, init은 최초 1회성이므로 **UTC 유지도 가능** (설계 선택).

> **추천**: init 템플릿은 UTC 유지. 문서 생성일이므로 타임존 변환의 실익이 없고, init 시점에 config가 아직 없을 수 있음. 이 경우 init.rs는 수정 불필요.

### 6단계: 호출 경로 업데이트

**vault/mod.rs** `write_session()` 내부 (Task 02에서 tz를 받으므로):
```rust
index::update_index(&self.path, session, &rel_path, tz)?;
log::append_log(&self.path, session, &rel_path, tz)?;
```

**ingest.rs** — `run_post_ingest_hook()` 호출:
```rust
let tz = config.timezone();
run_post_ingest_hook(&config.hooks, &session, &vault_path_str, tz)?;
```

**embed.rs** — `chunk_session()` 호출:
```rust
let tz = config.timezone();
let chunks = chunk_session(&session, tz);
```

## Dependencies

- **Task 01**: `chrono-tz` 의존성 + `Config::timezone()` 메서드
- Task 02와 동시 진행 가능 (parallel_group B)

## Verification

```bash
# 1. 컴파일 확인
cargo check --all

# 2. chunker 관련 테스트
cargo test -p secall-core -- search::chunker

# 3. vault 관련 테스트
cargo test -p secall-core -- vault

# 4. 전체 리그레션
cargo test --all
```

## Risks

| 리스크 | 영향 | 대응 |
|--------|------|------|
| 호출 경로 누락 | 시그니처 변경 후 컴파일 에러 | 컴파일러가 잡아줌 — `cargo check` 필수 |
| init.rs 수정 여부 | init 시점에 config 미존재 가능 | init 템플릿은 UTC 유지 (수정 불필요) 권장 |
| hook 환경변수 날짜 변경 | 기존 hook 스크립트가 UTC 날짜를 가정할 수 있음 | 기본값 UTC이므로 미설정 시 기존 동작 유지 |

## Scope boundary

수정 금지 파일:
- `Cargo.toml` — Task 01 영역
- `crates/secall-core/src/vault/config.rs` — Task 01 영역
- `crates/secall-core/src/ingest/markdown.rs` — Task 02 영역
- `README.md`, `CHANGELOG.md` — Task 04 영역
