---
type: task
status: draft
plan: secall-v0-2-claude-ai
task_number: 4
title: "detect.rs 연동 + CLI 통합"
parallel_group: B
depends_on: [2, 3]
updated_at: 2026-04-07
---

# Task 04: detect.rs 연동 + CLI 통합

## 문제

`ClaudeAiParser`가 구현되어도 `detect_parser()` (detect.rs:8-60)와 `ingest` 명령 (ingest.rs)에 연동되지 않으면 사용자가 실제로 파싱할 수 없다.

### 현재 흐름

```
secall ingest <path>
  → collect_paths()        (ingest.rs:177)
    → detect_parser()      (detect.rs:8)  — claude.ai 미지원
      → parser.parse()     — 1:1만
```

### 목표 흐름

```
secall ingest <path.zip>
  → collect_paths()        — ZIP 파일도 단일 path로 전달
    → detect_parser()      — ZIP/JSON sniffing으로 ClaudeAiParser 반환
      → parser.parse_all() — 1:N 파싱
        → 각 Session을 개별 ingest
```

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/ingest/detect.rs:6` | 수정 | `use super::claude_ai::ClaudeAiParser;` import 추가 |
| `crates/secall-core/src/ingest/detect.rs:8-60` | 수정 | `detect_parser()`에 claude.ai 탐지 로직 추가 |
| `crates/secall/src/commands/ingest.rs:63-175` | 수정 | `ingest_sessions()`에서 `parse_all()` 분기 추가 |
| `crates/secall/src/commands/ingest.rs:235-238` | 수정 | `parse_file()`에서 parse_all 지원 |

## Change description

### Step 1: detect.rs에 claude.ai 탐지 추가

`crates/secall-core/src/ingest/detect.rs` — import (line 6):

```rust
use super::{
    claude::ClaudeCodeParser,
    claude_ai::ClaudeAiParser,  // 추가
    codex::CodexParser,
    gemini::GeminiParser,
    SessionParser,
};
```

`detect_parser()` (lines 8-60) — 기존 path-based 탐지 뒤에 추가:

```rust
pub fn detect_parser(path: &Path) -> Result<Box<dyn SessionParser>> {
    let path_str = path.to_string_lossy();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    // --- 기존 path-based 탐지 (lines 11-22, 변경 없음) ---

    // claude.ai export: ZIP 파일 (.zip 확장자)
    if ext == "zip" {
        // ZIP 매직바이트 확인 (PK\x03\x04)
        if let Ok(data) = std::fs::read(path) {
            if data.starts_with(b"PK\x03\x04") {
                return Ok(Box::new(ClaudeAiParser));
            }
        }
    }

    // --- 기존 content sniffing (lines 24-57) ---

    // claude.ai export: conversations.json (JSON array with chat_messages)
    // 기존 Gemini sniffing 뒤에 추가
    if ext == "json" {
        if let Ok(data) = std::fs::read_to_string(path) {
            if data.trim_start().starts_with('[') {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) {
                    if let Some(arr) = v.as_array() {
                        if arr.first()
                            .map(|c| c["chat_messages"].is_array() && c["uuid"].is_string())
                            .unwrap_or(false)
                        {
                            return Ok(Box::new(ClaudeAiParser));
                        }
                    }
                }
            }
        }
    }

    Err(anyhow!("unknown session format: {}", path.display()))
}
```

> **탐지 순서**: 
> 1. 기존 path-based (Claude Code, Codex, Gemini) — 변경 없음
> 2. ZIP 파일 → ClaudeAiParser (신규)
> 3. 기존 content sniffing (Claude Code JSONL, Codex JSONL)
> 4. 기존 Gemini full-parse
> 5. claude.ai JSON array sniffing (신규)

### Step 2: ingest.rs에서 parse_all() 호출

`crates/secall/src/commands/ingest.rs` — `parse_file()` (lines 235-238) 수정:

현재:
```rust
fn parse_file(path: &Path) -> Result<secall_core::ingest::Session> {
    let parser = detect_parser(path)?;
    Ok(parser.parse(path)?)
}
```

변경:
```rust
/// 1:1 파서용 (기존)
fn parse_file(path: &Path) -> Result<secall_core::ingest::Session> {
    let parser = detect_parser(path)?;
    Ok(parser.parse(path)?)
}

/// 1:N 파서용 (claude.ai 등)
fn parse_file_all(path: &Path) -> Result<Vec<secall_core::ingest::Session>> {
    let parser = detect_parser(path)?;
    Ok(parser.parse_all(path)?)
}
```

`ingest_sessions()` (lines 63-175) — for 루프 내 parse 분기:

```rust
for session_path in &paths {
    // 1:N 파서 감지 (ZIP 또는 claude.ai JSON)
    let is_multi = session_path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e == "zip")
        .unwrap_or(false)
        || is_claude_ai_json(session_path);

    if is_multi {
        // 1:N 파싱
        match parse_file_all(session_path) {
            Ok(sessions) => {
                eprintln!("Parsed {} conversations from {}", sessions.len(), session_path.display());
                for session in sessions {
                    // 기존 단일 세션 ingest 로직 (duplicate check → vault write → BM25 → vector)
                    ingest_single_session(&config, &db, &engine, &vault, session, format, &mut ingested, &mut skipped, &mut errors)?;
                }
            }
            Err(e) => {
                tracing::warn!(path = %session_path.display(), error = %e, "failed to parse multi-session file");
                errors += 1;
            }
        }
    } else {
        // 기존 1:1 파싱 (변경 없음)
        // ...
    }
}
```

> **리팩토링**: 기존 for 루프 내부의 단일 세션 처리 로직을 `ingest_single_session()` 헬퍼로 추출. 1:N과 1:1 모두 같은 함수 호출.

### Step 3: is_claude_ai_json 헬퍼

```rust
/// JSON 파일이 claude.ai export인지 간이 확인 (파일 앞부분만 읽음)
fn is_claude_ai_json(path: &Path) -> bool {
    if path.extension().and_then(|e| e.to_str()) != Some("json") {
        return false;
    }
    // 첫 100바이트만 읽어서 JSON array + chat_messages 키 확인
    if let Ok(mut f) = std::fs::File::open(path) {
        let mut buf = [0u8; 200];
        if let Ok(n) = std::io::Read::read(&mut f, &mut buf) {
            let s = String::from_utf8_lossy(&buf[..n]);
            return s.trim_start().starts_with('[')
                && s.contains("chat_messages");
        }
    }
    false
}
```

### Step 4: 테스트 추가

`crates/secall-core/src/ingest/detect.rs` — tests 모듈에 추가:

```rust
#[test]
fn test_detect_claude_ai_json() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = dir.path().join("conversations.json");
    std::fs::write(&json_path, r#"[{"uuid":"test","name":"","created_at":"2026-01-01T00:00:00Z","chat_messages":[{"uuid":"m1","text":"hi","content":[],"sender":"human","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","attachments":[],"files":[]}]}]"#).unwrap();
    let parser = detect_parser(&json_path).unwrap();
    assert_eq!(parser.agent_kind(), AgentKind::ClaudeAi);
}
```

## Dependencies

- Task 02 (AgentKind::ClaudeAi, parse_all)
- Task 03 (ClaudeAiParser struct)

## Verification

```bash
# 1. 컴파일 확인
cargo check --all

# 2. detect 테스트 통과
cargo test -p secall-core detect

# 3. 전체 테스트 통과
cargo test --all

# 4. 실제 ZIP 파일 ingest 테스트
cargo run -p secall -- ingest desktop_conversation/claude/data-2026-04-06-17-52-30-batch-0000.zip 2>&1 | tail -10

# 5. ingest 후 claude-ai 세션 검색 확인
cargo run -p secall -- recall "무엇이든" --agent claude-ai --limit 3

# 6. vault에 claude-ai 파일 존재 확인
ls ~/Documents/Obsidian\ Vault/seCall/raw/sessions/2026-03-*/claude-ai_* 2>/dev/null | head -5

# 7. 기존 파서 회귀 테스트
cargo run -p secall -- ingest --auto 2>&1 | tail -5

# 8. clippy 통과
cargo clippy --all-targets -- -D warnings
```

## Risks

- **ingest_single_session 추출 리팩토링**: 기존 for 루프 내부 로직을 함수로 추출하면 기존 ingest 동작에 영향 줄 수 있음. 함수 추출 후 기존 테스트 전체 통과 확인 필수.
- **ZIP 파일의 기존 파서 오탐**: `.zip` 확장자를 가진 모든 파일을 claude.ai로 판단. 다른 ZIP 파일(예: 코드 아카이브)을 넣으면 파싱 에러. `conversations.json` 존재 여부를 ZIP 내부에서 확인하므로 graceful failure.
- **대량 conversations**: 22개 대화에서 946개 메시지. 각 대화가 별도 Session으로 ingest되므로 22회의 vault write + BM25 + vector. 수분 소요 가능.
- **--auto에 claude.ai 미포함**: 의도적. claude.ai export는 고정 경로가 없으므로 `--auto` 탐지 불가. 사용자가 명시적으로 경로 지정 필요.

## Scope boundary

수정 금지 파일:
- `crates/secall-core/src/ingest/claude.rs` — 기존 Claude Code 파서
- `crates/secall-core/src/ingest/claude_ai.rs` — Task 03 영역 (호출만)
- `crates/secall-core/src/ingest/codex.rs` — 기존 Codex 파서
- `crates/secall-core/src/ingest/gemini.rs` — 기존 Gemini 파서
- `crates/secall-core/src/ingest/types.rs` — Task 02 영역
- `crates/secall-core/src/ingest/mod.rs` — Task 02 영역
- `crates/secall-core/src/search/` — 검색 로직 변경 없음
- `crates/secall-core/src/vault/` — vault 로직 변경 없음
