---
type: plan
status: in_progress
updated_at: 2026-04-10
version: 2
---

# seCall P16 — Knowledge Graph 빌드

## Description

vault의 정적 세션 MD 파일(960개+)에서 knowledge graph를 결정적으로 추출한다.
frontmatter 메타데이터(project, agent, tools_used, date)만으로 세션 간 관계를 구축하며,
LLM 호출 없이 순수 Rust로 동작한다.

기존 구조(raw/, wiki/, DB 테이블)는 변경하지 않으며, 순수 추가(additive) 구조.

### v2 변경 사항 (설계 수정)

Review 3회 연속 실패에 따른 설계 결함 수정:

1. **`--since` 필터 오탐** (build.rs:60) — 경로의 모든 10글자 세그먼트를 날짜로 간주하여 `session001` 같은 파일명도 매칭됨. → 부모 디렉토리명만 검사 + YYYY-MM-DD 패턴 검증 추가
2. **증분 빌드 시 인접 엣지 비갱신** (build.rs:122) — INSERT OR IGNORE만 사용하여 중간 세션 추가 시 기존 A→C 엣지가 삭제되지 않음. → same_project/same_day 엣지를 전체 DELETE 후 재계산
3. **증분 빌드 시 since 필터가 관계 계산 범위를 제한** — since로 잘린 범위 밖 세션이 관계 계산에서 빠짐. → since 필터는 개별 노드 upsert에만 적용, 관계 계산은 전체 vault 대상

## Expected Outcome

- `secall graph build` — vault 전체에서 결정적 그래프 추출 (LLM 불필요, 수초 내 완료)
- `secall graph build --since YYYY-MM-DD` — 증분 빌드 (since는 노드 생성만 필터, 관계는 전체 재계산)
- `secall graph stats` — 노드/엣지/클러스터 통계 출력
- `secall graph export` — vault/graph/ 디렉토리에 graph.json 출력
- sync 파이프라인에 Phase 3.7로 자동 통합
- MCP에 `graph_query` 도구 추가

## Subtask Summary

| # | Title | depends_on | parallel_group | Status |
|---|-------|------------|----------------|--------|
| 1 | DB 스키마 + 마이그레이션 | — | A | pass (변경 없음) |
| 2 | Graph 코어 모듈 (rework) | Task 1 | B | rework |
| 3 | CLI 서브커맨드 | Task 1, 2 | C | pass (변경 없음) |
| 4 | Sync 통합 + MCP 확장 (rework) | Task 1, 2, 3 | D | rework |

## Constraints

- raw/sessions/*.md 파일은 읽기만 — 절대 수정하지 않음
- 기존 DB 테이블(sessions, turns, turns_fts, turn_vectors, query_cache) 변경 없음
- 결정적 추출만 (LLM 호출 없음)
- 외부 그래프 라이브러리(petgraph 등) 없이 SQLite + HashMap으로 구현
- Task 1, 3의 통과 코드는 수정하지 않음

## Non-goals

- 시맨틱 추출 (Claude 기반 주제/관계 추출) — 후속 플랜
- vis.js HTML 시각화 — Obsidian graph view로 대체
- Leiden/Louvain 커뮤니티 탐지 알고리즘
- wiki 프롬프트 수정
- graph.html 인터랙티브 뷰 생성
