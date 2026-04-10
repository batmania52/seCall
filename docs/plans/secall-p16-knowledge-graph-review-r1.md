# Review Report: seCall P16 — Knowledge Graph 빌드 — Round 1

> Verdict: fail
> Reviewer: 
> Date: 2026-04-10 13:13
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall-core/src/graph/build.rs:92 — 증분 빌드 시 `extract_session_relations()`가 `all_frontmatters`(이번 실행에서 새로 처리한 세션)만 대상으로 수행됩니다. 이미 그래프에 있던 기존 세션은 83-86행에서 제외되므로, 새 세션과 기존 세션 사이의 `same_project`/`same_day` 관계가 영구히 누락됩니다.
2. crates/secall/src/commands/sync.rs:118 — sync Phase 3.7이 위 증분 `build_graph(..., false)`를 호출하므로, 새 세션을 ingest한 뒤에도 기존 그래프와의 세션 간 관계가 완전하게 갱신되지 않습니다.

## Recommendations

1. 증분 빌드 시 관계 계산용 입력에는 최소한 “새 세션 + 같은 project/day의 기존 세션”을 포함하도록 바꾸고, 그 경우를 검증하는 테스트를 `graph/build.rs`에 추가하세요.
2. `docs/plans/secall-p16-knowledge-graph-result.md`에 task별 Verification 명령과 exit/result를 그대로 남겨서 리뷰 추적성을 높이세요.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | DB 스키마 + 마이그레이션 | ✅ done |
| 2 | Graph 코어 모듈 | ✅ done |
| 3 | CLI 서브커맨드 | ✅ done |
| 4 | Sync 통합 + MCP 확장 | ✅ done |

