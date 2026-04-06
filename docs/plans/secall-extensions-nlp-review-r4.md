# Review Report: seCall Extensions — 멀티에이전트 + 로컬 NLP — Round 4

> Verdict: fail
> Reviewer: 
> Date: 2026-04-06 07:35
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall-core/src/ingest/gemini.rs:162 — `functionResponse.name`을 확인하지 않고 FIFO 큐의 첫 pending 호출에 바로 연결합니다. 응답 순서가 어긋나거나 중간 응답이 누락되면 다른 tool call의 `output_summary`가 잘못 덮여 잘못된 세션이 저장됩니다.

## Recommendations

1. Gemini 매칭은 FIFO만 쓰지 말고 최소한 queued tool name과 `functionResponse.name`이 일치하는지 검증하고, 불일치 시 앞선 pending을 보존한 채 적절한 대상만 찾아 연결하거나 경고를 남기세요.
2. Task 2에는 동일 함수명 반복 케이스뿐 아니라 "서로 다른 함수 응답 순서 뒤바뀜" 회귀 테스트도 추가하는 편이 안전합니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | Codex CLI 파서 | ✅ done |
| 2 | Gemini CLI 파서 | ✅ done |
| 3 | ort ONNX 로컬 임베딩 | ✅ done |
| 4 | kiwi-rs 토크나이저 | ✅ done |
| 5 | secall lint | ✅ done |

