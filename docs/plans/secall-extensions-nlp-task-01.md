---
type: task
status: draft
plan: secall-extensions-nlp
task_number: 1
title: "Codex CLI 파서"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 01: Codex CLI 파서

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/ingest/codex.rs` | **신규 생성** | CodexParser 구현 |
| `crates/secall-core/src/ingest/mod.rs:5-8` | 수정 | `pub mod codex;` 추가, re-export |
| `crates/secall-core/src/ingest/detect.rs:8-31` | 수정 | Codex 경로 패턴 + content sniffing 추가 |
| `crates/secall-core/src/ingest/detect.rs:34-63` | 수정 | `find_codex_sessions()` 함수 추가 |
| `crates/secall/src/commands/ingest.rs` | 수정 | `--auto` 모드에 Codex 세션 탐색 추가 |

## Change description

### 1. Codex JSONL 포맷 분석

Codex CLI 세션 파일 위치: `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`

각 줄은 adjacently tagged JSON:
```json
{"type": "user", "message": {"role": "user", "content": "..."}}
{"type": "assistant", "message": {"role": "assistant", "content": "..."}}
{"type": "function_call", "call_id": "...", "name": "shell", "arguments": "{\"command\": \"ls\"}"}
{"type": "function_call_output", "call_id": "...", "output": "..."}
```

핵심 차이점 (vs Claude Code JSONL):
- `sessionId` 필드 없음 → 파일명에서 ID 추출 (`rollout-<uuid>.jsonl`)
- `type` 필드로 메시지 종류 구분 (adjacently tagged)
- 도구 호출이 `function_call` / `function_call_output` 쌍으로 분리
- 모델 정보는 별도 `metadata` 줄 (있을 수도 없을 수도 있음)

### 2. CodexParser 구현 (codex.rs)

```
pub struct CodexParser;

impl SessionParser for CodexParser {
    fn can_parse(&self, path: &Path) -> bool {
        // 경로에 /.codex/sessions/ 포함 AND .jsonl 확장자
    }

    fn parse(&self, path: &Path) -> Result<Session> {
        // 1. 파일명에서 session ID 추출 (rollout-<uuid> → uuid)
        // 2. JSONL 한 줄씩 읽으면서 RolloutItem 파싱
        // 3. type별 분기:
        //    - "user" → Turn { role: User, content }
        //    - "assistant" → Turn { role: Assistant, content }
        //    - "function_call" → Action::ToolUse 시작 (call_id 매핑용 임시 저장)
        //    - "function_call_output" → call_id로 매칭하여 output_summary 채움
        // 4. Session 구조체 조립
    }

    fn agent_kind(&self) -> AgentKind { AgentKind::Codex }
}
```

RolloutItem serde 모델:
```rust
#[derive(Deserialize)]
#[serde(tag = "type")]
enum RolloutItem {
    #[serde(rename = "user")]
    User { message: MessageBody },
    #[serde(rename = "assistant")]
    Assistant { message: MessageBody },
    #[serde(rename = "function_call")]
    FunctionCall {
        call_id: String,
        name: String,
        arguments: String,
    },
    #[serde(rename = "function_call_output")]
    FunctionCallOutput {
        call_id: String,
        output: String,
    },
}

#[derive(Deserialize)]
struct MessageBody {
    role: String,
    content: String,
}
```

### 3. detect.rs 수정

`detect_parser()` 함수에 Codex 경로 패턴 추가:
```rust
// 기존: /.claude/projects/ → ClaudeCodeParser
if path_str.contains("/.codex/sessions/") {
    return Ok(Box::new(CodexParser));
}
```

content sniffing에 Codex 패턴 추가:
```rust
// "type": "user" + "message" 객체 존재 → Codex
if v["type"].is_string() && v["message"].is_object() {
    return Ok(Box::new(CodexParser));
}
```

`find_codex_sessions()` 함수 추가:
```rust
pub fn find_codex_sessions(base: Option<&Path>) -> Result<Vec<PathBuf>> {
    // ~/.codex/sessions/ 아래 **/*.jsonl 탐색
}
```

### 4. ingest.rs --auto 모드 확장

`--auto` 모드에서 `find_codex_sessions()` 결과도 수집하여 파싱 대상에 포함.

## Dependencies

- 없음 (Task 02와 병렬 실행 가능)
- MVP Task 01~11 완료 전제 (SessionParser trait, AgentKind enum, detect.rs 존재)

## Verification

```bash
# 타입 체크
cargo check -p secall-core

# 단위 테스트 (codex 파서 + detect 확장)
cargo test -p secall-core codex

# 전체 기존 테스트 회귀 없음
cargo test -p secall-core
```

테스트 작성 요구사항:
- `test_codex_parse_basic`: 샘플 JSONL → Session 변환 검증
- `test_codex_function_call_matching`: function_call ↔ function_call_output call_id 매칭
- `test_codex_detect_path`: Codex 경로 패턴 감지
- `test_codex_detect_content`: content sniffing으로 Codex 감지

## Risks

- **Codex JSONL 스키마 변경**: Codex CLI가 업데이트되면 필드 추가/변경 가능. serde `#[serde(default)]`로 방어
- **adjacently tagged enum 파싱 실패**: `#[serde(tag = "type")]`이 Codex의 실제 포맷과 다를 수 있음 → 샘플 파일로 사전 검증 필수
- **파일명 규칙 변경**: `rollout-*.jsonl` 외 다른 파일명이 존재할 수 있음 → `*.jsonl`로 확장 검토

## Scope Boundary

수정 금지 파일:
- `ingest/claude.rs` — 기존 Claude Code 파서는 변경하지 않음
- `search/*` — 검색 모듈은 이 태스크에서 변경하지 않음
- `mcp/*` — MCP 서버는 이 태스크에서 변경하지 않음
