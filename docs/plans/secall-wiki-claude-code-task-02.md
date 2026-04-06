---
type: task
status: draft
plan: secall-wiki-claude-code
task_number: 2
title: "메타에이전트 프롬프트 설계"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 02: 메타에이전트 프롬프트 설계

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `docs/prompts/wiki-update.md` | **신규 생성** | Claude Code 메타에이전트 배치 모드 프롬프트 |
| `docs/prompts/wiki-incremental.md` | **신규 생성** | Claude Code 메타에이전트 증분 모드 프롬프트 |

## Change description

### 1. 프롬프트 설계 원칙

- Claude Code는 MCP 도구(`secall recall`, `secall get`, `secall status`)를 사용 가능
- 파일 시스템 접근 가능 — wiki/ 디렉토리에 직접 MD 파일 생성/수정
- SCHEMA.md를 먼저 읽어서 컨벤션을 파악하도록 지시
- 배치 모드(전체 재생성)와 증분 모드(새 세션만) 2가지

### 2. wiki-update.md (배치 모드)

```markdown
# Wiki Update Prompt

당신은 seCall 위키 관리 에이전트입니다.

## 역할
에이전트 세션 로그를 분석하여 Obsidian 위키 페이지를 생성·갱신합니다.

## 사용 가능한 도구
- `secall recall "쿼리" --limit 50` — 세션 검색
- `secall get <session-id> --full` — 세션 전문 조회
- `secall status` — 인덱스 현황

## 작업 순서

1. SCHEMA.md를 읽어 위키 컨벤션을 확인하세요
2. `secall status`로 전체 세션 현황을 파악하세요
3. `secall recall`로 주제별 세션을 검색하세요
4. 세션들을 주제별로 클러스터링하세요:
   - 프로젝트별 (project 필드 기준)
   - 기술 주제별 (반복 등장하는 키워드/개념)
   - 의사결정 (아키텍처 선택, 기술 결정)
5. 각 클러스터에 대해 wiki/ 페이지를 생성하세요:
   - wiki/projects/{project-name}.md
   - wiki/topics/{topic-name}.md
   - wiki/decisions/{date-decision-name}.md
6. wiki/overview.md를 갱신하세요
7. 모든 페이지에 SCHEMA.md의 frontmatter 규칙을 따르세요
8. sources 배열에 참조한 세션 ID를 반드시 포함하세요

## 페이지 작성 규칙

- 한국어로 작성
- 코드, 경로, 명령어는 원문 유지
- 핵심 인사이트 중심 — 세션 전문 복사 금지
- Obsidian 링크([[]])로 세션과 다른 위키 페이지 연결
- 이미 존재하는 페이지는 내용을 보강하고 updated 날짜를 갱신

## 주의사항

- raw/sessions/ 파일은 절대 수정하지 마세요 (immutable)
- 확실하지 않은 정보는 추측하지 말고, 세션에 근거가 있는 내용만 작성하세요
- 페이지가 너무 길어지면 하위 주제로 분리하세요
```

### 3. wiki-incremental.md (증분 모드)

```markdown
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
```

### 4. 프롬프트 환경변수 치환

`secall wiki update`가 프롬프트를 로드할 때, `{SECALL_*}` 플레이스홀더를 실제 값으로 치환.
증분 모드에서 hook 환경변수(`SECALL_SESSION_ID`, `SECALL_AGENT` 등)를 주입.

## Dependencies

- 없음 (Task 01과 병렬 실행 가능)
- SCHEMA.md는 Task 01에서 생성하지만, 프롬프트 문서 자체는 독립

## Verification

```bash
# 파일 존재 확인
test -f docs/prompts/wiki-update.md && echo "OK" || echo "FAIL"
test -f docs/prompts/wiki-incremental.md && echo "OK" || echo "FAIL"

# Manual: 프롬프트가 SCHEMA.md 참조를 포함하는지 확인
# Manual: MCP 도구 사용 예시가 올바른지 확인
# Manual: Claude Code에 프롬프트를 넣어 실제 동작 테스트
```

## Risks

- **프롬프트 품질**: Claude Code의 출력 품질이 프롬프트에 크게 의존. 반복 테스트 + 개선 필요
- **토큰 제한**: 세션이 많으면 recall 결과가 길어져 컨텍스트 초과 가능. `--limit`으로 제어
- **Opus vs Sonnet**: Opus는 품질 좋지만 느림. Sonnet은 빠르지만 복잡한 클러스터링 약할 수 있음

## Scope Boundary

- 이 task는 **문서(프롬프트) 작성만** 수행
- 코드 변경 없음
- CLI 구현은 Task 03
