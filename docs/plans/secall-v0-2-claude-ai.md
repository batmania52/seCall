---
type: plan
status: draft
updated_at: 2026-04-07
version: 1
---

# seCall v0.2 — claude.ai 파서 + 버전 명기

## Description

claude.ai 공식 export JSON 파싱 지원 추가 및 프로젝트 버전을 v0.2.0으로 bump.
기존 `SessionParser` trait에 1:N 파싱(`parse_all`)을 추가하고, `ClaudeAiParser`를 구현하여 ZIP 내 `conversations.json`에서 대화를 자동 추출.

실제 export 데이터 분석 결과 (22개 대화, 최대 946 메시지):
- `parent_message_uuid` 없음 (선형 구조) → 트리 변환 불필요
- `model`, `settings`, `project_uuid` 전부 없음
- 도구: `web_search`, `web_fetch`, `conversation_search`, `view`
- content block: `text`, `tool_use`, `tool_result`

## Expected Outcome

- `secall ingest <export.zip>` 또는 `secall ingest <conversations.json>`으로 claude.ai 대화 ingest 가능
- ZIP 파일 전달 시 자동 해제 → `conversations.json` 추출 → 파싱
- `secall recall "X" --agent claude-ai`로 claude.ai 대화만 필터 검색
- vault에 `raw/sessions/YYYY-MM-DD/claude-ai_대화제목_uuid.md` 형태로 저장
- `Cargo.toml` version `0.2.0`, CHANGELOG.md 생성

## Subtasks

| # | Title | 공수 | parallel_group | depends_on |
|---|-------|------|---------------|------------|
| 01 | 버전 bump + CHANGELOG | Small | A | — |
| 02 | AgentKind 확장 + SessionParser trait 1:N 지원 | Small | A | — |
| 03 | ClaudeAiParser 구현 (ZIP 자동해제 포함) | Medium | B | 02 |
| 04 | detect.rs 연동 + CLI 통합 | Small | B | 02, 03 |

## Constraints

- P6 완료 상태에서 시작
- 기존 테스트 전체 통과
- 기존 3개 파서 동작 변경 없음
- ZIP 내 `conversations.json`만 파싱

## Non-goals

- Chrome 확장 포맷 별도 지원 — 공식 export와 구조 동일
- 첨부파일/이미지 바이너리 처리 — `extracted_content` 텍스트만 인덱싱
- claude.ai 대화 브랜치 복원 — 실제 export에 `parent_message_uuid` 없음
- memories.json, projects.json, users.json 파싱 — 별도 플랜
- `secall ingest --auto`에 claude.ai 자동 탐지 — 경로 패턴 없음, 명시적 경로 필요
