---
type: reference
status: in_progress
updated_at: 2026-04-06
---

# seCall Roadmap

## Vision

에이전트(Claude Code, Codex, Gemini CLI)와의 대화를 수집 → 검색 → 지식 위키로 자동 구성하는 개인 지식 관리 시스템.

```
Phase 1: 수집 인프라    → Phase 2: 확장         → Phase 3: 위키 생성
(MVP)                    (Extensions)              (Wiki)

세션 로그 파싱           멀티에이전트 파서         세션 클러스터링
  ↓                       ↓                         ↓
SQLite + Vault           로컬 NLP (ort, kiwi)      주제 추출
  ↓                       ↓                         ↓
BM25 + 벡터 검색         Embedder/Tokenizer trait   위키 페이지 생성
  ↓                       ↓                         ↓
MCP 서버 + CLI           secall lint                Obsidian wiki/ 유지보수
```

## Phases

### Phase 1: MVP — 에이전트 세션 검색 인프라 ✅

- Plan: `docs/plans/secall-mvp.md` (Task 01~11)
- 상태: **완료**
- 내용:
  - Rust workspace (secall-core lib + secall bin)
  - Claude Code JSONL 파서
  - SQLite FTS5 + 벡터 BLOB 스토리지
  - 한국어 BM25 (lindera) + Ollama 벡터 + RRF 하이브리드 검색
  - Obsidian Vault MD 렌더링
  - CLI 8개 커맨드 (init, ingest, recall, get, status, embed, lint, mcp)
  - MCP 서버 (stdio)
  - Post-ingest hook

### Phase 2: Extensions — 멀티에이전트 + 로컬 NLP ✅

- Plan: `docs/plans/secall-extensions-nlp.md` (Task 01~05)
- 상태: **완료** (Review PASS)
- 내용:
  - Codex CLI 파서 (JSONL rollout, call_id 매칭)
  - Gemini CLI 파서 (JSON, functionResponse.name 매칭)
  - ort ONNX 로컬 임베딩 (Embedder trait + OrtEmbedder)
  - kiwi-rs 토크나이저 (Tokenizer trait + KiwiTokenizer)
  - secall lint (L001~L007)
  - Config 기반 backend 선택 (embedding, tokenizer)

### Phase 3: Wiki — Claude Code 메타에이전트 기반 위키 생성 📋

- Plan: `docs/plans/secall-wiki.md` (Task 01~06)
- 상태: **플랜 승격 완료, 구현 대기**
- 내용:
  - ort ONNX 모델 자동 다운로드 + 업데이트 체크 (`secall model download`)
  - Wiki Vault 구조 초기화 (wiki/, SCHEMA.md)
  - Claude Code (Opus/Sonnet)를 메타에이전트로 활용
  - 세션 클러스터링 + 주제 추출 프롬프트
  - `secall wiki update` CLI + post-ingest hook
  - wiki 품질 검증 lint L008~L010

### Phase 4: 검색 고도화 + 인프라 완성 📋

- Plan: `docs/plans/secall-phase-4.md` (Task 01~04)
- 상태: **플랜 승격 완료, 구현 대기**
- 내용:
  - ort ONNX 모델 자동 다운로드 + 업데이트 체크 (`secall model download`)
  - OpenAI text-embedding-3 embedder (Embedder trait 활용)
  - MCP HTTP/SSE transport (`secall mcp --http :8080`)
  - LLM 쿼리 확장 (`secall recall --expand`, Claude Code Haiku)

### 미래 (미정)

- Voyage AI / Cohere embedder
- LLM 리랭킹
- 위키 품질 피드백 루프
- TLS / 인증 (MCP HTTP)

## Architecture (최종)

```
에이전트 세션 로그 (.jsonl/.json)
        ↓ secall ingest --auto
   ┌────┴────┐
   │         │
SQLite DB    Obsidian Vault
(검색용)     ├── raw/sessions/  (seCall 생성, immutable)
   ↑         └── wiki/          (메타에이전트 생성)
   │                ↑
   │                │
secall mcp ←→ Claude Code (메타에이전트)
   │           - secall recall로 검색
   │           - 주제 클러스터링
   │           - wiki/ 페이지 생성/갱신
   │
   ↕
에이전트들 (Claude Code, Codex, Gemini CLI)
   - MCP로 recall/get/status 호출
   - wiki/ 페이지도 참조 가능
```

## Tech Stack

| Layer | Choice | Status |
|---|---|---|
| Core | Rust 2021, single binary | ✅ |
| DB | rusqlite 0.31 + FTS5 | ✅ |
| Tokenizer | lindera 2.3.4 / kiwi-rs 0.1.4 | ✅ |
| Embedding | Ollama bge-m3 / ort ONNX | ✅ |
| MCP | rmcp 1.3.0 (stdio) | ✅ |
| 메타에이전트 | Claude Code (Opus/Sonnet) | 📋 Phase 3 |
| Vault | Obsidian (raw/ + wiki/) | raw/ ✅, wiki/ 📋 |
