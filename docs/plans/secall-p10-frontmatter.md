---
type: plan
status: draft
updated_at: 2026-04-07
version: 1
---

# seCall P10 — 세션 요약 frontmatter

## Description

세션 MD 파일의 frontmatter에 `summary` 필드를 추가하여, Obsidian 파일 목록과 Dataview 쿼리에서 세션 내용을 즉시 파악할 수 있게 한다. 첫 번째 User 턴의 실질적 첫 줄을 ~80자로 truncate하여 사용하며, LLM 호출 없이 결정적(deterministic)으로 생성한다. 기존 vault MD도 backfill 마이그레이션으로 일괄 적용한다.

## Expected Outcome

- `secall ingest` 실행 시 새 세션 MD에 `summary: "..."` 필드 포함
- `secall migrate summary` 실행 시 기존 vault MD 파일에 summary 필드 backfill
- `secall reindex --from-vault` 시 summary 필드를 DB에 저장
- Dataview에서 `TABLE date, summary, turns` 쿼리 즉시 사용 가능

## Subtasks

| # | Title | depends_on | parallel_group |
|---|-------|------------|----------------|
| 01 | 세션 summary frontmatter 추가 | — | — |
| 02 | 기존 세션 summary backfill | 01 | — |

## Constraints

- summary 생성에 LLM을 사용하지 않음
- YAML 안전한 문자열 이스케이프 필수 (`:`, `#`, `"`, `\n` 등)
- backfill 시 기존 MD의 본문은 절대 변경하지 않음 (frontmatter 영역만 수정)
- UTF-8 safe truncation 사용

## Non-goals

- summary 기반 검색 가중치 조정
- wiki 파일에 summary 반영
- summary를 MCP 도구 응답에 포함
- 파일명 변경 (파일명은 기존 ID 기반 유지)
