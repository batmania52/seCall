---
type: task
plan: secall-p12-timezone
task_number: 2
title: 마크다운 렌더링 타임존 적용
depends_on: [1]
parallel_group: B
status: draft
updated_at: 2026-04-08
---

# Task 02: 마크다운 렌더링 타임존 적용

## Changed files

| 파일 | 라인 | 변경 유형 | 설명 |
|------|------|----------|------|
| `crates/secall-core/src/ingest/markdown.rs`:57-58 | 수정 | `render_session()` 시그니처에 `tz: chrono_tz::Tz` 파라미터 추가 |
| `crates/secall-core/src/ingest/markdown.rs`:77 | 수정 | frontmatter `date` — `start_time.with_timezone(&tz).format(...)` |
| `crates/secall-core/src/ingest/markdown.rs`:81 | 수정 | frontmatter `start_time` — UTC→TZ 변환 + 동적 오프셋 |
| `crates/secall-core/src/ingest/markdown.rs`:86 | 수정 | frontmatter `end_time` — UTC→TZ 변환 + 동적 오프셋 |
| `crates/secall-core/src/ingest/markdown.rs`:125 | 수정 | 세션 시간 요약 — `start_time.with_timezone(&tz).format("%H:%M")` |
| `crates/secall-core/src/ingest/markdown.rs`:153 | 수정 | 턴 타임스탬프 — `t.with_timezone(&tz).format("%H:%M")` |
| `crates/secall-core/src/ingest/markdown.rs`:228-230 | 수정 | `session_vault_path()` — 경로 날짜를 TZ 기준으로 변환 |
| `crates/secall-core/src/vault/mod.rs`:38-42 | 수정 | `write_session()` — `render_session()` 호출 시 tz 전달 |
| `crates/secall/src/commands/ingest.rs`:336-337 | 수정 | `vault.write_session()` 호출 — Config에서 tz 추출하여 전달 |

## Change description

### 핵심 원칙

- `DateTime<Utc>` → `DateTime<Tz>` 변환은 **렌더링 직전에만** 수행
- DB 저장, 내부 로직은 모두 UTC 유지
- `with_timezone(&tz)` 한 번 호출로 변환

### 1단계: render_session() 시그니처 변경

```rust
// 변경 전
pub fn render_session(session: &Session) -> String {
// 변경 후
pub fn render_session(session: &Session, tz: chrono_tz::Tz) -> String {
```

### 2단계: frontmatter 타임스탬프 변환 (6곳)

**date 필드** (line 77):
```rust
// 변경 전
session.start_time.format("%Y-%m-%d")
// 변경 후
session.start_time.with_timezone(&tz).format("%Y-%m-%d")
```

**start_time 필드** (line 81):
```rust
// 변경 전
session.start_time.format("%Y-%m-%dT%H:%M:%S+00:00")
// 변경 후
session.start_time.with_timezone(&tz).format("%Y-%m-%dT%H:%M:%S%:z")
```

> `%:z`는 `+09:00` 형식의 동적 오프셋을 생성. UTC면 `+00:00`, KST면 `+09:00`.

**end_time 필드** (line 86): 동일 패턴 적용.

**세션 시간 요약** (line 125):
```rust
session.start_time.with_timezone(&tz).format("%H:%M")
```

**턴 타임스탬프** (line 153):
```rust
t.with_timezone(&tz).format("%H:%M")
```

### 3단계: vault 경로 날짜 변환

`session_vault_path()` (line 228-230):
```rust
// 변경 전
pub fn session_vault_path(session: &Session) -> PathBuf {
    let date_str = session.start_time.format("%Y-%m-%d").to_string();
// 변경 후
pub fn session_vault_path(session: &Session, tz: chrono_tz::Tz) -> PathBuf {
    let date_str = session.start_time.with_timezone(&tz).format("%Y-%m-%d").to_string();
```

### 4단계: 호출 경로 업데이트

**vault/mod.rs** `write_session()`:
```rust
// 변경 전
pub fn write_session(&self, session: &Session) -> Result<PathBuf> {
    let md_content = render_session(session);
    let rel_path = session_vault_path(session);
// 변경 후
pub fn write_session(&self, session: &Session, tz: chrono_tz::Tz) -> Result<PathBuf> {
    let md_content = render_session(session, tz);
    let rel_path = session_vault_path(session, tz);
```

**ingest.rs** `ingest_single_session()` 내부:
```rust
let tz = config.timezone();
let rel_path = match vault.write_session(&session, tz) { ... };
```

## Dependencies

- **Task 01**: `chrono-tz` 의존성 + `Config::timezone()` 메서드

## Verification

```bash
# 1. 컴파일 확인
cargo check --all

# 2. 기존 테스트 통과 (UTC 기본값이므로 기존 assert 유지)
cargo test -p secall-core -- ingest::markdown

# 3. 전체 테스트 리그레션 확인
cargo test --all
```

## Risks

| 리스크 | 영향 | 대응 |
|--------|------|------|
| vault 경로 날짜 변경 | UTC 기준 `2026-04-07`이던 세션이 KST로 `2026-04-08`이 될 수 있음 → 기존 파일과 경로 불일치 | 기존 파일은 `secall ingest --force`로 재생성. Non-goals에 명시 |
| frontmatter 오프셋 변경 | 기존 `+00:00` → `+09:00`로 바뀌면 Obsidian Dataview 시간 비교 쿼리에 영향 | Dataview는 ISO 8601 오프셋을 정상 파싱하므로 문제 없음 |
| `render_session()` 호출자 전수 업데이트 필요 | 시그니처 변경으로 컴파일 에러 | 컴파일러가 누락된 호출자를 잡아줌 |

## Scope boundary

수정 금지 파일:
- `Cargo.toml` — Task 01 영역
- `crates/secall-core/src/vault/config.rs` — Task 01 영역
- `crates/secall-core/src/vault/index.rs` — Task 03 영역
- `crates/secall-core/src/vault/log.rs` — Task 03 영역
- `crates/secall-core/src/search/chunker.rs` — Task 03 영역
- `crates/secall-core/src/hooks/mod.rs` — Task 03 영역
- `crates/secall-core/src/vault/init.rs` — Task 03 영역
- `README.md`, `CHANGELOG.md` — Task 04 영역
