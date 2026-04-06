# Review Report: seCall MVP — 에이전트 세션 검색 인프라 — Round 1

> Verdict: fail
> Reviewer: 
> Date: 2026-04-06 04:59
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall-core/src/search/hybrid.rs:87 — `SearchEngine::search()`가 BM25에는 `filters`를 적용하지만 vector 후보는 `vi.search(db, query, candidate_limit)`로 그대로 가져와 RRF에 합칩니다. 그래서 `--since`/`temporal` 검색에서 날짜 범위 밖 세션이 vector-only hit로 다시 섞여 들어올 수 있어, Task 08의 temporal filter 계약을 충족하지 못합니다.
2. crates/secall-core/src/mcp/server.rs:77 — MCP `recall`의 semantic 분기는 `base_filters`를 계산해놓고도 `search_with_embedding(&db, &embedding, limit)`에 필터를 전달하지 않습니다. 따라서 semantic 또는 temporal+semantic 조합 쿼리는 여전히 날짜/agent/project 범위를 벗어난 결과를 반환할 수 있어, 이전 finding 1과 3이 완전히 해소되지 않았습니다.

## Recommendations

1. vector 검색 결과 생성 시 `SearchFilters`를 함께 받아 `session_meta` 기준으로 `project`, `agent`, `since`, `until`을 동일하게 적용하고, `SearchEngine::search()`, MCP semantic branch 모두 그 공통 경로를 사용하도록 정리하는 편이 안전합니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | Rust workspace 초기화 | ✅ done |
| 2 | SQLite 스키마 설계 + 초기화 | ✅ done |
| 3 | Claude Code JSONL 파서 | ✅ done |
| 4 | Markdown 렌더러 | ✅ done |
| 5 | Vault 구조 초기화 + index/log 관리 | ✅ done |
| 6 | 한국어 BM25 인덱서 | ✅ done |
| 7 | 벡터 인덱서 + 검색 | ✅ done |
| 8 | 하이브리드 검색 (RRF) | ✅ done |
| 9 | CLI 완성 | ✅ done |
| 10 | MCP 서버 | ✅ done |
| 11 | Ingest 완료 이벤트 + hook | ✅ done |
| 12 | `secall lint` | ✅ done |

