---
type: task
status: draft
updated_at: 2026-04-16
plan: p28-graph-semantic-cli-30
task_number: 4
parallel_group: null
depends_on: [1, 2, 3]
---

# Task 04 — 테스트 및 문서

## Changed files

- `crates/secall-core/src/vault/config.rs` (테스트 모듈) — 환경변수 오버라이드 테스트 추가
- `README.md` 또는 `docs/reference/index.md` — CLI 플래그 및 환경변수 문서화

## Change description

### 1. 환경변수 오버라이드 단위 테스트 (config.rs 테스트 모듈)

기존 `mod tests` 블록에 테스트 추가:

```rust
#[test]
fn test_graph_env_override_backend() {
    std::env::set_var("SECALL_GRAPH_BACKEND", "gemini");
    let config = Config::default().apply_env_overrides();
    assert_eq!(config.graph.semantic_backend, "gemini");
    std::env::remove_var("SECALL_GRAPH_BACKEND");
}

#[test]
fn test_graph_env_override_api_url() {
    std::env::set_var("SECALL_GRAPH_API_URL", "http://custom:8080");
    let config = Config::default().apply_env_overrides();
    assert_eq!(config.graph.ollama_url, Some("http://custom:8080".to_string()));
    std::env::remove_var("SECALL_GRAPH_API_URL");
}

#[test]
fn test_graph_env_override_model_gemini() {
    std::env::set_var("SECALL_GRAPH_BACKEND", "gemini");
    std::env::set_var("SECALL_GRAPH_MODEL", "gemini-2.0-flash");
    let config = Config::default().apply_env_overrides();
    assert_eq!(config.graph.gemini_model, Some("gemini-2.0-flash".to_string()));
    std::env::remove_var("SECALL_GRAPH_BACKEND");
    std::env::remove_var("SECALL_GRAPH_MODEL");
}

#[test]
fn test_graph_env_override_api_key() {
    std::env::set_var("SECALL_GRAPH_API_KEY", "test-key-123");
    let config = Config::default().apply_env_overrides();
    assert_eq!(config.graph.gemini_api_key, Some("test-key-123".to_string()));
    std::env::remove_var("SECALL_GRAPH_API_KEY");
}
```

**주의**: `apply_env_overrides`는 현재 `pub`이 아닐 수 있음 — 테스트 모듈이 같은 파일 내에 있으므로 접근 가능. 만약 `fn apply_env_overrides`가 private이면 그대로 둔다 (같은 모듈 내 테스트에서 접근 가능).

### 2. CLI help 출력 검증

별도 테스트 코드 불필요. Verification 명령으로 확인:

```bash
cargo run -p secall -- graph semantic --help
```

출력에 `--backend`, `--api-url`, `--model`, `--api-key` 4개 옵션이 포함되어야 함.

### 3. 문서 업데이트 (`docs/reference/index.md`)

`graph semantic` 섹션에 새 플래그와 환경변수 추가:

```markdown
### graph semantic

| Flag | Description | Default |
|------|------------|---------|
| `--delay <SECS>` | 세션 간 대기 시간 | 2.5 |
| `--limit <N>` | 최대 처리 세션 수 | 전체 |
| `--backend <NAME>` | LLM 백엔드 (ollama/gemini/anthropic/disabled) | config.toml |
| `--api-url <URL>` | API base URL (Ollama용) | config.toml |
| `--model <NAME>` | 모델명 | config.toml |
| `--api-key <KEY>` | API 키 (Gemini 등) | config.toml |

환경변수: `SECALL_GRAPH_BACKEND`, `SECALL_GRAPH_API_URL`, `SECALL_GRAPH_MODEL`, `SECALL_GRAPH_API_KEY`
우선순위: CLI 플래그 > 환경변수 > config.toml > 기본값
```

### 4. Issue #30 close 준비

커밋 메시지에 `Fixes #30` 포함하여 자동 close 되도록 한다.

## Dependencies

- Task 01, 02, 03 모두 완료 후 (전체 기능이 동작해야 테스트 가능)

## Verification

```bash
# 전체 테스트 실행
cargo test -p secall-core -- config 2>&1 | tail -10
```

```bash
# 빌드 + 전체 테스트
cargo test -p secall-core -p secall 2>&1 | tail -5
```

```bash
# CLI help 확인
cargo run -p secall -- graph semantic --help 2>&1 | grep -c -E 'backend|api-url|model|api-key'
# 기대값: 4 (4개 플래그 모두 표시)
```

```bash
# E2E: 환경변수로 disabled 설정 시 즉시 종료
SECALL_GRAPH_BACKEND=disabled cargo run -p secall -- graph semantic --limit 1 2>&1 | grep -i disabled
```

```bash
# E2E: CLI 플래그가 환경변수보다 우선
SECALL_GRAPH_BACKEND=ollama cargo run -p secall -- graph semantic --backend disabled --limit 1 2>&1 | grep -i disabled
```

## Risks

- 환경변수 테스트가 병렬 실행 시 서로 간섭 가능 (`set_var`/`remove_var`는 process-global) — Rust 기본 테스트 러너는 스레드 병렬이므로 `#[serial]` 매크로 또는 `--test-threads=1`이 필요할 수 있음. 기존 config 테스트가 이미 같은 패턴이면 동일하게 처리.
- `docs/reference/index.md`에 `graph semantic` 섹션이 없으면 새로 추가

## Scope boundary

수정 금지:
- `crates/secall-core/src/graph/semantic.rs` — 기존 추출 로직 변경 불필요
- `crates/secall/src/commands/graph.rs`의 비즈니스 로직 (Task 02에서 완료)
