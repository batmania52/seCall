# Review Report: seCall Wiki — Claude Code 메타에이전트 기반 위키 생성 — Round 2

> Verdict: fail
> Reviewer: 
> Date: 2026-04-06 08:40
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall/src/commands/wiki.rs:46 — `claude` subprocess를 실행할 때 vault 경로로 `current_dir`를 설정하지 않아, 사용자가 vault 루트가 아닌 곳에서 `secall wiki update`를 실행하면 메타에이전트가 `SCHEMA.md`와 `wiki/`를 현재 쉘 디렉토리 기준으로 읽고 써서 잘못된 위치를 수정할 수 있습니다.
2. crates/secall-core/src/ingest/lint.rs:263 — L008이 frontmatter에서 `title:`만 검사하고 `type:` 및 `sources:` 누락을 검출하지 않습니다. task 문서상 세 필드가 모두 필수이므로, `sources` 없는 wiki 페이지가 lint를 통과하는 기능 누락 결함입니다.

## Recommendations

1. Task 03 결과 보고에는 `wiki status`와 task 문서에 적힌 전체 회귀 검증 항목도 함께 남겨 두면 후속 리뷰가 더 명확해집니다.
2. Task 05는 `frontmatter exists`와 `required keys present`를 별도 테스트 케이스로 나누면 회귀 방지가 쉬워집니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | Wiki Vault 구조 초기화 | ✅ done |
| 2 | 메타에이전트 프롬프트 설계 | ✅ done |
| 3 | secall wiki CLI 커맨드 | ✅ done |
| 4 | post-ingest hook 연동 | ✅ done |
| 5 | 위키 품질 검증 (secall lint 확장) | ✅ done |

