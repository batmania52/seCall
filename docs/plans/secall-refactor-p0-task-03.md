---
type: task
status: draft
plan: secall-refactor-p0
task_number: 3
title: "Lint L002 session_id 추출 수정"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 03: Lint L002 session_id 추출 수정

## 문제

L002(`check_orphan_vault_files`)가 vault 파일의 **파일명 stem**을 session_id로 사용하여 DB 조회.

```
파일명: claude-code_seCall_a1b2c3d4.md
stem:   claude-code_seCall_a1b2c3d4    ← 이것을 session_id로 전달
DB id:  a1b2c3d4-e5f6-7890-abcd-...   ← 실제 UUID
```

`db.session_exists("claude-code_seCall_a1b2c3d4")` → 항상 false → **모든 vault 파일이 orphan으로 오탐**.

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/ingest/lint.rs:125-162` | 수정 | `check_orphan_vault_files()` — frontmatter에서 session_id 추출 |
| `crates/secall-core/src/ingest/lint.rs` (하단) | 추가 | `extract_session_id_from_frontmatter()` 헬퍼 함수 |
| `crates/secall-core/src/ingest/lint.rs` (tests) | 추가 | L002 false positive 방지 테스트 |

## Change description

### Step 1: frontmatter에서 session_id 추출하는 헬퍼 추가

vault 마크다운 파일의 frontmatter 형식 (`markdown.rs:render_session()`에서 생성):

```yaml
---
type: session
session_id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
agent: claude-code
project: seCall
...
---
```

헬퍼 함수:

```rust
/// vault 마크다운 파일에서 session_id를 추출
fn extract_session_id_from_file(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    extract_session_id_from_frontmatter(&content)
}

fn extract_session_id_from_frontmatter(content: &str) -> Option<String> {
    if !content.starts_with("---\n") {
        return None;
    }
    let fm_end = content[4..].find("\n---")?;
    let frontmatter = &content[4..4 + fm_end];

    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("session_id:") {
            let value = trimmed["session_id:".len()..].trim();
            // Remove surrounding quotes
            let id = value.trim_matches('"').trim_matches('\'');
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
}
```

### Step 2: check_orphan_vault_files() 수정

```rust
// lint.rs:125-162 — 변경 전
fn check_orphan_vault_files(
    db: &Database,
    config: &Config,
    findings: &mut Vec<LintFinding>,
) -> Result<()> {
    let sessions_dir = config.vault.path.join("raw").join("sessions");
    if !sessions_dir.exists() {
        return Ok(());
    }

    for entry in walkdir::WalkDir::new(&sessions_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if p.extension().map(|e| e == "md").unwrap_or(false) {
            let session_id = p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            if !session_id.is_empty() && !db.session_exists(&session_id).unwrap_or(true) {
                findings.push(LintFinding { code: "L002".to_string(), ... });
            }
        }
    }
    Ok(())
}

// 변경 후
fn check_orphan_vault_files(
    db: &Database,
    config: &Config,
    findings: &mut Vec<LintFinding>,
) -> Result<()> {
    let sessions_dir = config.vault.path.join("raw").join("sessions");
    if !sessions_dir.exists() {
        return Ok(());
    }

    for entry in walkdir::WalkDir::new(&sessions_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if !p.extension().map(|e| e == "md").unwrap_or(false) {
            continue;
        }

        // 1차: frontmatter에서 session_id 추출
        let session_id = match extract_session_id_from_file(p) {
            Some(id) => id,
            None => {
                // frontmatter 없으면 파일명에서 id_prefix 추출 (fallback)
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                // 파일명 형식: {agent}_{project}_{id_prefix}
                // 마지막 '_' 이후가 id_prefix
                match stem.rfind('_') {
                    Some(pos) => stem[pos + 1..].to_string(),
                    None => stem.to_string(),
                }
            }
        };

        if session_id.is_empty() {
            continue;
        }

        // frontmatter에서 추출한 full UUID → 정확한 EXISTS 쿼리
        // fallback id_prefix → LIKE 쿼리로 prefix 매칭
        let exists = if session_id.len() > 8 {
            // Full session_id (from frontmatter)
            db.session_exists(&session_id).unwrap_or(true)
        } else {
            // Short prefix (from filename fallback)
            db.session_exists_by_prefix(&session_id).unwrap_or(true)
        };

        if !exists {
            findings.push(LintFinding {
                code: "L002".to_string(),
                severity: Severity::Warn,
                message: format!("vault file exists but no DB record: {}", p.display()),
                session_id: Some(session_id),
                path: Some(p.to_string_lossy().to_string()),
            });
        }
    }
    Ok(())
}
```

### Step 3: session_exists_by_prefix() 추가

```rust
// db.rs (bm25.rs의 Database impl 블록 또는 store/db.rs)
impl Database {
    pub fn session_exists_by_prefix(&self, prefix: &str) -> Result<bool> {
        let pattern = format!("{}%", prefix);
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM sessions WHERE id LIKE ?1",
            [pattern],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}
```

### Step 4: 테스트 추가

```rust
#[test]
fn test_lint_l002_no_false_positive() {
    let db = Database::open_memory().unwrap();
    let (config, _tmp) = make_config_tmp();

    // DB에 세션 삽입
    db.conn().execute_batch(
        "INSERT INTO sessions(id, agent, start_time, ingested_at) \
         VALUES('a1b2c3d4-e5f6-7890-abcd-ef1234567890','claude-code','2026-01-01','2026-01-01')"
    ).unwrap();

    // vault 파일 생성 (파일명은 agent_project_prefix 형식)
    let sessions_dir = config.vault.path.join("raw").join("sessions").join("2026-01-01");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    std::fs::write(
        sessions_dir.join("claude-code_seCall_a1b2c3d4.md"),
        "---\ntype: session\nsession_id: \"a1b2c3d4-e5f6-7890-abcd-ef1234567890\"\nagent: claude-code\n---\n# Session\n"
    ).unwrap();

    let report = run_lint(&db, &config).unwrap();
    let l002 = report.findings.iter().filter(|f| f.code == "L002").collect::<Vec<_>>();
    assert!(l002.is_empty(), "L002 should not report false positive for existing session");
}

#[test]
fn test_lint_l002_detects_real_orphan() {
    let db = Database::open_memory().unwrap();
    let (config, _tmp) = make_config_tmp();

    // DB에 세션 없음 — vault 파일만 존재
    let sessions_dir = config.vault.path.join("raw").join("sessions").join("2026-01-01");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    std::fs::write(
        sessions_dir.join("claude-code_unknown_deadbeef.md"),
        "---\ntype: session\nsession_id: \"deadbeef-0000-0000-0000-000000000000\"\n---\n"
    ).unwrap();

    let report = run_lint(&db, &config).unwrap();
    let l002 = report.findings.iter().filter(|f| f.code == "L002").collect::<Vec<_>>();
    assert_eq!(l002.len(), 1, "L002 should detect real orphan vault file");
}

#[test]
fn test_extract_session_id_from_frontmatter() {
    let content = "---\ntype: session\nsession_id: \"abc-123\"\nagent: claude-code\n---\n# Session\n";
    assert_eq!(
        extract_session_id_from_frontmatter(content),
        Some("abc-123".to_string())
    );

    // No frontmatter
    assert_eq!(extract_session_id_from_frontmatter("# No frontmatter"), None);

    // Missing session_id field
    let no_id = "---\ntype: session\nagent: claude-code\n---\n";
    assert_eq!(extract_session_id_from_frontmatter(no_id), None);
}
```

## Dependencies

- 없음 (독립 실행 가능)
- `session_exists_by_prefix()`는 `bm25.rs`의 `Database impl` 블록 또는 `store/db.rs`에 추가

## Verification

```bash
# 1. 컴파일 확인
cargo check -p secall-core

# 2. lint 테스트 통과 (기존 + 신규)
cargo test -p secall-core lint

# 3. 전체 테스트 회귀 없음
cargo test
```

## Risks

- **frontmatter 없는 레거시 파일**: 초기 버전에서 생성된 vault 파일에 frontmatter가 없을 수 있음. fallback으로 파일명 파싱 + `session_exists_by_prefix()` LIKE 쿼리를 사용하여 대응.
- **LIKE 쿼리 성능**: prefix가 8자 이상이면 충돌 가능성 극히 낮음. 대규모 DB에서도 sessions 테이블 크기는 수천 행 수준이므로 성능 문제 없음.
- **frontmatter 형식 변경**: `render_session()`이 `session_id:` 필드를 출력하지 않으면 fallback 경로로 진행. markdown.rs를 확인하여 frontmatter에 `session_id`가 포함되는지 검증 필요.

## Scope boundary

다음 파일은 영향을 받을 수 있으나 이 task에서 수정하지 않음:
- `crates/secall-core/src/ingest/markdown.rs` — frontmatter 형식 변경 없음 (이미 session_id 포함 확인 필요)
- `crates/secall-core/src/vault/mod.rs:63-84` — `Vault::session_exists()`는 별도 로직 (파일명 substring 매칭), L002와 무관
