# Reference

Reference document index.

## Documents

- [roadmap.md](roadmap.md) — seCall 전체 로드맵 (Phase 1~4, 아키텍처, 기술 스택)
- [adr-blocking-io-in-async.md](adr-blocking-io-in-async.md) — ADR: async 내 blocking I/O는 spawn_blocking으로 래핑 (CLI 특성상 정당)
- [idea-two-tier-llm-pipeline.md](idea-two-tier-llm-pipeline.md) — 아이디어: 2계층 LLM 파이프라인 (저비용 초안 → 고품질 검수)

## CLI Reference

### `secall graph semantic`

시맨틱 그래프 엣지 재추출 (임베딩 미포함).

| 플래그 | 설명 | 기본값 |
|--------|------|--------|
| `--delay <SECS>` | 세션 간 대기 시간 (소수점 가능) | 2.5 |
| `--limit <N>` | 최대 처리 세션 수 | 전체 |
| `--backend <NAME>` | LLM 백엔드 (`ollama`/`gemini`/`anthropic`/`disabled`) | config.toml |
| `--api-url <URL>` | API base URL (Ollama 전용) | config.toml |
| `--model <NAME>` | 모델명 (예: `gemma4:e4b`, `gemini-2.5-flash`) | config.toml |
| `--api-key <KEY>` | API 키 (Gemini 등). 환경변수 사용 권장 | config.toml |

**환경변수** (우선순위: CLI 플래그 > 환경변수 > config.toml > 기본값):

| 환경변수 | 용도 | 예시 값 |
|----------|------|---------|
| `SECALL_GRAPH_BACKEND` | 시맨틱 백엔드 | `gemini`, `ollama`, `disabled` |
| `SECALL_GRAPH_API_URL` | API base URL (Ollama용) | `http://localhost:11434` |
| `SECALL_GRAPH_MODEL` | 모델명 | `gemma4:e4b`, `gemini-2.5-flash` |
| `SECALL_GRAPH_API_KEY` | API 키 | `AIza...` |

> **참고**: `SECALL_GEMINI_API_KEY`(기존)와 `SECALL_GRAPH_API_KEY`(신규)가 모두 설정된 경우, `SECALL_GRAPH_API_KEY`가 우선 적용됩니다.
