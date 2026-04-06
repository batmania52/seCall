# Review Report: seCall Extensions — 멀티에이전트 + 로컬 NLP — Round 2

> Verdict: fail
> Reviewer: 
> Date: 2026-04-06 07:18
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall-core/src/ingest/gemini.rs:149 — `functionResponse`를 함수명만으로 이전 assistant turn의 마지막 `ToolUse`에 매칭해 동일 함수명이 여러 번 호출된 경우 응답이 잘못 연결되거나 덮어써집니다.
2. crates/secall-core/src/search/tokenizer.rs:76 — 내부에 `RefCell` 기반 상태가 있다고 전제한 `kiwi_rs::Kiwi`를 동기화 없이 `unsafe impl Sync`로 노출해 동시 `tokenize()` 호출 시 메모리 안전성이 깨질 수 있습니다.

## Recommendations

1. Gemini 파서는 함수명 기반 역검색 대신 per-turn pending action queue를 두고 `functionResponse`를 호출 순서대로 소비하도록 바꾸세요.
2. Kiwi 토크나이저는 `unsafe impl Sync`를 제거하고 `Mutex` 등 실제 동기화로 안전성을 보장하세요.
3. crates/secall-core/src/ingest/gemini.rs:207 — `extract_project_id()`도 `can_parse()`와 동일하게 Windows 경로 구분자를 지원하도록 맞추는 편이 좋습니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | Codex CLI 파서 | ✅ done |
| 2 | Gemini CLI 파서 | ✅ done |
| 3 | ort ONNX 로컬 임베딩 | ✅ done |
| 4 | kiwi-rs 토크나이저 | ✅ done |
| 5 | secall lint | ✅ done |

