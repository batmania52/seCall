---
type: task
plan: secall-p10-frontmatter
task_number: 2
title: 기존 세션 summary backfill
status: draft
depends_on: [1]
parallel_group: null
updated_at: 2026-04-07
---

# Task 02 — 기존 세션 summary backfill

## Changed files

| 파일 | 변경 유형 |
|------|----------|
| `crates/secall/src/main.rs:21+` | 수정 — `Migrate` 서브커맨드 + `MigrateAction::Summary` 추가 |
| `crates/secall/src/commands/migrate.rs` | **신규** — backfill 로직 구현 |
| `crates/secall/src/commands/mod.rs` | 수정 — `pub mod migrate;` 추가 |
| `crates/secall-core/src/store/db.rs` | 수정 — `update_session_summary()` 메서드 추가 |
| `crates/secall-core/src/ingest/markdown.rs` | 수정 — `extract_summary_from_body()` pub 함수 추가 |

## Change description

### 1. CLI 정의 (`main.rs`)

`Commands` enum에 추가:

```rust
/// Run data migrations
Migrate {
    #[command(subcommand)]
    action: MigrateAction,
},
```

```rust
#[derive(Subcommand)]
enum MigrateAction {
    /// Backfill summary field for existing sessions
    Summary {
        /// Dry run — show what would be changed without writing
        #[arg(long)]
        dry_run: bool,
    },
}
```

match 분기에 `Commands::Migrate { action } => match action { MigrateAction::Summary { dry_run } => commands::migrate::run_summary(dry_run)? }` 추가.

### 2. summary 추출 — MD 본문에서 (`markdown.rs`)

기존 `extract_summary(session: &Session)` 는 `Session` 구조체 기반 (Task 01).
backfill은 vault MD 파일 본문에서 직접 추출해야 하므로 별도 pub 함수 추가:

```rust
/// vault MD 본문에서 첫 User 턴의 실질적 첫 줄을 summary로 추출.
pub fn extract_summary_from_body(content: &str) -> Option<String>
```

로직:
1. frontmatter 이후 본문에서 `## Turn N — User` 패턴의 첫 번째 매치 찾기
2. 해당 턴의 내용에서 비어있지 않은 첫 줄 추출
3. `truncate_str()` 80자 적용
4. 빈 내용이면 `None`

### 3. backfill 로직 (`commands/migrate.rs`)

```rust
pub fn run_summary(dry_run: bool) -> Result<()>
```

1. `Config::load_or_default()` → vault path
2. `walkdir` 로 `raw/sessions/**/*.md` 순회
3. 각 파일:
   a. `parse_session_frontmatter()` 로 frontmatter 파싱
   b. `fm.summary.is_some()` 이면 스킵 (이미 있음)
   c. `extract_summary_from_body(&content)` 로 summary 추출
   d. summary가 None이면 스킵
   e. **MD 파일 수정**: frontmatter 영역에 `summary: "..."` 줄 삽입
      - `status: raw` 라인 직전에 삽입 (Task 01과 동일 위치)
      - 전체 파일을 읽고 → frontmatter 부분만 수정 → 다시 쓰기
   f. **DB 업데이트**: `db.update_session_summary(&fm.session_id, &summary)`
4. dry_run이면 파일/DB 쓰기 없이 변경 예정 목록만 출력
5. 최종 통계: `N updated, M skipped (already has summary), K skipped (no user turn)`

### 4. DB 업데이트 메서드 (`db.rs`)

```rust
pub fn update_session_summary(&self, session_id: &str, summary: &str) -> Result<()> {
    self.conn().execute(
        "UPDATE sessions SET summary = ?1 WHERE id = ?2",
        rusqlite::params![summary, session_id],
    )?;
    Ok(())
}
```

### 5. MD 파일 frontmatter 삽입 전략

기존 본문을 절대 건드리지 않기 위해:
1. 파일 전체를 `read_to_string()`
2. `---\n` 로 시작 확인
3. 두 번째 `\n---\n` 위치 찾기 → frontmatter 영역 분리
4. frontmatter 문자열에서 `\nstatus:` 직전에 `summary: "..."\n` 삽입
5. frontmatter + `\n---\n` + 본문 재조합
6. 파일 덮어쓰기

`status:` 라인이 없는 경우: frontmatter 끝(`---` 직전)에 삽입.

## Dependencies

- **Task 01 완료 필수**: `extract_summary_from_body()` 는 Task 01에서 추가한 `truncate_str()`, `escape_yaml_string()` 재사용. DB에 `summary` 컬럼이 존재해야 함.

## Verification

```bash
# 타입 체크
cargo check --all

# 전체 테스트
cargo test --all

# clippy
cargo clippy --all-targets -- -D warnings

# CLI 도움말 확인
cargo run -- migrate --help
cargo run -- migrate summary --help

# dry-run 테스트 (실제 vault에서)
cargo run -- migrate summary --dry-run
```

## Risks

- **MD 파일 덮어쓰기**: frontmatter 삽입 시 파일 전체를 읽고 다시 쓰므로, 중간에 실패하면 파일 손상 가능. 대응: 임시 파일에 쓰고 rename으로 atomic 교체.
- **frontmatter 포맷 가정**: `status:` 라인이 항상 존재한다고 가정. 없는 경우의 fallback 필요.
- **대량 파일 처리**: vault에 수백 개 세션이 있을 경우 I/O 부하. 진행 상황을 stderr로 출력.
- **git 상태 변경**: backfill 후 vault의 모든 수정된 MD가 git diff에 나타남. `secall sync` 전에 실행하거나, sync가 자동 commit하므로 문제 없음.

## Scope boundary

수정 금지 파일:
- `crates/secall-core/src/store/schema.rs` — Task 01에서 이미 수정됨
- `crates/secall-core/src/store/session_repo.rs` — trait 변경 불필요 (pub fn으로 직접 추가)
- `crates/secall/src/commands/ingest.rs` — ingest 로직은 Task 01에서 처리됨
- `crates/secall/src/commands/reindex.rs` — reindex는 Task 01에서 처리됨
- `docs/prompts/` — wiki 프롬프트 범위 밖
- `README.md` — 기능 완료 후 별도 업데이트
