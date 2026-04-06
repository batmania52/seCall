---
type: task
status: draft
plan: secall-extensions-nlp
task_number: 2
title: "Gemini CLI 파서"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 02: Gemini CLI 파서

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/ingest/gemini.rs` | **신규 생성** | GeminiParser 구현 |
| `crates/secall-core/src/ingest/mod.rs:5-8` | 수정 | `pub mod gemini;` 추가, re-export |
| `crates/secall-core/src/ingest/detect.rs:8-31` | 수정 | Gemini 경로 패턴 + content sniffing 추가 |
| `crates/secall-core/src/ingest/detect.rs` | 수정 | `find_gemini_sessions()` 함수 추가 |
| `crates/secall/src/commands/ingest.rs` | 수정 | `--auto` 모드에 Gemini 세션 탐색 추가 |

## Change description

### 1. Gemini CLI JSON 포맷 분석

Gemini CLI 세션 파일 위치: `~/.gemini/tmp/<projectId>/chats/session-*.json`

포맷: **단일 JSON 파일** (JSONL이 아님). camelCase 필드명.
```json
{
  "id": "session-abc123",
  "createTime": "2026-04-05T10:00:00Z",
  "updateTime": "2026-04-05T10:30:00Z",
  "messages": [
    {
      "role": "user",
      "parts": [{ "text": "검색 기능 구현해줘" }]
    },
    {
      "role": "model",
      "parts": [
        { "text": "네, 구현하겠습니다." },
        { "functionCall": { "name": "edit_file", "args": { "path": "src/main.rs" } } }
      ]
    },
    {
      "role": "function",
      "parts": [
        { "functionResponse": { "name": "edit_file", "response": { "result": "ok" } } }
      ]
    }
  ]
}
```

핵심 차이점 (vs Claude Code / Codex):
- 단일 JSON 파일 (JSONL이 아님)
- `parts` 배열 내 untagged union: `text`, `functionCall`, `functionResponse` 중 하나
- 모델 응답의 role은 `"model"` (not `"assistant"`)
- 도구 응답은 별도 `"function"` role 메시지
- projectId는 경로에서 추출

### 2. GeminiParser 구현 (gemini.rs)

```
pub struct GeminiParser;

impl SessionParser for GeminiParser {
    fn can_parse(&self, path: &Path) -> bool {
        // 경로에 /.gemini/ 포함 AND .json 확장자
    }

    fn parse(&self, path: &Path) -> Result<Session> {
        // 1. 전체 파일을 읽어 serde_json::from_str로 GeminiSession 파싱
        // 2. messages 순회하면서 Turn 변환:
        //    - "user" → Turn { role: User }
        //    - "model" → Turn { role: Assistant }
        //    - "function" → 이전 model Turn의 Action에 output 추가
        // 3. parts 배열 처리:
        //    - text → content에 연결
        //    - functionCall → Action::ToolUse
        //    - functionResponse → 매칭하여 output_summary
        // 4. projectId는 경로에서 추출 (/.gemini/tmp/<projectId>/chats/)
        // 5. Session 구조체 조립
    }

    fn agent_kind(&self) -> AgentKind { AgentKind::GeminiCli }
}
```

serde 모델:
```rust
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiSession {
    id: String,
    create_time: Option<String>,
    update_time: Option<String>,
    messages: Vec<GeminiMessage>,
}

#[derive(Deserialize)]
struct GeminiMessage {
    role: String,
    parts: Vec<GeminiPart>,
}

// Untagged enum — 필드 존재 여부로 variant 결정
#[derive(Deserialize)]
#[serde(untagged)]
enum GeminiPart {
    Text { text: String },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: GeminiFunctionCall,
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: GeminiFunctionResponse,
    },
}

#[derive(Deserialize)]
struct GeminiFunctionCall {
    name: String,
    args: serde_json::Value,
}

#[derive(Deserialize)]
struct GeminiFunctionResponse {
    name: String,
    response: serde_json::Value,
}
```

### 3. detect.rs 수정

`detect_parser()` 함수에 Gemini 경로 패턴 추가:
```rust
if path_str.contains("/.gemini/") && path_str.ends_with(".json") {
    return Ok(Box::new(GeminiParser));
}
```

content sniffing 추가 (Gemini JSON은 첫 줄이 `{`이고 `messages` 배열 존재):
```rust
// 파일 전체가 JSON이고 "messages" 배열 + 첫 message에 "parts" 존재
if v["messages"].is_array() && v["messages"][0]["parts"].is_array() {
    return Ok(Box::new(GeminiParser));
}
```

주의: content sniffing 시 전체 파일 로드가 필요할 수 있음. 파일 크기 체크(< 100MB) 후 시도.

`find_gemini_sessions()` 함수 추가:
```rust
pub fn find_gemini_sessions(base: Option<&Path>) -> Result<Vec<PathBuf>> {
    // ~/.gemini/tmp/**/chats/session-*.json 탐색
}
```

### 4. ingest.rs --auto 모드 확장

`--auto` 모드에서 `find_gemini_sessions()` 결과도 수집.

## Dependencies

- 없음 (Task 01과 병렬 실행 가능)
- MVP Task 01~11 완료 전제

## Verification

```bash
# 타입 체크
cargo check -p secall-core

# 단위 테스트
cargo test -p secall-core gemini

# 전체 기존 테스트 회귀 없음
cargo test -p secall-core
```

테스트 작성 요구사항:
- `test_gemini_parse_basic`: 샘플 JSON → Session 변환 검증
- `test_gemini_parts_union`: text / functionCall / functionResponse 각 variant 파싱
- `test_gemini_function_matching`: functionCall ↔ functionResponse 매칭
- `test_gemini_detect_path`: Gemini 경로 패턴 감지
- `test_gemini_project_extraction`: 경로에서 projectId 추출

## Risks

- **Untagged enum 순서 민감**: `#[serde(untagged)]`는 variant 순서대로 시도 → `Text`를 마지막에 배치 (다른 variant가 매칭 안 될 때 fallback). `FunctionCall`과 `FunctionResponse`가 둘 다 `name` 필드를 가지므로 `functionCall` / `functionResponse` 키로 구분
- **대용량 JSON**: Gemini 세션이 길면 전체 파일을 메모리에 로드 → 100MB 이상이면 경고 출력
- **Gemini CLI 버전 변화**: Google이 포맷을 변경할 수 있음 → `#[serde(default)]` 방어

## Scope Boundary

수정 금지 파일:
- `ingest/claude.rs` — 기존 파서 변경 금지
- `ingest/codex.rs` — Task 01 영역
- `search/*` — 검색 모듈은 이 태스크에서 변경하지 않음
