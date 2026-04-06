---
type: task
status: draft
plan: secall-extensions-nlp
task_number: 5
title: "secall lint"
parallel_group: C
depends_on: [1, 2, 3, 4]
updated_at: 2026-04-06
---

# Task 05: secall lint

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/ingest/lint.rs` | **신규 생성** | Lint 검증 엔진 |
| `crates/secall-core/src/ingest/mod.rs` | 수정 | `pub mod lint;` 추가 |
| `crates/secall/src/commands/lint.rs` | **신규 생성** | CLI 커맨드 핸들러 |
| `crates/secall/src/commands/mod.rs` | 수정 | `pub mod lint;` 추가 |
| `crates/secall/src/main.rs:20-95` | 수정 | `Lint` 서브커맨드 추가 |

## Change description

### 1. Lint 검증 항목

| # | 검증 | 심각도 | 설명 |
|---|---|---|---|
| L001 | DB 세션 → Vault MD 존재 | error | DB에 있는 세션의 vault_path가 실제 파일로 존재하는지 |
| L002 | Vault MD → DB 세션 존재 | warn | Vault에 MD 파일이 있지만 DB에 레코드가 없는 경우 |
| L003 | 중복 세션 ID | error | 동일 session_id가 2개 이상 존재 |
| L004 | 벡터 임베딩 누락 | info | DB 세션 중 turn_vectors 레코드가 없는 세션 |
| L005 | FTS5 인덱스 무결성 | error | turns_fts와 turns 테이블 레코드 수 불일치 |
| L006 | 에이전트별 세션 통계 | info | claude-code, codex, gemini-cli 별 세션 수 (정보성) |
| L007 | 고아 벡터 | warn | turn_vectors에 있지만 turns에 없는 레코드 |

### 2. Lint 엔진 (lint.rs)

```rust
use crate::store::db::Database;
use crate::vault::config::Config;

#[derive(Debug, Serialize)]
pub struct LintReport {
    pub findings: Vec<LintFinding>,
    pub summary: LintSummary,
}

#[derive(Debug, Serialize)]
pub struct LintFinding {
    pub code: String,       // "L001"
    pub severity: Severity, // error, warn, info
    pub message: String,
    pub session_id: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Serialize)]
pub enum Severity {
    Error,
    Warn,
    Info,
}

#[derive(Debug, Serialize)]
pub struct LintSummary {
    pub total_sessions: usize,
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
    pub agents: HashMap<String, usize>,
}

pub fn run_lint(db: &Database, config: &Config) -> Result<LintReport> {
    let mut findings = Vec::new();

    // L001: DB session → vault file exists
    check_vault_files(db, config, &mut findings)?;

    // L002: vault file → DB session exists
    check_orphan_vault_files(db, config, &mut findings)?;

    // L003: duplicate session IDs
    check_duplicate_sessions(db, &mut findings)?;

    // L004: missing embeddings
    check_missing_embeddings(db, &mut findings)?;

    // L005: FTS5 integrity
    check_fts_integrity(db, &mut findings)?;

    // L006: agent stats (info)
    let agents = collect_agent_stats(db)?;

    // L007: orphan vectors
    check_orphan_vectors(db, &mut findings)?;

    let summary = LintSummary {
        total_sessions: db.count_sessions()?,
        errors: findings.iter().filter(|f| matches!(f.severity, Severity::Error)).count(),
        warnings: findings.iter().filter(|f| matches!(f.severity, Severity::Warn)).count(),
        info: findings.iter().filter(|f| matches!(f.severity, Severity::Info)).count(),
        agents,
    };

    Ok(LintReport { findings, summary })
}
```

### 3. CLI 서브커맨드 (main.rs)

```rust
/// Verify index and vault integrity
Lint {
    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Only show errors (skip warn/info)
    #[arg(long)]
    errors_only: bool,
},
```

텍스트 출력 예시:
```
secall lint report
==================
L001 [ERROR] session abc123: vault file missing at ~/obsidian-vault/seCall/2026/04/abc123.md
L004 [INFO]  session def456: no vector embeddings
L005 [ERROR] FTS5 index has 120 rows but turns table has 125 rows

Summary: 10 sessions, 2 errors, 0 warnings, 1 info
Agents: claude-code(5), codex(3), gemini-cli(2)
```

JSON 출력: `LintReport`를 `serde_json::to_string_pretty()`.

### 4. DB 헬퍼 메서드

`Database`에 lint 전용 쿼리 추가가 필요할 수 있음:
- `count_sessions()` → i64
- `list_session_vault_paths()` → Vec<(String, Option<String>)>
- `count_fts_rows()` → i64
- `count_turns()` → i64
- `find_sessions_without_vectors()` → Vec<String>
- `find_orphan_vectors()` → Vec<(i64, String)>
- `agent_counts()` → HashMap<String, usize>

이 메서드들은 `store/db.rs`에 `impl Database` 블록으로 추가.

## Dependencies

- Task 01, 02, 03, 04 모두 완료 후 (lint가 새 에이전트/임베더를 인식해야 함)
- 단, lint 자체는 읽기 전용이므로 코드 작성은 병렬 가능. 통합 테스트만 의존

## Verification

```bash
# 타입 체크
cargo check

# lint 모듈 단위 테스트
cargo test -p secall-core lint

# CLI 서브커맨드 등록 확인
cargo run -p secall -- lint --help

# 전체 테스트 회귀 없음
cargo test
```

테스트 작성 요구사항:
- `test_lint_empty_db`: 빈 DB → findings 0개, summary 정상
- `test_lint_missing_vault_file`: DB에 세션 있지만 vault 파일 없음 → L001 error
- `test_lint_duplicate_session`: 동일 ID 2개 → L003 error
- `test_lint_fts_mismatch`: turns와 turns_fts 불일치 → L005 error
- `test_lint_report_json`: JSON 직렬화 검증

## Risks

- **DB 헬퍼 추가**: `store/db.rs`에 메서드 추가 시 기존 코드와 충돌 가능성 낮음 (읽기 전용 쿼리)
- **Vault 경로 확인**: Vault 경로가 config에 따라 다를 수 있으므로 `Config::load_or_default()` 사용
- **대량 세션**: 세션 수가 많으면 lint 시간 증가. 각 체크를 SQL 단일 쿼리로 구현하여 O(1) DB 접근

## Scope Boundary

수정 금지 파일:
- `ingest/claude.rs`, `ingest/codex.rs`, `ingest/gemini.rs` — 파서 코드 변경 금지
- `search/*` — 검색 모듈 변경 금지
- `mcp/*` — MCP 서버 변경 금지
