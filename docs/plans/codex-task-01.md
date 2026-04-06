---
type: task
status: draft
plan: codex
task_number: 1
title: "codex.rs 파서 재작성"
parallel_group: null
depends_on: []
updated_at: 2026-04-06
---

# Task 01: codex.rs 파서 재작성

## 문제

현재 `RolloutItem` enum이 `#[serde(tag = "type")]`으로 플랫 태그 매칭(`user`, `assistant`, `function_call`, `function_call_output`)을 시도하지만, 실제 Codex JSONL은 래퍼 구조:

```json
{"type": "session_meta", "payload": {...}}
{"type": "response_item", "payload": {"type": "message", "role": "user", ...}}
{"type": "response_item", "payload": {"type": "function_call", "name": "shell", ...}}
```

`type` 값이 `"response_item"` 등이라 enum variant에 매칭되지 않고, `Err(_) => continue`로 전부 skip → turns 빈 배열 → 에러.

## Changed files

| 파일 | 줄 범위 | 변경 |
|---|---|---|
| `crates/secall-core/src/ingest/codex.rs:30-53` | 수정 | `RolloutItem`, `MessageBody` 구조체를 래퍼 구조로 교체 |
| `crates/secall-core/src/ingest/codex.rs:55-172` | 수정 | `parse_codex_jsonl()` 함수 재작성 |
| `crates/secall-core/src/ingest/codex.rs:174-185` | 수정 | `extract_content()` → content item 배열 처리 변경 |
| `crates/secall-core/src/ingest/codex.rs:187-281` | 수정 | 테스트를 실제 JSONL 형식으로 업데이트 |

## Change description

### Step 1: 최상위 래퍼 구조체 정의 (line 30-53 교체)

기존 `RolloutItem` enum과 `MessageBody`를 제거하고 래퍼 구조로 교체:

```rust
/// 최상위 JSONL 라인 — type + payload
#[derive(Deserialize)]
struct JsonlLine {
    #[serde(rename = "type")]
    line_type: String,
    #[serde(default)]
    payload: serde_json::Value,
    #[serde(default)]
    timestamp: Option<String>,
}

/// session_meta payload
#[derive(Deserialize)]
struct SessionMeta {
    id: String,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    model_provider: Option<String>,
}

/// response_item payload (untagged — type 필드로 수동 분기)
#[derive(Deserialize)]
struct ResponsePayload {
    #[serde(rename = "type")]
    item_type: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: serde_json::Value,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    call_id: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
    #[serde(default)]
    output: Option<String>,
}
```

**설계 이유**: `response_item` payload의 `type` 값이 `"message"`, `"function_call"`, `"function_call_output"`, `"reasoning"` 등 다양하고, 각각 다른 필드 집합을 가짐. `#[serde(tag = "type")]` enum은 모든 variant의 필드를 미리 정의해야 하므로 비직렬화 실패에 취약. 플랫 구조체 + `item_type` 수동 분기가 가장 안전.

### Step 2: parse_codex_jsonl() 재작성 (line 55-172 교체)

```rust
pub fn parse_codex_jsonl(path: &Path) -> Result<Session> {
    let session_id = /* 기존 filename 추출 로직 유지 */;

    let file = std::fs::File::open(path)?;
    let file_mtime = /* 기존 mtime 로직 유지 */;
    let reader = std::io::BufReader::new(file);

    let mut turns: Vec<Turn> = Vec::new();
    let mut pending_calls: HashMap<String, (usize, usize)> = HashMap::new();
    let mut turn_idx: u32 = 0;

    // session_meta에서 추출
    let mut meta_id: Option<String> = None;
    let mut meta_timestamp: Option<DateTime<Utc>> = None;
    let mut meta_cwd: Option<String> = None;

    for line_result in reader.lines() {
        let line = line_result?;
        let line = line.trim();
        if line.is_empty() { continue; }

        let jl: JsonlLine = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        match jl.line_type.as_str() {
            "session_meta" => {
                if let Ok(meta) = serde_json::from_value::<SessionMeta>(jl.payload) {
                    meta_id = Some(meta.id);
                    meta_cwd = meta.cwd;
                    meta_timestamp = meta.timestamp
                        .and_then(|t| DateTime::parse_from_rfc3339(&t).ok())
                        .map(|dt| dt.with_timezone(&Utc));
                }
            }
            "response_item" => {
                let rp: ResponsePayload = match serde_json::from_value(jl.payload) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                match rp.item_type.as_str() {
                    "message" => {
                        let role_str = rp.role.as_deref().unwrap_or("");
                        // developer = 시스템 프롬프트 → skip
                        if role_str == "developer" { continue; }

                        let role = match role_str {
                            "user" => Role::User,
                            "assistant" => Role::Assistant,
                            _ => continue,
                        };

                        let content = extract_content(&rp.content);
                        if role == Role::User && content.is_empty() { continue; }

                        // 턴 타임스탬프: 래퍼의 timestamp 필드
                        let ts = jl.timestamp
                            .and_then(|t| DateTime::parse_from_rfc3339(&t).ok())
                            .map(|dt| dt.with_timezone(&Utc));

                        turns.push(Turn {
                            index: turn_idx,
                            role,
                            timestamp: ts,
                            content,
                            actions: Vec::new(),
                            tokens: None,
                            thinking: None,
                            is_sidechain: false,
                        });
                        turn_idx += 1;
                    }
                    "function_call" => {
                        let name = rp.name.unwrap_or_else(|| "unknown".to_string());
                        let call_id = rp.call_id.unwrap_or_default();
                        let arguments = rp.arguments.unwrap_or_default();

                        if let Some(last) = turns.last_mut() {
                            let action_idx = last.actions.len();
                            last.actions.push(Action::ToolUse {
                                name,
                                input_summary: arguments,
                                output_summary: String::new(),
                                tool_use_id: Some(call_id.clone()),
                            });
                            if !call_id.is_empty() {
                                pending_calls.insert(call_id, (turns.len() - 1, action_idx));
                            }
                        }
                    }
                    "function_call_output" => {
                        let call_id = rp.call_id.unwrap_or_default();
                        let output = rp.output.unwrap_or_default();

                        if let Some((turn_pos, action_idx)) = pending_calls.remove(&call_id) {
                            if let Some(turn) = turns.get_mut(turn_pos) {
                                if let Some(Action::ToolUse { output_summary, .. }) =
                                    turn.actions.get_mut(action_idx)
                                {
                                    *output_summary = output;
                                }
                            }
                        }
                    }
                    // "reasoning" 등 → skip
                    _ => {}
                }
            }
            // "event_msg", "turn_context" 등 → skip
            _ => {}
        }
    }

    if turns.is_empty() {
        return Err(anyhow!(
            "codex session has no parseable turns: {}",
            path.display()
        ));
    }

    // session_meta의 id가 있으면 우선 사용 (filename fallback)
    let final_id = meta_id.unwrap_or(session_id);

    // cwd에서 프로젝트명 추출: "/Users/d9ng/proj/seCall" → "seCall"
    let project = meta_cwd.as_deref()
        .and_then(|p| Path::new(p).file_name())
        .and_then(|n| n.to_str())
        .map(|s| s.to_string());

    // start_time 우선순위: session_meta.timestamp > file mtime > Utc::now()
    let start_time = meta_timestamp
        .or(file_mtime)
        .unwrap_or_else(Utc::now);

    Ok(Session {
        id: final_id,
        agent: AgentKind::Codex,
        model: None,
        project,
        cwd: meta_cwd,
        git_branch: None,
        start_time,
        end_time: None,
        turns,
        total_tokens: Default::default(),
    })
}
```

### Step 3: extract_content() 수정 (line 174-185 교체)

실제 content 배열은 `input_text`/`output_text` type을 사용:

```rust
fn extract_content(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|v| {
                let t = v.get("type").and_then(|t| t.as_str())?;
                // input_text (user), output_text (assistant) 모두 처리
                if t == "input_text" || t == "output_text" {
                    v.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}
```

**변경점**: 기존은 `v.get("text")` 무조건 추출 → `type`이 `"input_text"` 또는 `"output_text"`인 항목만 추출.

### Step 4: 테스트 업데이트 (line 187-281 교체)

모든 테스트를 실제 JSONL 형식으로 교체:

1. **test_codex_parse_basic** — `session_meta` + `response_item` message 형식
2. **test_codex_function_call_matching** — `response_item` function_call/output 형식
3. **test_codex_detect_path** — 변경 없음 (경로 판별 로직 동일)
4. **test_codex_detect_content** — 새 형식의 content sniffing
5. **test_codex_timestamp_from_meta** — `session_meta.timestamp` → `start_time` 매핑 확인
6. **test_codex_session_id_from_meta** — `session_meta.id` 우선 사용 확인
7. **test_codex_project_from_cwd** — `session_meta.cwd` → `project` 추출 확인
8. **test_codex_skip_developer_and_reasoning** — developer role과 reasoning type이 턴에 포함되지 않음 확인

테스트 JSONL 라인 예시:

```json
// session_meta
{"type":"session_meta","payload":{"id":"test-uuid","timestamp":"2026-04-05T10:00:00Z","cwd":"/Users/test/proj/myapp"}}

// user message
{"timestamp":"2026-04-05T10:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"검색 기능 구현해줘"}]}}

// assistant message
{"timestamp":"2026-04-05T10:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"구현하겠습니다"}]}}

// developer (skip)
{"timestamp":"2026-04-05T10:00:00Z","type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"system prompt"}]}}

// reasoning (skip)
{"timestamp":"2026-04-05T10:00:03Z","type":"response_item","payload":{"type":"reasoning","content":null,"summary":[]}}

// function_call
{"timestamp":"2026-04-05T10:00:04Z","type":"response_item","payload":{"type":"function_call","name":"shell","call_id":"call-1","arguments":"{\"command\":\"ls\"}"}}

// function_call_output
{"timestamp":"2026-04-05T10:00:05Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call-1","output":"file1.rs\nfile2.rs"}}
```

## Dependencies

- 없음. 다른 task에 의존하지 않음.
- 외부 crate 추가 없음 (기존 serde, serde_json, chrono, anyhow 사용).

## Verification

```bash
# 1. 컴파일 확인
cargo check -p secall-core

# 2. Codex 파서 유닛 테스트
cargo test -p secall-core codex

# 3. 전체 테스트 회귀 없음
cargo test

# 4. 실제 Codex 세션 파싱 (0 errors 확인)
cargo run -p secall -- ingest ~/.codex/sessions 2>&1 | tail -1
# 기대 출력: Summary: N ingested, M skipped (duplicate), 0 errors
```

> **[Developer 필수]** subtask-done 시그널 전에 위 명령의 실행 결과를 result 문서에 기록하세요. 형식: `✅ 명령 — exit 0` 또는 `❌ 명령 — 에러 내용 (사유)`. 검증 증빙 미제출 시 리뷰에서 conditional 처리됩니다.

## Risks

- **Codex CLI 버전 차이**: v0.118.0 기준 분석. 이전 버전이 다른 형식을 사용할 수 있음. `JsonlLine`의 `payload`를 `serde_json::Value`로 받고, 파싱 실패 시 `continue`로 방어하므로 크래시는 없음. 다만 이전 버전 세션이 0 turns로 에러 처리될 수 있음.
- **arguments 필드 타입**: 분석 샘플에서 `arguments`는 JSON 문자열이었으나, 일부 함수에서 구조체일 수 있음. `Option<String>`으로 받되, 실패 시 `serde_json::Value`로 fallback하여 `to_string()` 처리 고려.
- **대용량 output**: `function_call_output.output`이 매우 클 수 있음 (파일 전체 내용 등). 현재는 그대로 저장. vault 마크다운 렌더링 시 `TOOL_OUTPUT_MAX_CHARS` truncation이 적용되므로 vault 크기 문제는 없으나, 메모리 사용량 주의.

## Scope boundary

다음 파일은 이 task에서 수정하지 않음:

- `crates/secall-core/src/ingest/claude.rs` — Claude 파서
- `crates/secall-core/src/ingest/gemini.rs` — Gemini 파서
- `crates/secall-core/src/ingest/types.rs` — Session/Turn/Action 타입
- `crates/secall-core/src/ingest/markdown.rs` — vault 마크다운 렌더링
- `crates/secall-core/src/ingest/detect.rs` — 세션 파일 탐색
- `crates/secall/src/commands/ingest.rs` — ingest CLI 커맨드
