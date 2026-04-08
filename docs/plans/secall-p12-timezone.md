---
type: plan
status: draft
updated_at: 2026-04-08
version: 1
---

# seCall 타임존 설정 — 렌더링 시간대 변환

## Description

세션 마크다운 렌더링 시 모든 타임스탬프가 UTC로 출력되어 한국(KST) 등 비-UTC 타임존 사용자에게 불편한 문제를 해결한다.
`config.toml`에 `timezone` 설정을 추가하여 IANA 타임존 이름(예: `Asia/Seoul`)으로 렌더링 시점에 변환한다.

- **내부 데이터(DB, 원본 JSON)는 UTC 유지** — 렌더링 레이어에서만 변환
- 기본값 `UTC` — 하위호환 완전 유지
- GitHub Issue: hang-in/seCall#4

## Expected Outcome

- `config.toml`에 `timezone = "Asia/Seoul"` 설정 시 vault MD의 모든 타임스탬프가 KST로 렌더링
- vault 디렉토리 경로(`raw/sessions/YYYY-MM-DD/`)도 설정된 타임존 기준 날짜
- 14개 포맷 위치(6개 파일)에 일관 적용
- 기존 동작(UTC)과 100% 호환

## Subtasks

| # | Title | depends_on | parallel_group |
|---|-------|-----------|----------------|
| 01 | 의존성 + Config 구조체 | — | A |
| 02 | 마크다운 렌더링 타임존 적용 | 01 | B |
| 03 | 보조 렌더링 위치 적용 | 01 | B |
| 04 | 테스트 + 문서 | 02, 03 | C |

> Task 02, 03은 병렬 실행 가능 (parallel_group B).

## Constraints

- DB 저장 값은 항상 UTC (렌더링 전용 변환)
- IANA 타임존 이름만 지원 (`KST`, `EST` 등 약어 불가)
- `chrono-tz` 크레이트 사용 — TZ 데이터베이스 임베딩

## Non-goals

- DB 쿼리/필터의 타임존 변환 (검색은 UTC 기준 유지)
- 사용자별 다른 타임존 (단일 전역 설정)
- 기존 vault 파일 자동 마이그레이션 (수동 `secall ingest --force` 사용)
- MCP 서버 응답의 타임스탬프 변환
