---
type: task
status: draft
updated_at: 2026-04-16
plan: p28-graph-semantic-cli-30
task_number: 3
parallel_group: A
depends_on: []
---

# Task 03 — 환경변수 fallback (`config.rs`)

## Changed files

- `crates/secall-core/src/vault/config.rs:338-342` — `apply_env_overrides` 메서드 확장

## Change description

### `apply_env_overrides`에 graph 관련 환경변수 4개 추가

현재 `apply_env_overrides`는 `SECALL_VAULT_PATH`만 처리한다. 여기에 graph 관련 환경변수를 추가한다:

```rust
fn apply_env_overrides(mut self) -> Self {
    if let Ok(p) = std::env::var("SECALL_VAULT_PATH") {
        self.vault.path = PathBuf::from(p);
    }
    // Graph semantic 환경변수 (CLI 플래그보다 낮은 우선순위)
    if let Ok(b) = std::env::var("SECALL_GRAPH_BACKEND") {
        self.graph.semantic_backend = b;
    }
    if let Ok(u) = std::env::var("SECALL_GRAPH_API_URL") {
        self.graph.ollama_url = Some(u);
    }
    if let Ok(m) = std::env::var("SECALL_GRAPH_MODEL") {
        match self.graph.semantic_backend.as_str() {
            "gemini" => self.graph.gemini_model = Some(m),
            "anthropic" => self.graph.anthropic_model = Some(m),
            _ => self.graph.ollama_model = Some(m),
        }
    }
    if let Ok(k) = std::env::var("SECALL_GRAPH_API_KEY") {
        self.graph.gemini_api_key = Some(k);
    }
    self
}
```

**우선순위 체인**: CLI 플래그 (Task 02) > 환경변수 (이 Task) > config.toml > Default

환경변수는 `Config::load_or_default()` → `apply_env_overrides()` 시점에 적용되고, CLI 플래그는 그 이후에 `commands/graph.rs`에서 덮어쓰므로 자연스럽게 우선순위가 보장된다.

### 환경변수명 규칙

| 환경변수 | 용도 | 예시 값 |
|---------|------|--------|
| `SECALL_GRAPH_BACKEND` | 시맨틱 백엔드 | `gemini`, `ollama`, `anthropic`, `disabled` |
| `SECALL_GRAPH_API_URL` | API base URL | `http://localhost:11434` |
| `SECALL_GRAPH_MODEL` | 모델명 | `gemma4:e4b`, `gemini-2.5-flash` |
| `SECALL_GRAPH_API_KEY` | API 키 | `AIza...` |

기존 `SECALL_GEMINI_API_KEY` 환경변수 (semantic.rs에서 직접 읽음)와의 관계:
- `SECALL_GRAPH_API_KEY`는 config 레벨에서 `gemini_api_key` 필드에 설정
- `SECALL_GEMINI_API_KEY`는 semantic.rs에서 `gemini_api_key`가 None일 때 fallback으로 읽음
- 둘 다 설정 시 `SECALL_GRAPH_API_KEY`가 우선 (config 필드에 먼저 설정되므로)

## Dependencies

- 없음 (Task 01과 parallel_group A로 병렬 가능)

## Verification

```bash
cargo check -p secall-core
```

```bash
cargo test -p secall-core -- config 2>&1 | tail -5
```

```bash
# 환경변수 적용 확인 (backend=disabled로 즉시 종료)
SECALL_GRAPH_BACKEND=disabled cargo run -p secall -- graph semantic --delay 0 --limit 0 2>&1 | grep -i disabled
```

## Risks

- `SECALL_GRAPH_MODEL` 매핑이 `semantic_backend` 값에 의존 — `SECALL_GRAPH_BACKEND`와 `SECALL_GRAPH_MODEL`을 동시에 설정할 때, `apply_env_overrides` 내 순서가 중요함 (backend를 먼저 적용해야 model 매핑이 올바름 — 위 코드에서 이미 이 순서를 보장)
- 기존 `SECALL_GEMINI_API_KEY` 환경변수와 혼동 가능 — Task 04 문서에서 명확히 안내

## Scope boundary

수정 금지:
- `crates/secall/src/main.rs` (Task 01 영역)
- `crates/secall/src/commands/graph.rs` (Task 02 영역)
- `crates/secall-core/src/graph/semantic.rs` — 기존 `SECALL_GEMINI_API_KEY` fallback은 유지
