---
type: plan
status: draft
version: "1.0"
updated_at: 2026-04-06
slug: secall-extensions-nlp
title: "seCall Extensions — 멀티에이전트 + 로컬 NLP"
---

# seCall Extensions — 멀티에이전트 + 로컬 NLP

## Description

MVP(Task 01~11)는 Claude Code 전용 파서 + Ollama 의존 임베딩으로 동작한다.
이 플랜은 두 축을 확장한다:

1. **멀티에이전트 파서**: Codex CLI, Gemini CLI 세션 로그를 파싱하여 통합 검색 지원
2. **로컬 NLP**: ort ONNX 임베딩 + kiwi-rs 토크나이저로 Ollama/lindera 의존 없이 동작
3. **운영 도구**: `secall lint`로 세션-DB-Vault 무결성 검증

## Expected Outcome

- `secall ingest --auto`가 Claude Code, Codex CLI, Gemini CLI 세션을 모두 인식하고 파싱
- Ollama 없이도 벡터 검색 동작 (ort bge-m3 ONNX)
- kiwi-rs 기반 한국어 형태소 분석이 lindera 대비 더 높은 정확도 제공
- `secall lint`로 인덱스 무결성 검증 가능
- 기존 Ollama/lindera는 fallback으로 유지 (breaking change 없음)

## Architecture

```
SessionParser trait (기존)
├── ClaudeCodeParser  (기존, ingest/claude.rs)
├── CodexParser       (신규, ingest/codex.rs)     ← Task 01
└── GeminiParser      (신규, ingest/gemini.rs)    ← Task 02

Embedder trait (신규 추출)
├── OllamaEmbedder    (기존 리팩터, search/embedding.rs)
└── OrtEmbedder       (신규, search/embedding.rs) ← Task 03

Tokenizer trait (기존)
├── LinderaKoTokenizer (기존, search/tokenizer.rs)
├── SimpleTokenizer    (기존, search/tokenizer.rs)
└── KiwiTokenizer      (신규, search/tokenizer.rs) ← Task 04
```

## Subtasks

1. **Codex CLI 파서** — `ingest/codex.rs` 신규. `SessionParser` trait 구현. Codex JSONL rollout 파싱.
   - parallel_group: A
   - depends_on: —

2. **Gemini CLI 파서** — `ingest/gemini.rs` 신규. `SessionParser` trait 구현. Gemini JSON 파싱.
   - parallel_group: A
   - depends_on: —

3. **ort ONNX 로컬 임베딩** — `Embedder` trait 추출 + `OrtEmbedder` 구현. config 기반 backend 선택.
   - parallel_group: B
   - depends_on: —

4. **kiwi-rs 토크나이저** — `KiwiTokenizer` 추가. config 기반 tokenizer 선택. BM25 FTS5 연동.
   - parallel_group: B
   - depends_on: —

5. **secall lint** — `secall lint` CLI 커맨드. 세션-DB-Vault 무결성 검증.
   - parallel_group: C
   - depends_on: 01, 02, 03, 04

## Dependency Graph

```
Task 01 (Codex) ──┐
Task 02 (Gemini) ─┤
                   ├── Task 05 (lint)
Task 03 (ort) ────┤
Task 04 (kiwi) ──┘
```

## Constraints

- 기존 ClaudeCodeParser, OllamaEmbedder, LinderaKoTokenizer는 제거하지 않음
- AgentKind enum에 Codex, GeminiCli 이미 존재 (types.rs:7-11) — 활용
- config.toml에 embedding.backend, search.tokenizer 필드 추가
- ort ONNX 모델 파일은 `~/.cache/secall/models/` 에 자동 다운로드 or 수동 배치

## Non-goals

- MCP HTTP transport (별도 플랜)
- temporal 자연어 파싱 고도화
- GUI/TUI
- 자동 모델 fine-tuning
- Wiki auto-generation
