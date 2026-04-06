# Review Report: seCall Phase 4 — 검색 고도화 + 인프라 완성 — Round 3

> Verdict: pass
> Reviewer: 
> Date: 2026-04-06 09:39
> Plan Revision: 0

---

## Verdict

**pass**

## Recommendations

1. [vector.rs](/Users/d9ng/privateProject/seCall/crates/secall-core/src/search/vector.rs) 의 `default_model_path()`는 [model_manager.rs](/Users/d9ng/privateProject/seCall/crates/secall-core/src/search/model_manager.rs) 의 동일 로직과 중복이므로 한 곳으로 통합해 경로 drift를 막는 편이 안전합니다.
2. [server.rs](/Users/d9ng/privateProject/seCall/crates/secall-core/src/mcp/server.rs) 의 HTTP endpoint가 `/mcp` 이므로, 사용자 문서나 help 텍스트에 endpoint 경로를 명시해 SSE `/sse` 기대와 혼동이 없게 하는 편이 좋습니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | ort 모델 자동 다운로드 | ✅ done |
| 2 | OpenAI 임베딩 API embedder | ✅ done |
| 3 | MCP HTTP transport | ✅ done |
| 4 | LLM 쿼리 확장 | ✅ done |

