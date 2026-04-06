---
type: plan
status: draft
updated_at: 2026-04-06
version: 1
---

# seCall P6 — 안정성 + 성능 개선

## Description

P5 GitHub 연동 테스트에서 발견된 결함 2건(ANN capacity 경고, sync pull 실패)과 ingest 성능 병목을 해결.
실사용 안정성 확보가 목적이며, 기능 추가 없이 기존 기능의 품질을 올리는 데 집중.

## Expected Outcome

- `secall sync` 실행 시 ANN "Reserve capacity" 경고 0건
- 이전 sync 실패 후에도 다음 sync가 정상 동작 (pull 전 자동 commit)
- ingest 임베딩 속도 개선 (세션별 순차 → 배치 병렬화)
- `secall sync` 1회 실행이 2분 이하로 완료 (현재 ~5분)

## Subtasks

| # | Title | 공수 | parallel_group | depends_on |
|---|-------|------|---------------|------------|
| 01 | ANN reserve 사전 할당 | Small | A | — |
| 02 | Sync pull 안전성 | Small | A | — |
| 03 | Ingest 임베딩 병렬화 | Medium | A | — |

## Constraints

- P5 완료 상태에서 시작
- 기존 126 테스트 전체 통과
- BM25/벡터 검색 결과 변경 없음 (동작 동일, 성능만 개선)

## Non-goals

- vault index.md/log.md 원자성 — 잔존 리스크이나 데이터 손실 사례 없음, 보류
- 청킹 알고리즘 개선 — 별도 검토
- ort stable 마이그레이션 — stable 미출시
- sqlite-vec 재도입 — macOS arm64 C 컴파일 이슈 미해결
- TUI 대시보드 — P7 이후
