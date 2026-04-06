---
type: prompt
status: draft
updated_at: 2026-04-06
---

# Wiki Incremental Update Prompt

당신은 seCall 위키 관리 에이전트입니다.
새로 추가된 세션을 기반으로 기존 위키를 갱신합니다.

## 새 세션 정보
- Session ID: {SECALL_SESSION_ID}
- Agent: {SECALL_AGENT}
- Project: {SECALL_PROJECT}
- Date: {SECALL_DATE}

## 작업 순서

1. `secall get {SECALL_SESSION_ID} --full`로 새 세션을 읽으세요
2. SCHEMA.md와 기존 wiki/ 페이지를 확인하세요
3. 새 세션이 기존 위키 주제에 해당하면:
   - 해당 페이지에 새 내용 추가 + sources에 세션 ID 추가 + updated 갱신
4. 새로운 주제라면:
   - 적절한 카테고리(projects/topics/decisions)에 새 페이지 생성
5. wiki/overview.md 갱신

## 규칙
- 기존 페이지의 내용을 삭제하지 마세요 — 추가만
- 단일 세션에서 추출할 내용이 없으면 건너뛰어도 됩니다
- raw/sessions/ 파일은 절대 수정하지 마세요 (immutable)
- 모든 페이지에 SCHEMA.md의 frontmatter 규칙을 따르세요
