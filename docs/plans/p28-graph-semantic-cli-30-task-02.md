---
type: task
status: draft
updated_at: 2026-04-16
plan: p28-graph-semantic-cli-30
task_number: 2
parallel_group: null
depends_on: [1]
---

# Task 02 — GraphConfig 오버라이드 로직 (`main.rs` + `commands/graph.rs`)

## Changed files

- `crates/secall/src/commands/graph.rs:10` — `run_semantic` 시그니처 변경
- `crates/secall/src/commands/graph.rs:10-20` — CLI 플래그로 `GraphConfig` 필드 오버라이드 로직 추가

## Change description

### 1. `run_semantic` 시그니처 확장 (graph.rs:10)

```rust
pub async fn run_semantic(
    delay_secs: f64,
    limit: Option<usize>,
    backend: Option<String>,
    api_url: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
) -> Result<()> {
```

### 2. config 오버라이드 적용 (graph.rs:11 이후, config 로드 직후)

`Config::load_or_default()` 호출 후, CLI 플래그가 `Some`인 경우 `config.graph` 필드를 덮어쓴다:

```rust
let mut config = Config::load_or_default();

// CLI 플래그 오버라이드 (우선순위: CLI > 환경변수 > config.toml > 기본값)
if let Some(b) = backend {
    config.graph.semantic_backend = b;
}
if let Some(u) = api_url {
    config.graph.ollama_url = Some(u);
}
if let Some(m) = model {
    // backend에 따라 적절한 모델 필드에 설정
    match config.graph.semantic_backend.as_str() {
        "gemini" => config.graph.gemini_model = Some(m),
        "anthropic" => config.graph.anthropic_model = Some(m),
        _ => config.graph.ollama_model = Some(m),
    }
}
if let Some(k) = api_key {
    config.graph.gemini_api_key = Some(k);
}
```

**`api_url` 처리 설계 결정**: `--api-url`은 현재 `ollama_url` 필드에 매핑한다. Ollama 백엔드에서만 base URL을 사용하고, Gemini/Anthropic은 SDK 내부에 고정된 엔드포인트를 사용하기 때문이다. 향후 커스텀 엔드포인트가 필요하면 별도 필드를 추가한다.

단, Gemini 백엔드 사용자가 `--api-url`을 지정하는 유스케이스(OpenAI-compatible proxy 등)를 위해 `gemini` 백엔드일 때도 `ollama_url` 대신 범용 필드로 전달하는 방안을 고려한다. 현재 `extract_with_gemini`가 URL을 하드코딩하고 있으므로, 이 태스크에서는 Ollama 전용으로 제한하고 help text에 명시한다.

### 3. 기존 코드 변경 없음

`extract_and_store`, `extract_with_llm` 등 secall-core 코드는 이미 `GraphConfig` 필드를 참조하므로 수정 불필요. CLI에서 `config.graph`를 덮어쓰면 자동으로 반영된다.

## Dependencies

- Task 01 (CLI 플래그 정의가 선행되어야 함)

## Verification

```bash
cargo check -p secall
```

```bash
cargo build -p secall 2>&1 | tail -3
```

```bash
# 기존 동작 유지 확인 (backend 없이 실행 시 config.toml 값 사용)
cargo run -p secall -- graph semantic --delay 0 --limit 0 2>&1 | head -3
```

## Risks

- `--model` 매핑이 `--backend` 값에 의존 — `--backend` 없이 `--model`만 지정하면 config.toml의 기존 backend 기준으로 매핑됨 (의도된 동작)
- `--api-url`이 Ollama 전용으로 제한됨 — Gemini/Anthropic 커스텀 엔드포인트는 미지원 (Non-goal)

## Scope boundary

수정 금지:
- `crates/secall-core/src/graph/semantic.rs` — 오버라이드는 config 레벨에서 처리, core 로직 변경 불필요
- `crates/secall-core/src/vault/config.rs` (Task 03 영역)
- `crates/secall/src/main.rs`의 `GraphAction::Semantic` 외 다른 variant
