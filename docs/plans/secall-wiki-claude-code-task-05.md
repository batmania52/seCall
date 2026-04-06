---
type: task
status: draft
plan: secall-wiki-claude-code
task_number: 5
title: "위키 품질 검증 (secall lint 확장)"
parallel_group: B
depends_on: [1]
updated_at: 2026-04-06
---

# Task 05: 위키 품질 검증 (secall lint 확장)

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/ingest/lint.rs` | 수정 | L008, L009, L010 체크 함수 추가 + run_lint() 호출 |
| `crates/secall-core/src/store/db.rs` | 수정 | `list_all_session_ids()` 헬퍼 추가 (L010용) |

## Change description

### 1. 새 검증 항목

| # | 검증 | 심각도 | 설명 |
|---|---|---|---|
| L008 | wiki 페이지 frontmatter 누락 | warn | wiki/ 아래 .md 파일에 YAML frontmatter(title, type, sources)가 없는 경우 |
| L009 | wiki → raw 링크 깨짐 | error | wiki 페이지의 sources[]에 존재하지 않는 session ID가 참조됨 |
| L010 | 고아 세션 (wiki 미참조) | info | DB 세션 중 어떤 wiki 페이지의 sources에도 포함되지 않는 세션 |

### 2. check_wiki_frontmatter (L008)

```rust
fn check_wiki_frontmatter(config: &Config, findings: &mut Vec<LintFinding>) -> Result<()> {
    let wiki_dir = config.vault.path.join("wiki");
    if !wiki_dir.exists() {
        return Ok(()); // wiki 미초기화 — skip
    }

    for entry in walkdir::WalkDir::new(&wiki_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if !p.extension().map(|e| e == "md").unwrap_or(false) {
            continue;
        }

        let content = std::fs::read_to_string(p).unwrap_or_default();

        // YAML frontmatter 존재 확인
        if !content.starts_with("---\n") {
            findings.push(LintFinding {
                code: "L008".to_string(),
                severity: Severity::Warn,
                message: format!("wiki page missing frontmatter: {}", p.display()),
                session_id: None,
                path: Some(p.to_string_lossy().to_string()),
            });
            continue;
        }

        // title 필드 존재 확인
        let fm_end = content[4..].find("\n---").map(|i| i + 4);
        if let Some(end) = fm_end {
            let fm = &content[4..end];
            if !fm.contains("title:") {
                findings.push(LintFinding {
                    code: "L008".to_string(),
                    severity: Severity::Warn,
                    message: format!(
                        "wiki page missing 'title' in frontmatter: {}",
                        p.display()
                    ),
                    session_id: None,
                    path: Some(p.to_string_lossy().to_string()),
                });
            }
        }
    }
    Ok(())
}
```

### 3. check_wiki_source_links (L009)

```rust
fn check_wiki_source_links(
    db: &Database,
    config: &Config,
    findings: &mut Vec<LintFinding>,
) -> Result<()> {
    let wiki_dir = config.vault.path.join("wiki");
    if !wiki_dir.exists() {
        return Ok(());
    }

    for entry in walkdir::WalkDir::new(&wiki_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if !p.extension().map(|e| e == "md").unwrap_or(false) {
            continue;
        }

        let content = std::fs::read_to_string(p).unwrap_or_default();
        for sid in extract_sources(&content) {
            if !db.session_exists(&sid).unwrap_or(true) {
                findings.push(LintFinding {
                    code: "L009".to_string(),
                    severity: Severity::Error,
                    message: format!("wiki references non-existent session '{sid}'"),
                    session_id: Some(sid),
                    path: Some(p.to_string_lossy().to_string()),
                });
            }
        }
    }
    Ok(())
}
```

### 4. check_orphan_sessions (L010)

```rust
fn check_orphan_sessions(
    db: &Database,
    config: &Config,
    findings: &mut Vec<LintFinding>,
) -> Result<()> {
    let wiki_dir = config.vault.path.join("wiki");
    if !wiki_dir.exists() {
        return Ok(());
    }

    // Collect all session IDs referenced in wiki pages
    let mut referenced: HashSet<String> = HashSet::new();
    for entry in walkdir::WalkDir::new(&wiki_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if p.extension().map(|e| e == "md").unwrap_or(false) {
            let content = std::fs::read_to_string(p).unwrap_or_default();
            for sid in extract_sources(&content) {
                referenced.insert(sid);
            }
        }
    }

    for sid in db.list_all_session_ids() {
        if !referenced.contains(&sid) {
            findings.push(LintFinding {
                code: "L010".to_string(),
                severity: Severity::Info,
                message: "session not referenced in any wiki page".to_string(),
                session_id: Some(sid),
                path: None,
            });
        }
    }
    Ok(())
}
```

### 5. extract_sources 헬퍼

frontmatter의 `sources: ["id1", "id2"]` 을 간단히 파싱:

```rust
fn extract_sources(content: &str) -> Vec<String> {
    let mut sources = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("sources:") {
            if let Some(arr_start) = trimmed.find('[') {
                if let Some(arr_end) = trimmed.find(']') {
                    let arr = &trimmed[arr_start + 1..arr_end];
                    for item in arr.split(',') {
                        let s = item.trim().trim_matches('"').trim_matches('\'');
                        if !s.is_empty() {
                            sources.push(s.to_string());
                        }
                    }
                }
            }
        }
    }
    sources
}
```

### 6. run_lint() 수정

`run_lint()`에 3개 체크 함수 호출 추가:

```rust
// 기존 L001~L007 이후 추가
check_wiki_frontmatter(config, &mut findings)?;
check_wiki_source_links(db, config, &mut findings)?;
check_orphan_sessions(db, config, &mut findings)?;
```

### 7. DB 헬퍼

`store/db.rs`에 추가:

```rust
impl Database {
    pub fn list_all_session_ids(&self) -> Vec<String> {
        let mut stmt = self.conn()
            .prepare("SELECT id FROM sessions")
            .unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }
}
```

## Dependencies

- Task 01 (wiki/ 디렉토리 구조 + SCHEMA.md)
- 기존 lint 코드 (`ingest/lint.rs`) 존재

## Verification

```bash
# 타입 체크
cargo check -p secall-core

# lint 테스트
cargo test -p secall-core lint

# 전체 테스트 회귀
cargo test -p secall-core
```

테스트 작성 요구사항:
- `test_lint_wiki_no_dir`: wiki/ 미존재 시 L008~L010 건너뜀 (0 findings)
- `test_lint_wiki_missing_frontmatter`: frontmatter 없는 wiki 페이지 → L008 warn
- `test_lint_wiki_broken_source`: sources에 존재하지 않는 세션 ID → L009 error
- `test_lint_wiki_orphan_session`: wiki에 참조되지 않는 세션 → L010 info

## Risks

- **extract_sources 파싱 한계**: 간단한 문자열 파싱이라 멀티라인 sources는 미지원. SCHEMA.md에서 단일 라인 형식을 강제하여 해결
- **대량 세션 시 L010 노이즈**: 위키가 아직 적으면 대부분 세션이 L010으로 잡힘. severity가 Info이므로 `--errors-only`로 필터 가능

## Scope Boundary

수정 금지 파일:
- `ingest/claude.rs`, `ingest/codex.rs`, `ingest/gemini.rs` — 파서 변경 금지
- `search/*` — 검색 모듈 변경 금지
- `mcp/*` — MCP 서버 변경 금지
- `vault/init.rs` — Task 01 영역
