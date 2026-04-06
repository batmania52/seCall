---
type: task
status: draft
plan: secall-wiki-claude-code
task_number: 4
title: "post-ingest hook 연동"
parallel_group: B
depends_on: [3]
updated_at: 2026-04-06
---

# Task 04: post-ingest hook 연동

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `docs/reference/wiki-setup.md` | **신규 생성** | 설정 가이드 |
| `examples/hooks/wiki-update.sh` | **신규 생성** | hook 예시 스크립트 |

## Change description

### 1. wiki-setup.md 가이드 내용

위키 기능 전체 설정을 한 문서에 정리:

- **사전 요구사항**: Claude Code CLI 설치, MCP 서버 등록 (`~/.claude/settings.json`에 secall), `secall init`
- **수동 실행**: `secall wiki update` 명령어 옵션 설명 (`--model`, `--since`, `--session`, `--dry-run`)
- **자동 실행**: config.toml hook 설정 + hook 스크립트 경로
- **비용 고려**: Sonnet(증분, 일상) vs Opus(배치, 주 1회)

config.toml 예시:
```toml
[hooks]
post_ingest = "~/.config/secall/hooks/wiki-update.sh"
hook_timeout_secs = 300  # 5분 (Opus 기준)
```

### 2. hook 예시 스크립트

`examples/hooks/wiki-update.sh`:
```bash
#!/bin/bash
# seCall post-ingest hook: 새 세션 ingest 후 위키 증분 업데이트
#
# 환경변수 (seCall이 자동 설정):
#   SECALL_SESSION_ID  — 방금 ingest된 세션 ID
#   SECALL_AGENT       — 에이전트 종류 (claude-code, codex, gemini-cli)
#   SECALL_PROJECT     — 프로젝트 이름
#   SECALL_DATE        — 세션 날짜

set -euo pipefail

# Claude Code가 설치되어 있는지 확인
if ! command -v claude &> /dev/null; then
    echo "[wiki-hook] claude not found, skipping wiki update" >&2
    exit 0
fi

# 증분 모드로 위키 업데이트 (Sonnet, 빠름)
secall wiki update --session "$SECALL_SESSION_ID" --model sonnet

echo "[wiki-hook] Wiki updated for session $SECALL_SESSION_ID" >&2
```

### 3. 기존 hook 시스템과의 연동

현재 `hooks/mod.rs`가 이미:
- `post_ingest` 스크립트 경로를 config에서 읽음
- 환경변수 (`SECALL_SESSION_ID` 등)를 설정하여 스크립트 실행
- timeout 지원 (기본 30s)

**코드 변경 없이** hook 스크립트와 config 설정만으로 동작.

## Dependencies

- Task 03 (`secall wiki update` 커맨드 존재해야 함)

## Verification

```bash
# 파일 존재 확인
test -f docs/reference/wiki-setup.md && echo "OK" || echo "FAIL"
test -f examples/hooks/wiki-update.sh && echo "OK" || echo "FAIL"

# hook 스크립트 문법 검증
bash -n examples/hooks/wiki-update.sh && echo "syntax OK" || echo "syntax FAIL"

# Manual: config.toml에 hook 설정 후 secall ingest → hook 트리거 → wiki 갱신 확인
```

## Risks

- **hook timeout**: Opus 사용 시 300초도 부족할 수 있음. config에서 `hook_timeout_secs` 조절 가능하지만 문서에 명시 필요
- **hook 실패 시**: 기존 hooks/mod.rs가 에러를 로그하고 계속 진행하므로 ingest 자체는 영향 없음

## Scope Boundary

- 이 task는 **문서 + 예시 스크립트만** 작성
- seCall 코드 변경 없음 (기존 hook 시스템 활용)
