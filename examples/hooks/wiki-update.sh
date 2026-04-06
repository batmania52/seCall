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
