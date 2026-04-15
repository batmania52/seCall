# Review Report: P27 — BM25-only 선택 시 graph semantic 자동 비활성화 (#25) — Round 2

> Verdict: pass
> Reviewer: 
> Date: 2026-04-15 13:52
> Plan Revision: 0

---

## Verdict

**pass**

## Findings

1. 없음

## Recommendations

1. `docs/plans/p27-bm25-only-graph-semantic-25-task-02.md`의 Change description은 현재 `semantic_backend == "disabled"` 중심으로만 적혀 있으므로, 실제 수정된 `config.embedding.backend != "none"` 가드도 반영해 두면 계약 문서와 구현의 일치성이 더 명확해집니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | init.rs에서 BM25-only 선택 시 graph.semantic = false 자동 설정 | ✅ done |
| 2 | ingest.rs 방어 로직 + 전체 테스트 검증 | ✅ done |

