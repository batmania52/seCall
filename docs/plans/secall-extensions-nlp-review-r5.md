# Review Report: seCall Extensions — 멀티에이전트 + 로컬 NLP — Round 5

> Verdict: conditional
> Reviewer: 
> Date: 2026-04-06 07:52
> Plan Revision: 0

---

## Verdict

**conditional**

## Findings

1. crates/secall-core/src/ingest/detect.rs:118 — `find_codex_sessions()`가 `.codex/sessions` 아래 모든 `.jsonl`을 수집해 task 계약의 `rollout-*.jsonl` 범위를 넘습니다. `--auto`가 비세션 파일까지 파싱 대상으로 삼을 수 있습니다.
2. crates/secall-core/src/ingest/detect.rs:149 — `find_gemini_sessions()`가 `chats/session-*.json` 경로 제약 없이 파일명만 검사합니다. Gemini 세션이 아닌 JSON이 자동 수집될 수 있습니다.
3. crates/secall-core/src/ingest/lint.rs:54 — `run_lint()`가 L006 agent stats를 finding으로 생성하지 않아 task에 명시된 `info` lint 항목이 결과에 나타나지 않습니다.

## Recommendations

1. session finder 테스트를 경로 제약 중심으로 보강해 auto-ingest 오탐을 막으세요.
2. L006의 의도를 summary인지 finding인지 하나로 정한 뒤 문서와 구현을 일치시키세요.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | Codex CLI 파서 | ✅ done |
| 2 | Gemini CLI 파서 | ✅ done |
| 3 | ort ONNX 로컬 임베딩 | ✅ done |
| 4 | kiwi-rs 토크나이저 | ✅ done |
| 5 | secall lint | ✅ done |

