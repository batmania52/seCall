---
type: plan
status: in_progress
updated_at: 2026-04-15
---

# P26 — Gemini API 백엔드 추가 (시맨틱 그래프 + Log 일기)

## Description

시맨틱 그래프 추출(`graph/semantic.rs`)과 Log 일기 생성(`commands/log.rs`)에 Google Gemini API 백엔드를 추가한다.

현재 두 기능은 `ollama` / `anthropic` / `disabled` 세 가지 백엔드만 지원한다.
Gemini API(Google AI Studio)를 추가하여 로컬 Ollama 없이도 동작 가능하게 하고,
`gemini-2.5-flash` 등 최신 모델을 선택할 수 있도록 한다.

Wiki 생성(`wiki/`)은 이미 4개 백엔드(claude, haiku, ollama, lmstudio)를 지원하므로 이번 범위에서 제외한다.

## Scope

- `crates/secall-core/src/vault/config.rs` — GraphConfig에 `gemini_api_key`, `gemini_model` 필드 추가
- `crates/secall-core/src/graph/semantic.rs` — `extract_with_gemini()` 추가, `extract_with_llm()` 디스패치 확장
- `crates/secall/src/commands/log.rs` — `call_gemini()` 추가, 백엔드 분기 확장

## Non-goals

- Wiki 파이프라인 (`wiki/`) Gemini 백엔드 추가
- Obsidian 플러그인 변경
- REST API 변경
- 임베딩 백엔드 변경 (`search/embedding.rs`)

## Google AI Studio 가용 모델

| 모델 | 용도 | 비고 |
|------|------|------|
| `gemini-2.5-pro` | 최고 품질 | 느림, 비쌈 |
| `gemini-2.5-flash` | 균형 | **추천 기본값** |
| `gemini-2.0-flash` | 빠른 추론 | 가벼운 작업 |
| `gemini-1.5-flash` | 경량 | 무료 티어 사용 가능 |

API 엔드포인트: `https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={API_KEY}`

## Subtasks

| # | 파일 | 내용 |
|---|------|------|
| 01 | `vault/config.rs` | Config 확장 — gemini 필드 추가 |
| 02 | `graph/semantic.rs` | 시맨틱 그래프 Gemini 백엔드 |
| 03 | `commands/log.rs` | Log 일기 Gemini 백엔드 |
| 04 | 검증 | 통합 테스트 및 cargo test |

## Expected Outcome

- `config.toml`에 `[graph] semantic_backend = "gemini"` 설정 후 동작
- `SECALL_GEMINI_API_KEY` 환경변수 또는 config `gemini_api_key` 필드로 인증
- 모델 선택: `gemini_model = "gemini-2.5-flash"` (기본값)
- 기존 ollama/anthropic 동작 무변경
