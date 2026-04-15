---
type: task
status: in_progress
updated_at: 2026-04-15
plan: p26-gemini-api-log
task_number: 01
title: Config 확장 — Gemini 필드 추가
parallel_group: A
depends_on: []
---

# Task 01 — Config 확장 (GraphConfig에 Gemini 필드 추가)

## Changed files

- `crates/secall-core/src/vault/config.rs` (수정)
  - `GraphConfig` 구조체: 151-176줄

## Change description

### 현재 상태 (`GraphConfig`, L151-176)

```rust
pub struct GraphConfig {
    pub semantic: bool,
    pub semantic_backend: String,     // "ollama" | "anthropic" | "disabled"
    pub ollama_url: Option<String>,
    pub ollama_model: Option<String>,
    pub anthropic_model: Option<String>,
}
```

### 변경 내용

`GraphConfig` 구조체에 두 필드를 추가한다:

```rust
pub gemini_api_key: Option<String>,   // Google AI Studio API 키 (없으면 env SECALL_GEMINI_API_KEY)
pub gemini_model: Option<String>,     // 기본값: "gemini-2.5-flash"
```

`Default` 구현에도 두 필드를 추가한다:

```rust
gemini_api_key: None,
gemini_model: None,
```

API 키 우선순위 (런타임에서 처리):
1. `config.graph.gemini_api_key` (설정값)
2. `SECALL_GEMINI_API_KEY` 환경변수 (fallback)

> Config 구조체는 TOML 역직렬화 대상이므로 `serde(default)` 속성이 이미 적용되어 있어 기존 config.toml에 필드 없어도 기본값으로 동작한다.

## Dependencies

없음 (독립 task)

## Verification

```bash
cd /Users/d9ng/privateProject/seCall
cargo check -p secall-core 2>&1 | tail -5
```

컴파일 에러 없이 `warning: ...` 또는 `Finished` 출력되면 통과.

## Risks

- TOML 역직렬화: `serde(default)` 적용 여부 확인 필요. 적용되어 있으면 기존 config.toml 파일 하위 호환성 문제 없음.
- 구조체 변경은 secall-core 내부에서만 사용되므로 외부 API 영향 없음.

## Scope boundary

수정 금지 파일:
- `graph/semantic.rs` (Task 02 영역)
- `commands/log.rs` (Task 03 영역)
