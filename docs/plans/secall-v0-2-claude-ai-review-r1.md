# Review Report: seCall v0.2 — claude.ai 파서 + 버전 명기 — Round 1

> Verdict: fail
> Reviewer: 
> Date: 2026-04-07 03:55
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall/src/commands/ingest.rs:79 — claude.ai JSON을 1:N으로 처리할지 결정할 때 `is_claude_ai_json()`의 200바이트 휴리스틱에 의존합니다. pretty-printed export처럼 앞 200바이트 안에 `"chat_messages"`가 나오지 않으면 `is_multi`가 `false`가 되고, 이후 `parse_file()`가 `ClaudeAiParser::parse()`를 호출해 첫 대화 1개만 ingest합니다. 유효한 `conversations.json`에서 나머지 대화가 조용히 누락되는 데이터 손실 버그입니다.

## Recommendations

1. `ingest_sessions()`에서 별도 200바이트 휴리스틱으로 multi-session 여부를 추정하지 말고, `detect_parser()` 결과가 `ClaudeAiParser`인지 확인한 뒤 항상 `parse_all()` 경로로 보내는 방식으로 바꾸는 것이 안전합니다.
2. `crates/secall/src/commands/ingest.rs`에 pretty-printed `conversations.json` fixture를 추가해 여러 대화가 모두 ingest되는 회귀 테스트를 넣는 편이 좋습니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | 버전 bump + CHANGELOG | ✅ done |
| 2 | AgentKind 확장 + SessionParser trait 1:N 지원 | ✅ done |
| 3 | ClaudeAiParser 구현 (ZIP 자동해제 포함) | ✅ done |
| 4 | detect.rs 연동 + CLI 통합 | ✅ done |

