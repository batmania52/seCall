# Review Report: seCall P16 — Knowledge Graph 빌드 (설계 수정) — Round 5

> Verdict: fail
> Reviewer: 
> Date: 2026-04-10 14:17
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall-core/src/graph/build.rs:110 — `include_in_relations = session_is_new || (!force && already)` 때문에 fresh DB에서 `graph build --since ...` 실행 시 since 이전 세션이 `all_frontmatters`에서 빠집니다. 그 결과 `same_project`/`same_day` 관계가 전체 vault 기준으로 재계산되지 않아 Task 02의 "since는 노드 upsert에만 적용, 관계 계산은 전체 vault 대상" 계약을 위반합니다.

## Recommendations

1. fresh DB + `--since` 케이스를 명시적으로 지원하거나 금지하세요. 지원할 경우 since 이전 세션의 session 노드를 최소 형태로 먼저 보장한 뒤 관계를 재계산하고, 금지할 경우 CLI와 task 문서에 선행 전체 빌드 필요 조건을 명확히 적고 회귀 테스트를 추가하는 편이 안전합니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | DB 스키마 + 마이그레이션 | ✅ done |
| 2 | Graph 코어 모듈 (rework) | ✅ done |
| 3 | CLI 서브커맨드 | ✅ done |
| 4 | Sync 통합 + MCP 확장 (rework) | ✅ done |

