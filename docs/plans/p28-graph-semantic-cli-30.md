---
type: plan
status: draft
updated_at: 2026-04-16
issue: "#30"
version: 1
---

# P28 — Graph Semantic CLI 플래그 확장 (#30)

## Description

`secall graph semantic` 실행 시 LLM 백엔드, API URL, 모델, API 키를 CLI 플래그로 지정할 수 있게 확장한다.
현재는 `secall.toml` 설정 파일을 직접 수정해야만 변경 가능하며, 이는 CI/CD, 임시 테스트, 다중 백엔드 비교 등의 시나리오에서 불편하다.

**우선순위**: CLI 플래그 > 환경변수 > config.toml > 하드코딩 기본값

## Expected Outcome

- `secall graph semantic --backend gemini --api-key <KEY>` 형태로 일회성 오버라이드 가능
- `SECALL_GRAPH_BACKEND`, `SECALL_GRAPH_API_URL`, `SECALL_GRAPH_MODEL`, `SECALL_GRAPH_API_KEY` 환경변수 지원
- 기존 config.toml 동작은 변경 없음 (하위 호환)

## Subtask Summary

| # | Title | parallel_group | depends_on |
|---|-------|---------------|------------|
| 01 | CLI 플래그 추가 (`main.rs`) | A | — |
| 02 | GraphConfig 오버라이드 로직 (`main.rs` + `commands/graph.rs`) | — | 01 |
| 03 | 환경변수 fallback (`config.rs`) | A | — |
| 04 | 테스트 및 문서 | — | 01, 02, 03 |

## Constraints

- 기존 `--delay`, `--limit` 플래그 동작 변경 없음
- `GraphConfig` struct 필드 추가 없음 (기존 필드를 CLI/환경변수로 오버라이드)
- 환경변수명은 기존 `SECALL_` prefix 패턴 준수

## Non-goals

- `graph build`, `graph stats`, `graph export` 서브커맨드에 플래그 추가
- config.toml 자동 생성/수정 기능
- 새로운 LLM 백엔드 추가
