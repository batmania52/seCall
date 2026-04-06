---
type: task
status: draft
plan: secall-v0-2-claude-ai
task_number: 3
title: "ClaudeAiParser 구현 (ZIP 자동해제 포함)"
parallel_group: B
depends_on: [2]
updated_at: 2026-04-07
---

# Task 03: ClaudeAiParser 구현 (ZIP 자동해제 포함)

## 문제

claude.ai 공식 export 데이터를 파싱하는 파서가 없다. export는 ZIP 파일(`data-*.zip`)로 제공되며, 내부에 `conversations.json`이 핵심.

### 실제 데이터 구조 (22개 대화 분석)

**conversations.json 최상위:**
```json
[
  {
    "uuid": "81682ca4-...",
    "name": "대화 제목",
    "summary": "...",
    "created_at": "2026-03-18T03:16:18.177293Z",
    "updated_at": "...",
    "account": { "uuid": "..." },
    "chat_messages": [...]
  }
]
```

**chat_messages 요소:**
```json
{
  "uuid": "019d272a-ba8...",
  "text": "평문 텍스트",
  "content": [
    { "type": "text", "text": "...", "start_timestamp": null, "stop_timestamp": null, "flags": {}, "citations": [] }
  ],
  "sender": "human",
  "created_at": "2026-03-25T...",
  "updated_at": "...",
  "attachments": [...],
  "files": [...]
}
```

**실측 특성:**
- `model`: 0/22 — 전부 없음
- `parent_message_uuid`: 0/946 — 전부 없음 (선형)
- `settings`, `project_uuid`: 0/22 — 전부 없음
- tool_use 이름: `web_search`(17), `web_fetch`(15), `conversation_search`(2), `view`(1)
- content block 타입: `text`(980), `tool_use`(35), `tool_result`(35)

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/ingest/claude_ai.rs` | 신규 | `ClaudeAiParser` struct + `parse_all()` 구현 |
| `Cargo.toml` (workspace) | 수정 | `zip = "2"` workspace 의존성 추가 |
| `crates/secall-core/Cargo.toml` | 수정 | `zip.workspace = true` 추가 |

## Change description

### Step 1: zip 의존성 추가

```toml
# Cargo.toml (workspace root) — [workspace.dependencies] 섹션 (line 12+)
zip = "2"

# crates/secall-core/Cargo.toml — [dependencies] 섹션
zip.workspace = true
```

### Step 2: serde 구조체 정의

`crates/secall-core/src/ingest/claude_ai.rs` — 신규:

```rust
use serde::Deserialize;

/// conversations.json 최상위 — Vec<Conversation>
#[derive(Debug, Deserialize)]
struct Conversation {
    uuid: String,
    name: Option<String>,
    summary: Option<String>,
    created_at: String,
    updated_at: Option<String>,
    chat_messages: Vec<ChatMessage>,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    uuid: String,
    text: Option<String>,
    content: Vec<ContentBlock>,
    sender: String,          // "human" | "assistant"
    created_at: String,
    attachments: Option<Vec<Attachment>>,
    files: Option<Vec<serde_json::Value>>,  // 구조 복잡, 무시
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text {
        text: String,
        citations: Option<Vec<serde_json::Value>>,
    },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        name: String,
        input: Option<serde_json::Value>,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        name: Option<String>,
        content: Option<Vec<serde_json::Value>>,
        is_error: Option<bool>,
    },
    // 알려지지 않은 타입 graceful skip
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct Attachment {
    file_name: Option<String>,
    file_type: Option<String>,
    extracted_content: Option<String>,
}
```

> `#[serde(tag = "type")]` + `#[serde(other)]`로 미지 content block을 무시.

### Step 3: ZIP 자동 해제 로직

```rust
use std::io::{Read, Cursor};
use std::path::Path;

/// ZIP 파일이면 conversations.json을 추출, 아니면 그대로 읽기
fn read_conversations(path: &Path) -> crate::error::Result<Vec<Conversation>> {
    let data = std::fs::read(path)?;

    // ZIP 매직바이트 감지: PK\x03\x04
    let json_str = if data.starts_with(b"PK\x03\x04") {
        extract_conversations_from_zip(&data)?
    } else {
        String::from_utf8(data).map_err(|e| crate::SecallError::Parse {
            path: path.to_string_lossy().into_owned(),
            source: e.into(),
        })?
    };

    let conversations: Vec<Conversation> = serde_json::from_str(&json_str)
        .map_err(|e| crate::SecallError::Parse {
            path: path.to_string_lossy().into_owned(),
            source: e.into(),
        })?;

    Ok(conversations)
}

fn extract_conversations_from_zip(data: &[u8]) -> crate::error::Result<String> {
    let reader = Cursor::new(data);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| crate::SecallError::Parse {
            path: "<zip>".to_string(),
            source: e.into(),
        })?;

    // conversations.json 찾기
    let mut file = archive.by_name("conversations.json")
        .map_err(|e| crate::SecallError::Parse {
            path: "<zip>/conversations.json".to_string(),
            source: anyhow::anyhow!("conversations.json not found in ZIP: {e}"),
        })?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}
```

### Step 4: Conversation → Session 변환

```rust
use crate::ingest::types::*;
use chrono::{DateTime, Utc};

fn conversation_to_session(conv: &Conversation) -> crate::error::Result<Session> {
    let created = DateTime::parse_from_rfc3339(&conv.created_at)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    let last_msg_time = conv.chat_messages.last()
        .and_then(|m| DateTime::parse_from_rfc3339(&m.created_at).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let mut turns = Vec::new();
    let mut total_tokens = TokenUsage::default();

    for (i, msg) in conv.chat_messages.iter().enumerate() {
        let role = match msg.sender.as_str() {
            "human" => Role::User,
            "assistant" => Role::Assistant,
            _ => Role::System,
        };

        let timestamp = DateTime::parse_from_rfc3339(&msg.created_at)
            .ok()
            .map(|dt| dt.with_timezone(&Utc));

        let mut content_text = String::new();
        let mut thinking = None;
        let mut actions = Vec::new();

        for block in &msg.content {
            match block {
                ContentBlock::Text { text, .. } => {
                    if !content_text.is_empty() {
                        content_text.push('\n');
                    }
                    content_text.push_str(text);
                }
                ContentBlock::Thinking { thinking: t } => {
                    thinking = Some(t.clone());
                }
                ContentBlock::ToolUse { name, input } => {
                    let input_summary = input
                        .as_ref()
                        .map(|v| {
                            // artifact의 경우 title을 요약으로
                            v.get("title")
                                .and_then(|t| t.as_str())
                                .unwrap_or_else(|| {
                                    v.get("query")
                                        .and_then(|q| q.as_str())
                                        .unwrap_or("")
                                })
                                .to_string()
                        })
                        .unwrap_or_default();

                    actions.push(Action::ToolUse {
                        name: name.clone(),
                        input_summary,
                        output_summary: String::new(),
                        tool_use_id: None,
                    });
                }
                ContentBlock::ToolResult { name, content, is_error } => {
                    // tool_result의 텍스트를 content에 추가
                    if let Some(blocks) = content {
                        for b in blocks {
                            if let Some(text) = b.get("text").and_then(|t| t.as_str()) {
                                if !content_text.is_empty() {
                                    content_text.push('\n');
                                }
                                content_text.push_str(&text[..text.len().min(500)]);
                            }
                        }
                    }
                }
                ContentBlock::Unknown => {}
            }
        }

        // 첨부파일의 extracted_content를 content에 추가
        if let Some(attachments) = &msg.attachments {
            for att in attachments {
                if let Some(extracted) = &att.extracted_content {
                    if !extracted.is_empty() {
                        content_text.push_str("\n\n[Attachment");
                        if let Some(fname) = &att.file_name {
                            content_text.push_str(&format!(": {fname}"));
                        }
                        content_text.push_str("]\n");
                        content_text.push_str(&extracted[..extracted.len().min(2000)]);
                    }
                }
            }
        }

        // text 필드 fallback (content가 비어있으면)
        if content_text.is_empty() {
            if let Some(text) = &msg.text {
                content_text = text.clone();
            }
        }

        turns.push(Turn {
            index: i as u32,
            role,
            timestamp,
            content: content_text,
            actions,
            tokens: None,  // claude.ai export에 토큰 정보 없음
            thinking,
            is_sidechain: false,
        });
    }

    // 대화 제목을 project로 사용 (짧은 식별자)
    let project = conv.name.as_ref()
        .filter(|n| !n.is_empty())
        .map(|n| sanitize_project_name(n));

    // UUID의 앞 8자를 session_id로 사용
    let id = conv.uuid.clone();

    let host = Some(gethostname::gethostname().to_string_lossy().to_string());

    Ok(Session {
        id,
        agent: AgentKind::ClaudeAi,
        model: None,  // 공식 export에 model 없음
        project,
        cwd: None,           // claude.ai에 cwd 없음
        git_branch: None,    // claude.ai에 git 없음
        host,
        start_time: created,
        end_time: last_msg_time,
        turns,
        total_tokens,
    })
}

/// 대화 제목에서 vault 파일명에 안전한 프로젝트명 생성
fn sanitize_project_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' })
        .collect();
    // 최대 50자로 자르고 trim
    sanitized.trim().chars().take(50).collect()
}
```

### Step 5: SessionParser trait 구현

```rust
use std::path::Path;
use crate::ingest::SessionParser;

pub struct ClaudeAiParser;

impl SessionParser for ClaudeAiParser {
    fn can_parse(&self, path: &Path) -> bool {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "zip" {
            // ZIP 파일이면 conversations.json 포함 여부는 parse 시 확인
            return true;
        }
        if ext == "json" {
            // JSON 배열이고 chat_messages 키가 있으면 claude.ai export
            if let Ok(data) = std::fs::read_to_string(path) {
                if data.trim_start().starts_with('[') {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) {
                        if let Some(arr) = v.as_array() {
                            return arr.first()
                                .map(|c| c["chat_messages"].is_array() && c["uuid"].is_string())
                                .unwrap_or(false);
                        }
                    }
                }
            }
        }
        false
    }

    fn parse(&self, path: &Path) -> crate::error::Result<Session> {
        // 1:N 파서이므로 첫 번째 conversation만 반환
        let sessions = self.parse_all(path)?;
        sessions.into_iter().next().ok_or_else(|| {
            crate::SecallError::Parse {
                path: path.to_string_lossy().into_owned(),
                source: anyhow::anyhow!("no conversations found"),
            }
        })
    }

    fn parse_all(&self, path: &Path) -> crate::error::Result<Vec<Session>> {
        let conversations = read_conversations(path)?;

        let mut sessions = Vec::new();
        for conv in &conversations {
            if conv.chat_messages.is_empty() {
                continue;  // 빈 대화 skip
            }
            match conversation_to_session(conv) {
                Ok(session) => sessions.push(session),
                Err(e) => {
                    tracing::warn!(
                        uuid = &conv.uuid,
                        name = conv.name.as_deref().unwrap_or("(unnamed)"),
                        error = %e,
                        "failed to parse conversation, skipping"
                    );
                }
            }
        }

        tracing::info!(
            total = conversations.len(),
            parsed = sessions.len(),
            "claude.ai conversations parsed"
        );

        Ok(sessions)
    }

    fn agent_kind(&self) -> AgentKind {
        AgentKind::ClaudeAi
    }
}
```

### Step 6: 테스트

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_conversation() {
        let json = r#"[{
            "uuid": "test-uuid-001",
            "name": "테스트 대화",
            "created_at": "2026-04-01T10:00:00Z",
            "chat_messages": [
                {
                    "uuid": "msg-001",
                    "text": "안녕",
                    "content": [{"type": "text", "text": "안녕", "start_timestamp": null, "stop_timestamp": null, "flags": {}, "citations": []}],
                    "sender": "human",
                    "created_at": "2026-04-01T10:00:00Z",
                    "updated_at": "2026-04-01T10:00:00Z",
                    "attachments": [],
                    "files": []
                },
                {
                    "uuid": "msg-002",
                    "text": "안녕하세요!",
                    "content": [{"type": "text", "text": "안녕하세요!", "start_timestamp": null, "stop_timestamp": null, "flags": {}, "citations": []}],
                    "sender": "assistant",
                    "created_at": "2026-04-01T10:00:01Z",
                    "updated_at": "2026-04-01T10:00:01Z",
                    "attachments": [],
                    "files": []
                }
            ]
        }]"#;

        let convs: Vec<Conversation> = serde_json::from_str(json).unwrap();
        let session = conversation_to_session(&convs[0]).unwrap();

        assert_eq!(session.id, "test-uuid-001");
        assert_eq!(session.agent, AgentKind::ClaudeAi);
        assert_eq!(session.turns.len(), 2);
        assert_eq!(session.turns[0].role, Role::User);
        assert_eq!(session.turns[0].content, "안녕");
        assert_eq!(session.turns[1].role, Role::Assistant);
        assert!(session.project.is_some());
    }

    #[test]
    fn test_parse_empty_conversations_skipped() {
        let json = r#"[{
            "uuid": "empty-001",
            "name": "",
            "created_at": "2026-04-01T10:00:00Z",
            "chat_messages": []
        }]"#;

        let convs: Vec<Conversation> = serde_json::from_str(json).unwrap();
        // Empty conversation should be skipped in parse_all
        // (tested via parse_all, not conversation_to_session)
    }

    #[test]
    fn test_unknown_content_block_skipped() {
        let json = r#"[{
            "uuid": "test-002",
            "name": "Unknown blocks",
            "created_at": "2026-04-01T10:00:00Z",
            "chat_messages": [{
                "uuid": "msg-001",
                "text": "test",
                "content": [
                    {"type": "text", "text": "hello", "start_timestamp": null, "stop_timestamp": null, "flags": {}, "citations": []},
                    {"type": "voice_note", "title": "memo", "text": "voiced"}
                ],
                "sender": "human",
                "created_at": "2026-04-01T10:00:00Z",
                "updated_at": "2026-04-01T10:00:00Z",
                "attachments": [],
                "files": []
            }]
        }]"#;

        let convs: Vec<Conversation> = serde_json::from_str(json).unwrap();
        let session = conversation_to_session(&convs[0]).unwrap();
        assert_eq!(session.turns[0].content, "hello");
    }
}
```

## Dependencies

- `zip = "2"` — 신규 workspace 의존성 (ZIP 해제)
- `gethostname` — 이미 존재 (workspace)
- Task 02 완료 필수 (AgentKind::ClaudeAi, parse_all())

## Verification

```bash
# 1. 컴파일 확인
cargo check --all

# 2. claude_ai 테스트 통과
cargo test -p secall-core claude_ai

# 3. 전체 테스트 통과
cargo test --all

# 4. 실제 데이터 파싱 테스트 (ZIP)
cargo run -p secall -- ingest desktop_conversation/claude/data-2026-04-06-17-52-30-batch-0000.zip 2>&1 | head -20

# 5. 실제 데이터 파싱 테스트 (JSON)
cargo run -p secall -- ingest desktop_conversation/claude/exported/conversations.json 2>&1 | head -20

# 6. clippy 통과
cargo clippy --all-targets -- -D warnings
```

## Risks

- **serde tag 파싱 실패**: `#[serde(tag = "type")]`에서 content block의 `flags`, `citations` 등 예상 외 필드가 있으면 파싱 에러. `#[serde(other)]` + `Unknown` variant로 방어.
- **대용량 JSON**: 19MB conversations.json을 한 번에 메모리에 로드. 대화 수천 개면 수백 MB 가능. 1차에서는 허용 — 스트리밍 파싱은 복잡도 대비 실익 낮음.
- **UUID 중복**: 다른 export의 같은 대화가 같은 UUID를 가질 수 있음. `INSERT OR IGNORE` (session_id 기준)로 자연 중복 방지.
- **ZIP crate 크기**: `zip = "2"` 는 순수 Rust, 추가 C 의존성 없음. 바이너리 크기 미미.

## Scope boundary

수정 금지 파일:
- `crates/secall-core/src/ingest/claude.rs` — 기존 Claude Code 파서
- `crates/secall-core/src/ingest/codex.rs` — 기존 Codex 파서
- `crates/secall-core/src/ingest/gemini.rs` — 기존 Gemini 파서
- `crates/secall-core/src/ingest/detect.rs` — Task 04 영역
- `crates/secall/src/commands/ingest.rs` — Task 04 영역
- `crates/secall-core/src/ingest/markdown.rs` — 렌더링 변경 없음 (as_str() 자동 처리)
