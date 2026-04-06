---
type: plan
status: draft
version: "1.0"
updated_at: 2026-04-06
slug: secall-wiki-claude-code
title: "seCall Wiki — Claude Code 메타에이전트 기반 위키 생성"
---

# seCall Wiki — Claude Code 메타에이전트 기반 위키 생성

## Description

축적된 에이전트 세션 로그를 Claude Code(Opus/Sonnet)가 메타에이전트로서 분석하여,
Obsidian Vault의 `wiki/` 디렉토리에 주제별 지식 페이지를 자동 생성·유지보수한다.

seCall은 **트리거 + 검증** 역할만 수행하고, 실제 위키 콘텐츠 생성은 Claude Code가 MCP를 통해 seCall을 검색하며 수행한다.

## Expected Outcome

1. `secall wiki update` → Claude Code가 최근 세션을 분석하여 wiki/ 페이지 생성/갱신
2. post-ingest hook으로 자동 위키 갱신 가능
3. `wiki/` 아래에 주제별(projects/, topics/, decisions/) 마크다운 생성
4. 기존 세션 → 위키 페이지 간 Obsidian 양방향 링크
5. secall lint L008~L010으로 위키 품질 검증

## Architecture

```
secall ingest --auto
        ↓
   세션 DB 축적
        ↓ post-ingest hook (optional)
   secall wiki update [--since DATE] [--model opus|sonnet]
        ↓
   Claude Code (터미널 에이전트)
   ├── secall MCP recall → 관련 세션 검색
   ├── secall MCP get → 세션 상세 조회
   ├── 세션 클러스터링 + 주제 추출
   └── wiki/ 마크다운 생성/갱신
        ↓
   Obsidian Vault
   ├── raw/sessions/  (기존, immutable)
   └── wiki/          (메타에이전트 생성)
       ├── projects/
       ├── topics/
       ├── decisions/
       └── overview.md
```

## Subtasks

1. **Wiki Vault 구조 초기화** — `secall init` 확장. wiki/ 디렉토리 + SCHEMA.md 자동 생성.
   - parallel_group: A
   - depends_on: —

2. **메타에이전트 프롬프트 설계** — wiki-update 프롬프트. Claude Code용 시스템 프롬프트 + MCP 도구 사용법 + 위키 컨벤션.
   - parallel_group: A
   - depends_on: —

3. **secall wiki CLI 커맨드** — `secall wiki update` 서브커맨드. Claude Code를 subprocess로 실행.
   - parallel_group: B
   - depends_on: 01, 02

4. **post-ingest hook 연동** — hook 설정 가이드 + 예시 스크립트.
   - parallel_group: B
   - depends_on: 03

5. **위키 품질 검증 (secall lint 확장)** — L008~L010 추가.
   - parallel_group: B
   - depends_on: 01

## Dependency Graph

```
Task 01 (vault init) ──┬── Task 03 (wiki CLI) ── Task 04 (hook)
Task 02 (prompt) ──────┘       │
                               └── Task 05 (lint)
```

## Constraints

- Claude Code CLI(`claude`)가 PATH에 설치되어 있어야 함
- MCP 서버 설정(`~/.claude/settings.json`에 secall 등록) 선행 필요
- wiki/ 콘텐츠는 seCall이 아니라 Claude Code가 생성 — seCall은 트리거 + 검증만
- Opus(고품질, 느림) 또는 Sonnet(빠름, 저렴)으로 모델 선택 가능

## Non-goals

- seCall 코드 내에서 직접 LLM API 호출 (Claude Code에 위임)
- 실시간 스트리밍 위키 갱신
- 위키 편집 UI
- 다국어 위키 (한국어 우선)
- ort 모델 자동 다운로드 (별도 플랜)
