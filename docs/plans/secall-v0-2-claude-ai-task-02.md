---
type: task
status: draft
plan: secall-v0-2-claude-ai
task_number: 2
title: "AgentKind 확장 + SessionParser trait 1:N 지원"
parallel_group: A
depends_on: []
updated_at: 2026-04-07
---

# Task 02: AgentKind 확장 + SessionParser trait 1:N 지원

## 문제

1. `AgentKind` enum (types.rs:7-11)에 `ClaudeAi` variant가 없어 claude.ai 대화를 구분할 수 없다.
2. `SessionParser::parse()` (mod.rs:18)가 1:1 (1 file → 1 Session)만 지원. claude.ai export는 1 file → N conversations이므로 1:N 파싱이 필요하다.

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/ingest/types.rs:7-21` | 수정 | `AgentKind::ClaudeAi` variant 추가, `as_str()` 매핑 |
| `crates/secall-core/src/ingest/mod.rs:13-22` | 수정 | `SessionParser` trait에 `parse_all()` default method 추가 |
| `crates/secall-core/src/ingest/mod.rs:3` | 수정 | `pub mod claude_ai;` 추가 (Task 03 파일 등록) |

## Change description

### Step 1: AgentKind에 ClaudeAi 추가

`crates/secall-core/src/ingest/types.rs` — AgentKind (lines 6-21):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentKind {
    ClaudeCode,
    ClaudeAi,     // 추가
    Codex,
    GeminiCli,
}

impl AgentKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentKind::ClaudeCode => "claude-code",
            AgentKind::ClaudeAi => "claude-ai",     // 추가
            AgentKind::Codex => "codex",
            AgentKind::GeminiCli => "gemini-cli",
        }
    }
}
```

> `"claude-ai"` 문자열은 vault 파일명, frontmatter `agent:`, 검색 필터에 사용됨.

### Step 2: SessionParser trait에 parse_all() 추가

`crates/secall-core/src/ingest/mod.rs` — SessionParser trait (lines 13-22):

```rust
pub trait SessionParser: Send + Sync {
    /// Check if this parser can handle the given path
    fn can_parse(&self, path: &Path) -> bool;

    /// Parse the session file and return a Session
    fn parse(&self, path: &Path) -> crate::error::Result<Session>;

    /// The agent kind this parser handles
    fn agent_kind(&self) -> AgentKind;

    /// Parse a file that may contain multiple sessions (1:N).
    /// Default: wraps parse() for 1:1 parsers.
    fn parse_all(&self, path: &Path) -> crate::error::Result<Vec<Session>> {
        Ok(vec![self.parse(path)?])
    }
}
```

> 기존 3개 파서(ClaudeCode, Codex, Gemini)는 `parse_all()` 오버라이드 불필요. default가 1:1로 동작.

### Step 3: claude_ai 모듈 등록

`crates/secall-core/src/ingest/mod.rs` — module 선언부 (lines 3-9):

```rust
pub mod claude;
pub mod claude_ai;  // 추가
pub mod codex;
pub mod detect;
pub mod gemini;
pub mod lint;
pub mod markdown;
pub mod types;
```

> 실제 `claude_ai.rs` 파일은 Task 03에서 생성. 이 task에서는 `pub mod claude_ai;` 선언만 추가.
> 단, Task 03이 먼저 완료되지 않으면 컴파일 에러. 순서: Task 02 + Task 03 동시 또는 Task 03 먼저.

### Step 4: 기존 코드 영향 확인

`AgentKind` enum에 variant 추가하면 `match` 문이 non-exhaustive 에러 발생하는 곳 확인:

- `types.rs:14-19` — `as_str()`: Step 1에서 처리
- `markdown.rs:65` — `session.agent.as_str()`: 자동 처리 (as_str() 반환값 사용)
- `detect.rs:8` — `detect_parser()`: Task 04에서 처리

> `match` exhaustiveness 에러가 나면 컴파일이 실패하므로, 미처리 지점이 있으면 즉시 발견됨.

## Dependencies

- 없음
- Task 01과 독립적으로 구현 가능
- Task 03, 04의 선행 조건

## Verification

```bash
# 1. 컴파일 확인 (claude_ai.rs가 없으면 실패 — Task 03과 함께 확인)
cargo check --all

# 2. 기존 테스트 통과
cargo test --all

# 3. AgentKind::ClaudeAi 존재 확인
grep -n "ClaudeAi" crates/secall-core/src/ingest/types.rs

# 4. parse_all default method 존재 확인
grep -n "parse_all" crates/secall-core/src/ingest/mod.rs

# 5. clippy 통과
cargo clippy --all-targets -- -D warnings
```

## Risks

- **match exhaustiveness**: `AgentKind`에 variant 추가 시 모든 match 문에서 처리 필요. 컴파일러가 강제하므로 누락 불가.
- **serde 호환**: 기존 DB에 `"claude-ai"` agent 값이 없음. 새 variant는 새 데이터에만 적용. 역직렬화 시 unknown variant는 에러 → 기존 데이터에 영향 없음.
- **Task 03 의존**: `pub mod claude_ai;` 선언 시 파일이 없으면 컴파일 에러. Task 02와 03을 동시 진행하거나, 빈 파일을 placeholder로 생성.

## Scope boundary

수정 금지 파일:
- `crates/secall-core/src/ingest/claude.rs` — 기존 Claude Code 파서 변경 없음
- `crates/secall-core/src/ingest/codex.rs` — 기존 Codex 파서 변경 없음
- `crates/secall-core/src/ingest/gemini.rs` — 기존 Gemini 파서 변경 없음
- `crates/secall-core/src/ingest/detect.rs` — Task 04 영역
- `crates/secall/src/commands/ingest.rs` — Task 04 영역
- `Cargo.toml` — Task 01 영역 (version), Task 03 영역 (zip 의존성)
