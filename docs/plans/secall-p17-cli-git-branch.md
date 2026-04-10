---
type: plan
status: draft
version: 1.0
updated_at: 2026-04-10
---

# seCall P17 — 대화형 온보딩 + 설정 CLI + git branch 수정

## Description

새 사용자가 `secall init`만 실행하면 대화형 프롬프트로 모든 설정을 완료할 수 있도록 개선한다.
이후 설정 변경도 `secall config` 커맨드로 처리하여 사용자가 `.toml` 파일을 직접 편집할 필요가 없도록 한다.
추가로 `vault/git.rs`의 "main" 브랜치 하드코딩 문제를 수정한다.

## Motivation

현재 문제:

1. **git branch 하드코딩**: `git.rs`의 `pull`, `push`, `init`에서 "main"이 하드코딩되어, `master` 또는 커스텀 브랜치 사용자는 sync 실패
2. **설정 변경이 불편**: config.toml을 직접 편집해야 하므로 키 이름·구조를 알아야 함
3. **초기 설정 미흡**: `secall init`이 비대화형이라 사용자가 옵션을 미리 알아야 하고, Ollama 설치·모델 pull 등의 사전 조건을 안내하지 않음

## Expected Outcome

- `secall init` 인자 없이 실행 → 대화형 위저드로 vault 경로, 토크나이저, 임베딩 백엔드, git branch 설정
- Ollama 선택 시 → 설치 여부 확인 → 미설치면 안내 → 설치되어 있으면 `ollama pull bge-m3` 자동 실행
- `secall config show` → 현재 설정 확인
- `secall config set <key> <value>` → CLI로 설정 변경
- git 연동 시 config에 설정된 branch 이름 사용

## Subtask Summary

| # | Title | Group | Depends | Status |
|---|-------|-------|---------|--------|
| 01 | git branch 하드코딩 제거 | A | — | draft |
| 02 | `secall config` 서브커맨드 | B | 01 | draft |
| 03 | 대화형 온보딩 (`secall init` 개선) | C | 01, 02 | draft |
| 04 | status 설정 요약 표시 | D | 02 | draft |

## Constraints

- 기존 `secall init --vault /path` 동작 유지 (non-interactive)
- Windows에서 kiwi 옵션 비표시
- `secall config set`은 유효성 검증 후 저장

## Non-goals

- ORT 런타임 자동 설치
- Ollama 자동 설치 (안내만 출력)
- GUI/TUI 인터페이스
- config.toml 직접 편집 방지 (잠금 등) — CLI 제공으로 충분
