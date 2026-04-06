---
type: plan
status: draft
version: "1.0"
updated_at: 2026-04-06
slug: codex
title: "Codex 파서 핫픽스"
---

# Codex 파서 핫픽스

## Description

실제 Codex CLI JSONL 형식에 맞게 파서를 재작성한다.

현재 파서(`codex.rs`)는 `{"type":"user","message":{...}}` 플랫 구조를 기대하지만, 실제 Codex CLI(v0.118.0+)는 `{"type":"response_item","payload":{...}}` 래퍼 구조를 사용한다.

### 실제 JSONL 구조 (분석 결과)

| 라인 type | payload 구조 | 용도 |
|---|---|---|
| `session_meta` | `{id, timestamp, cwd, model_provider, cli_version}` | 세션 메타 |
| `event_msg` | `{type: "task_started\|token_count\|agent_message\|user_message\|task_complete"}` | 이벤트 |
| `response_item` | `{type: "message", role: "user\|assistant\|developer", content: [{type: "input_text\|output_text", text}]}` | 대화 메시지 |
| `response_item` | `{type: "function_call", name, call_id, arguments}` | 도구 호출 |
| `response_item` | `{type: "function_call_output", call_id, output}` | 도구 결과 |
| `response_item` | `{type: "reasoning", content: null\|encrypted, summary: []}` | 추론 (skip) |

### 샘플 데이터 통계 (113줄 세션)

- `session_meta`: 1
- `turn_context`: 1
- `event_msg`: 22 (token_count 11, agent_message 8, user_message 1, task_started 1, task_complete 1)
- `response_item`: 89 (function_call 34, function_call_output 34, reasoning 10, assistant message 8, user message 2, developer message 1)

## Expected Outcome

- `~/.codex/sessions/` 하위 134개 파일 전부 파싱 성공
- `session_meta`에서 `cwd` → project 이름, `timestamp` → start_time 추출
- `response_item` message에서 user/assistant 턴 생성
- `function_call`/`function_call_output`에서 tool action 매칭
- `reasoning`과 `developer` role은 skip

## Subtasks

1. **codex.rs 파서 재작성** — RolloutItem enum을 래퍼 구조로 변경, session_meta 파싱 추가, response_item 내부 분기 처리, 테스트를 실제 형식으로 업데이트
   - parallel_group: —
   - depends_on: —

## Constraints

- `Session`, `Turn`, `Action` 구조체는 변경하지 않음
- `developer` role 메시지는 시스템 프롬프트이므로 턴에 포함하지 않음
- `reasoning`은 encrypted 또는 null이므로 무조건 skip
- 다른 파서(claude.rs, gemini.rs)는 건드리지 않음

## Non-goals

- `event_msg` 내 `token_count`에서 토큰 사용량 집계 (향후 개선)
- `agent_message`/`user_message` 이벤트를 턴에 반영
- Codex `exec` vs `interactive` 모드 분기 처리
- `turn_context` 파싱

## Risks

- **Codex CLI 버전 차이**: v0.118.0 기준 분석. 이전 버전은 다른 형식일 수 있음. `serde(default)` + skip으로 방어.
- **encrypted reasoning**: 향후 복호화 API가 제공되면 thinking 필드에 매핑 가능. 현재는 skip.
