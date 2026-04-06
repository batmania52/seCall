---
type: plan
status: draft
version: "1.0"
updated_at: 2026-04-06
slug: secall-phase-4
title: "seCall Phase 4 — 검색 고도화 + 인프라 완성"
---

# seCall Phase 4 — 검색 고도화 + 인프라 완성

## Description

Phase 1~3(MVP + Extensions + Wiki) 완료 후 남은 로드맵 항목을 일괄 구현한다:
1. ort ONNX 모델 자동 다운로드 + 업데이트 체크 → Ollama 없이 벡터 검색 즉시 사용
2. OpenAI 임베딩 API embedder → 외부 고품질 임베딩 옵션
3. MCP HTTP transport → 원격 에이전트 접근
4. LLM 쿼리 확장 → Claude Code를 이용한 검색 전처리

## Expected Outcome

- `secall model download` → bge-m3 ONNX 자동 다운로드, 업데이트 체크
- `embedding.backend = "openai"` 설정 시 OpenAI text-embedding-3 사용
- `secall mcp --http :8080` → HTTP 기반 MCP 서버 실행
- `secall recall --expand "쿼리"` → Claude Code로 쿼리 확장 후 검색

## Architecture

```
secall recall --expand "쿼리"
        ↓
   Claude Code (쿼리 확장)     ← Task 04
        ↓ 확장된 키워드
   SearchEngine
   ├── BM25 (lindera/kiwi)
   └── Vector
       ├── OllamaEmbedder    (기존)
       ├── OrtEmbedder        (기존, Task 01이 모델 자동 다운로드)
       └── OpenAIEmbedder     ← Task 02
        ↓
   MCP Server
   ├── stdio   (기존)
   └── HTTP    ← Task 03
```

## Subtasks

1. **ort 모델 자동 다운로드** — `ModelManager` + `secall model` CLI. HuggingFace HTTP.
   - parallel_group: A
   - depends_on: —

2. **OpenAI 임베딩 API embedder** — `OpenAIEmbedder` 구현. Embedder trait 활용.
   - parallel_group: A
   - depends_on: —

3. **MCP HTTP transport** — `secall mcp --http :8080`. rmcp transport-sse feature.
   - parallel_group: A
   - depends_on: —

4. **LLM 쿼리 확장** — `secall recall --expand`. Claude Code subprocess로 쿼리 확장.
   - parallel_group: A
   - depends_on: —

## Dependency Graph

```
Task 01 (ort model)
Task 02 (OpenAI embedder)      ← 모두 독립, 병렬 실행 가능
Task 03 (MCP HTTP)
Task 04 (쿼리 확장)
```

## Constraints

- 기존 검색 동작은 변경하지 않음 (모든 기능은 opt-in)
- OpenAI embedder는 `OPENAI_API_KEY` 미설정 시 graceful skip
- HTTP transport는 localhost 전용 (TLS/인증 없음)
- 쿼리 확장은 Claude Code CLI 의존 (없으면 원본 쿼리 사용)

## Non-goals

- Voyage AI / Cohere embedder (OpenAI만 우선)
- LLM 리랭킹 (쿼리 확장만 우선)
- TLS / 인증 (HTTP transport)
- GUI / TUI
