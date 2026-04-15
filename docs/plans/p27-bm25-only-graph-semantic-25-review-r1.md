# Review Report: P27 — BM25-only 선택 시 graph semantic 자동 비활성화 (#25) — Round 1

> Verdict: fail
> Reviewer: 
> Date: 2026-04-15 13:48
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall/src/commands/ingest.rs:365 — `semantic_enabled` 가드가 `config.embedding.backend == "none"` 불일치 상태를 차단하지 못합니다. 따라서 사용자가 `embedding.backend = "none"`인데 `graph.semantic = true`, `graph.semantic_backend = "ollama"`로 남긴 경우 여전히 semantic extraction과 LLM 호출이 발생해 Plan의 "수동 설정 파일 편집 대응" 요구를 충족하지 못합니다.

## Recommendations

1. `semantic_enabled` 조건에 `config.embedding.backend != "none"` 또는 동등한 BM25-only 차단 조건을 포함해, 수동 설정 불일치 상태에서도 semantic extraction이 시작되지 않도록 보강하세요.
2. 결과 문서에는 Task 01의 수동 검증 수행 여부도 함께 남겨 두는 편이 이후 재검토에 유리합니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | init.rs에서 BM25-only 선택 시 graph.semantic = false 자동 설정 | ✅ done |
| 2 | ingest.rs 방어 로직 + 전체 테스트 검증 | ✅ done |

