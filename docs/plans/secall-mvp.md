---
type: plan
status: draft
updated_at: 2026-04-05
version: 2.0
---

# seCall MVP — 에이전트 세션 검색 인프라

## Description

터미널 에이전트(Claude Code, Codex, Gemini CLI)의 세션 로그를 파싱하여 Obsidian vault에 마크다운으로 저장하고, 한국어 형태소 분석 BM25 + 벡터 하이브리드 검색을 제공하는 로컬 CLI 도구. MCP 서버로 에이전트가 직접 도구 호출 가능. LLM Wiki 패턴의 인프라 레이어.

## Expected Outcome

1. `secall ingest <session>` — Claude Code JSONL → Obsidian MD + 인덱싱
2. `secall recall "query"` — 한국어 하이브리드 검색으로 과거 세션 검색
3. `secall mcp` — 에이전트가 MCP 도구로 `recall`, `get`, `status` 호출
4. Obsidian에서 세션 기록을 browsing 가능 (frontmatter, 링크, graph view)

## Architecture

```
┌─────────────────────────────────────────────────┐
│  secall (Rust, single binary)                   │
│                                                 │
│  ┌──────────┐  ┌──────────┐  ┌───────────────┐ │
│  │ ingest/  │  │ search/  │  │ mcp/          │ │
│  │ parser   │→ │ bm25     │← │ server (stdio │ │
│  │ markdown │  │ vector   │  │   + http)     │ │
│  │ indexer  │  │ hybrid   │  └───────────────┘ │
│  └──────────┘  └──────────┘                     │
│       ↓              ↑                          │
│  ┌──────────────────────────────────────┐       │
│  │ store/ — SQLite (FTS5 + sqlite-vec) │       │
│  └──────────────────────────────────────┘       │
│       ↓                                         │
│  ┌──────────────────────────────────────┐       │
│  │ Obsidian Vault (raw/sessions/*.md)   │       │
│  └──────────────────────────────────────┘       │
└─────────────────────────────────────────────────┘
         ↕ MCP                    ↕ hook trigger
   ┌───────────┐           ┌──────────────┐
   │ LLM Agent │           │ wiki updater │
   │ (외부)    │           │ (에이전트)   │
   └───────────┘           └──────────────┘
```

## Vault Structure (LLM Wiki 3-Layer)

```
vault/
├── SCHEMA.md                 # 위키 구조 컨벤션
├── index.md                  # 위키 페이지 카탈로그
├── log.md                    # append-only 연대기
├── raw/sessions/             # seCall이 생성 (immutable)
│   └── YYYY-MM-DD/
│       └── <agent>_<project>_<id>.md
└── wiki/                     # 에이전트가 유지보수
    ├── projects/
    ├── topics/
    ├── decisions/
    └── overview.md
```

## Tech Stack

| Layer | Choice | Rationale |
|---|---|---|
| Language | Rust 2021, MSRV 1.75+ | Single binary, performance |
| CLI | clap v4 | Standard |
| DB | rusqlite v0.39 + FTS5 | Proven by qmd, sqlite-vec 0.1.9 compatible |
| Vector | sqlite-vec (rusqlite extension) | Single SQLite file |
| Tokenizer | lindera v2.3.4 (embed-ko-dic) | Pure Rust, MIT, ~1M downloads |
| Embedding | Ollama API (bge-m3) | Optional, multilingual |
| MCP | rmcp v1.3.0 | `#[tool_router]` + `#[tool_handler]` macros, stdio |
| Serialization | serde + serde_json | Standard |
| Async | tokio | Ollama HTTP, MCP server |

## Subtask Summary

### Phase 0: Project Bootstrap (parallel_group: 0)

| Task | Title | Depends |
|---|---|---|
| 01 | Rust workspace 초기화 | — |
| 02 | SQLite 스키마 설계 + 초기화 | — |

### Phase 1: Ingest Pipeline (parallel_group: 1)

| Task | Title | Depends |
|---|---|---|
| 03 | Claude Code JSONL 파서 | 01 |
| 04 | Markdown 렌더러 | 03 |
| 05 | Vault 구조 초기화 + index/log 관리 | 04 |

### Phase 2: Search Engine (parallel_group: 2)

| Task | Title | Depends |
|---|---|---|
| 06 | 한국어 BM25 인덱서 | 02, 03 |
| 07 | 벡터 인덱서 + 검색 | 02, 03 |
| 08 | 하이브리드 검색 (RRF) | 06, 07 |

### Phase 3: MCP + CLI (parallel_group: 3)

| Task | Title | Depends |
|---|---|---|
| 09 | CLI 완성 | 08, 05 |
| 10 | MCP 서버 | 08 |
| 11 | Ingest 완료 이벤트 + hook | 09 |

## Dependency Graph

```
T01 ──┬──→ T03 ──→ T04 ──→ T05 ──┐
      │      │                     ├──→ T09 ──→ T11
T02 ──┼──→ T06 ──┐                │
      │           ├──→ T08 ───────┘
      └──→ T07 ──┘        │
                           └──→ T10
```

## Constraints

- Single binary deployment (workspace: `secall-core` lib + `secall` bin)
- Single SQLite file — no external DB
- Ollama optional — BM25-only fallback when unavailable
- lindera + ko-dic — pure Rust, `embed-ko-dic` feature for dictionary bundling
- LLM Wiki principle — seCall is infrastructure only; wiki content is agent's responsibility
- Vault path configurable via `~/.config/secall/config.toml`
- Korean + English mixed text support

## Non-goals

- Wiki page auto-generation/summarization (agent's job)
- LLM reranking / query expansion (post-MVP)
- Real-time streaming ingest (batch only)
- GUI / TUI (CLI + MCP only)
- candle embedding internalization (ort ONNX in Phase 4)
- Multi-user / remote server (local single-user only)

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| lindera ko-dic dictionary outdated (2018) | High | Medium | Tokenizer trait for kiwi-rs swap |
| sqlite-vec Rust bindings immature | Medium | High | BM25-only fallback, rusqlite loadable extension |
| Claude Code JSONL format changes | Medium | Medium | Version field check, parser trait isolation |
| Ollama not installed | High | Low | Graceful degradation by design |
| rmcp crate immature | Medium | Medium | stdio JSON-RPC is simple, can implement directly |
