---
type: task
status: draft
plan: secall-v0-2-claude-ai
task_number: 1
title: "버전 bump + CHANGELOG"
parallel_group: A
depends_on: []
updated_at: 2026-04-07
---

# Task 01: 버전 bump + CHANGELOG

## 문제

프로젝트 버전이 `0.1.0`으로 고정되어 있고, 변경 이력을 추적하는 CHANGELOG가 없다.
v0.2 릴리스에 맞춰 버전을 올리고 변경 사항을 기록해야 한다.

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `Cargo.toml:6` | 수정 | `version = "0.1.0"` → `"0.2.0"` |
| `CHANGELOG.md` | 신규 | v0.1.0 + v0.2.0 변경 이력 |
| `README.md` | 수정 | v0.2 변경사항 섹션 추가 (claude.ai 파서) |

## Change description

### Step 1: Cargo.toml 버전 bump

`Cargo.toml:6` — workspace.package.version:

```toml
[workspace.package]
version = "0.2.0"
```

> crates/secall-core/Cargo.toml과 crates/secall/Cargo.toml은 `version.workspace = true`이므로 자동 반영.

### Step 2: CHANGELOG.md 생성

프로젝트 루트에 신규 생성:

```markdown
# Changelog

## v0.2.0 (2026-04-07)

### Added
- claude.ai 공식 export JSON 파서 (`ClaudeAiParser`)
- ZIP 자동 해제 지원 (`secall ingest <export.zip>`)
- `AgentKind::ClaudeAi` variant
- `SessionParser::parse_all()` — 1:N 파싱 지원

### Changed
- `AgentKind` enum에 `ClaudeAi` variant 추가
- `detect.rs`에 claude.ai export 자동 탐지 로직 추가

## v0.1.0 (2026-04-06)

### Added
- 초기 릴리스
- Claude Code / Codex CLI / Gemini CLI 파서
- BM25 + 벡터 하이브리드 검색 (RRF k=60)
- MCP 서버 (stdio + HTTP)
- Obsidian 호환 vault 구조
- Git 기반 멀티 기기 동기화 (`secall sync`)
- ANN 인덱스 (usearch HNSW)
- CI/CD GitHub Actions
```

### Step 3: README.md 업데이트

Multi-Agent Ingestion 테이블에 claude.ai 행 추가:

```markdown
| Agent | Format | Status |
|---|---|---|
| Claude Code | JSONL | ✅ Stable |
| Codex CLI | JSONL | ✅ Stable |
| Gemini CLI | JSON | ✅ Stable |
| claude.ai | JSON (ZIP) | ✅ New in v0.2 |
```

Quick Start에 예시 추가:

```bash
# Ingest claude.ai export (ZIP or extracted JSON)
secall ingest ~/Downloads/data-2026-04-06.zip
```

## Dependencies

- 없음
- Task 02, 03, 04와 독립적으로 구현 가능

## Verification

```bash
# 1. 버전 확인
grep 'version = "0.2.0"' Cargo.toml

# 2. 컴파일 확인
cargo check --all

# 3. CHANGELOG 존재 확인
test -f CHANGELOG.md && echo "OK"

# 4. 전체 테스트 통과
cargo test --all
```

## Risks

- **버전 bump 시 의존 crate 호환**: workspace version이므로 하위 crate 자동 반영. 외부 의존 없음.

## Scope boundary

수정 금지 파일:
- `crates/secall-core/src/ingest/types.rs` — Task 02 영역
- `crates/secall-core/src/ingest/mod.rs` — Task 02 영역
- `crates/secall-core/src/ingest/detect.rs` — Task 04 영역
- `crates/secall-core/Cargo.toml` — Task 03에서 `zip` 의존성 추가
