---
type: task
plan: secall-p10-frontmatter
task_number: 1
title: 세션 summary frontmatter 추가
status: draft
depends_on: []
parallel_group: null
updated_at: 2026-04-07
---

# Task 01 — 세션 summary frontmatter 추가

## Changed files

| 파일 | 변경 유형 |
|------|----------|
| `crates/secall-core/src/store/schema.rs:4-22` | 수정 — sessions 테이블에 `summary TEXT` 컬럼 추가 |
| `crates/secall-core/src/store/db.rs:299-342` | 수정 — `run_migrations()`에 v2 마이그레이션 추가 |
| `crates/secall-core/src/store/db.rs:346-388` | 수정 — `insert_session_from_vault()` INSERT 쿼리에 summary 추가 |
| `crates/secall-core/src/search/bm25.rs:191-231` | 수정 — `insert_session()` INSERT 쿼리에 summary 추가 |
| `crates/secall-core/src/ingest/markdown.rs:7-24` | 수정 — `SessionFrontmatter` struct에 `summary: Option<String>` 추가 |
| `crates/secall-core/src/ingest/markdown.rs:59-110` | 수정 — `render_session()`에 summary 생성 + frontmatter 출력 |
| `crates/secall-core/src/ingest/markdown.rs:285+` | 수정 — summary 생성 관련 단위 테스트 추가 |
| `crates/secall-core/src/ingest/types.rs` | 확인만 — `Role::User` enum 사용 (수정 불필요) |

## Change description

### 1. summary 추출 함수 (`markdown.rs`)

`render_session()` 근처에 `extract_summary()` 함수 추가:

```
fn extract_summary(session: &Session) -> Option<String>
```

로직:
1. `session.turns.iter().find(|t| t.role == Role::User)` — 첫 User 턴 찾기
2. `content.lines()` 순회하며 비어있지 않은 첫 줄 추출 (빈 줄, 공백만 있는 줄 스킵)
3. 기존 `truncate_str()` (markdown.rs:259) 사용하여 80자 truncate
4. `None` 반환: User 턴이 없거나 내용이 빈 경우

### 2. frontmatter 출력 (`markdown.rs:render_session()`)

`status: raw` 라인 직전 (line 109 부근)에 summary 출력 추가:

```rust
if let Some(summary) = extract_summary(session) {
    let escaped = escape_yaml_string(&summary);
    out.push_str(&format!("summary: \"{escaped}\"\n"));
}
```

YAML 이스케이프 헬퍼:
```rust
fn escape_yaml_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
```

줄바꿈(`\n`), 콜론(`:`), `#` 등은 따옴표 래핑으로 이미 안전. 내부 `"` 와 `\` 만 이스케이프.

### 3. `SessionFrontmatter` struct (markdown.rs:7-24)

```rust
pub summary: Option<String>,  // 추가
```

`#[serde(default)]`로 이미 보호되어 있어, 기존 MD (summary 없는 파일)도 파싱 시 `None`으로 처리됨.

### 4. DB 스키마 마이그레이션 (schema.rs + db.rs)

**schema.rs**: `CREATE_SESSIONS` 상수에 `summary TEXT` 컬럼 추가 (line 18, `vault_path` 다음).

**db.rs**: `run_migrations()`에 v2 마이그레이션 추가:
```sql
ALTER TABLE sessions ADD COLUMN summary TEXT;
```
`CURRENT_SCHEMA_VERSION`을 `1` → `2`로 변경.

### 5. INSERT 쿼리 수정

**bm25.rs** `insert_session()` (line 208-229): INSERT 컬럼 리스트와 VALUES에 summary 추가. `extract_summary(session)` 호출하여 값 전달.

**db.rs** `insert_session_from_vault()` (line 352-377): INSERT에 `fm.summary` 추가.

### 6. 테스트 추가 (markdown.rs tests)

- `test_summary_from_first_user_turn` — 일반 케이스
- `test_summary_skips_empty_lines` — 첫 줄이 빈 경우 건너뛰기
- `test_summary_truncation` — 80자 초과 시 truncation
- `test_summary_none_when_no_user_turn` — User 턴 없으면 None
- `test_summary_yaml_escape` — 특수문자(`"`, `\`) 이스케이프

## Dependencies

- 없음 (첫 번째 태스크)

## Verification

```bash
# 타입 체크
cargo check --all

# 전체 테스트
cargo test --all

# clippy 경고 없음
cargo clippy --all-targets -- -D warnings

# summary 관련 테스트만 실행
cargo test -p secall-core test_summary
```

## Risks

- **DB 마이그레이션**: 기존 DB에 `ALTER TABLE ADD COLUMN` 실행. SQLite는 이 작업이 안전하지만, 마이그레이션 버전 번호 충돌 주의.
- **기존 MD 파싱**: summary 필드가 없는 기존 MD는 `serde(default)`로 `None` 처리되므로 호환성 문제 없음.
- **YAML 이스케이프 누락**: 사용자 입력에 따옴표, 백슬래시가 포함될 수 있음 → `escape_yaml_string()` 헬퍼로 처리.

## Scope boundary

수정 금지 파일:
- `crates/secall/src/main.rs` — Task 02에서 `migrate` 서브커맨드 추가
- `crates/secall/src/commands/` — Task 02 영역
- `docs/prompts/` — wiki 프롬프트는 이 태스크 범위 밖
- `README.md` — 기능 완료 후 별도 업데이트
