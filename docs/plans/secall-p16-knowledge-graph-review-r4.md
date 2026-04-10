# Review Report: seCall P16 — Knowledge Graph 빌드 — Round 4

> Verdict: fail
> Reviewer: 
> Date: 2026-04-10 13:54
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall-core/src/graph/build.rs:60 — `--since` 필터가 날짜 디렉토리만 보지 않고 경로의 모든 10글자 세그먼트를 검사합니다. 따라서 `session001`처럼 길이 10인 세션 ID 파일명이 있으면 날짜와 무관하게 `part >= since_date`가 참이 되어, 오래된 세션도 잘못 포함됩니다.
2. crates/secall-core/src/graph/build.rs:122 — 증분 빌드가 새로 계산된 `same_project`/`same_day` 엣지를 `INSERT OR IGNORE`만 하고 기존 세션 간의 오래된 인접 엣지는 삭제하지 않습니다. 예를 들어 A→C가 있던 상태에서 시간상 중간 세션 B가 추가되면, 기대값은 A→B, B→C인데 기존 A→C가 남아 adjacency 기반 그래프 의미가 깨집니다.

## Recommendations

1. `build_graph`에서 `since`는 `raw/sessions/YYYY-MM-DD/...` 디렉토리명만 기준으로 판정하도록 바꾸고, 10글자 세션 ID가 있는 케이스를 회귀 테스트로 추가하세요.
2. 증분 빌드 전에 영향받는 세션의 `same_project`/`same_day` 관계를 재계산할 수 있도록 기존 관계 삭제 범위를 정의하고, “중간 세션 삽입” 회귀 테스트를 추가하세요.
3. 결과 문서는 Task 01~04별 Verification 명령과 결과를 분리해서 남기면 다음 재검토가 더 명확해집니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | DB 스키마 + 마이그레이션 | ✅ done |
| 2 | Graph 코어 모듈 | ✅ done |
| 3 | CLI 서브커맨드 | ✅ done |
| 4 | Sync 통합 + MCP 확장 | ✅ done |

