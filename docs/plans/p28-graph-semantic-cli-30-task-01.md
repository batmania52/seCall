---
type: task
status: draft
updated_at: 2026-04-16
plan: p28-graph-semantic-cli-30
task_number: 1
parallel_group: A
depends_on: []
---

# Task 01 — CLI 플래그 추가 (`main.rs`)

## Changed files

- `crates/secall/src/main.rs:312-322` — `GraphAction::Semantic` variant에 플래그 4개 추가
- `crates/secall/src/main.rs:488-489` — match arm에서 새 플래그를 `run_semantic`에 전달

## Change description

### 1. `GraphAction::Semantic` struct에 필드 추가 (main.rs:315-322)

기존 `delay`, `limit` 아래에 다음 4개 플래그를 추가한다:

```rust
Semantic {
    #[arg(long, default_value_t = 2.5)]
    delay: f64,
    #[arg(long)]
    limit: Option<usize>,

    /// LLM backend override: "ollama" | "gemini" | "anthropic" | "disabled"
    #[arg(long)]
    backend: Option<String>,
    /// API base URL (e.g. http://localhost:11434 for Ollama, or custom endpoint)
    #[arg(long)]
    api_url: Option<String>,
    /// Model name override (e.g. gemma4:e4b, gemini-2.5-flash)
    #[arg(long)]
    model: Option<String>,
    /// API key override (Gemini 등)
    #[arg(long)]
    api_key: Option<String>,
},
```

### 2. match arm 수정 (main.rs:487-490)

```rust
GraphAction::Semantic { delay, limit, backend, api_url, model, api_key } => {
    commands::graph::run_semantic(delay, limit, backend, api_url, model, api_key).await?;
}
```

현재 `run_semantic(delay, limit)` 시그니처는 Task 02에서 변경하므로, Task 01과 Task 02를 연속 적용해야 컴파일된다.

## Dependencies

- 없음 (첫 번째 태스크)

## Verification

```bash
# Task 01 + Task 02 적용 후 컴파일 확인 (단독으로는 시그니처 불일치로 실패)
cargo check -p secall 2>&1 | head -5
```

```bash
# help 출력에 새 플래그 표시 확인
cargo run -p secall -- graph semantic --help 2>&1 | grep -E 'backend|api-url|model|api-key'
```

## Risks

- `backend` 값 검증은 이 태스크에서 하지 않음 (기존 `extract_with_llm`의 match에서 `bail!`로 처리됨)
- `api_key`가 CLI 히스토리에 노출될 수 있음 — 환경변수 사용 권장을 help text에 명시해야 함 (Task 04 문서에서 안내)

## Scope boundary

수정 금지:
- `crates/secall/src/commands/graph.rs` (Task 02 영역)
- `crates/secall-core/src/vault/config.rs` (Task 03 영역)
- `crates/secall-core/src/graph/semantic.rs` (수정 불필요)
