---
type: task
status: draft
plan: secall-refactor-p2
task_number: 1
title: "tracing 도입"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 01: tracing 도입

## 문제

12개 파일에 32개 `eprintln!` 호출이 유일한 로깅 수단. 로그 레벨 필터링, 구조적 필드, 타임스탬프 등 운영 기능이 없음.

### eprintln! 분포

| 파일 | 수 |
|---|---|
| `crates/secall-core/src/search/vector.rs` | 10 |
| `crates/secall-core/src/search/model_manager.rs` | 6 |
| `crates/secall-core/src/mcp/server.rs` | 4 |
| `crates/secall-core/src/hooks/mod.rs` | 3 |
| `crates/secall/src/commands/ingest.rs` | 3 |
| `crates/secall-core/src/search/tokenizer.rs` | 2 |
| `crates/secall-core/src/search/bm25.rs` | 2 |
| `crates/secall/src/commands/wiki.rs` | 2 |
| `crates/secall-core/src/ingest/claude.rs` | 1 |
| `crates/secall-core/src/ingest/gemini.rs` | 1 |
| `crates/secall/src/commands/embed.rs` | 1 |
| `crates/secall-core/src/search/query_expand.rs` | 1 |

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `Cargo.toml` (workspace) | 추가 | `tracing`, `tracing-subscriber` 의존성 |
| `crates/secall-core/Cargo.toml` | 추가 | `tracing.workspace = true` |
| `crates/secall/Cargo.toml` | 추가 | `tracing.workspace = true`, `tracing-subscriber.workspace = true` |
| `crates/secall/src/main.rs:169` | 추가 | subscriber 초기화 (stderr 전용) |
| 12개 소스 파일 | 수정 | `eprintln!` → `tracing::warn!/info!/debug!` 변환 |

## Change description

### Step 1: Cargo.toml 의존성 추가

```toml
# Cargo.toml (workspace)
[workspace.dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# crates/secall-core/Cargo.toml
[dependencies]
tracing.workspace = true

# crates/secall/Cargo.toml
[dependencies]
tracing.workspace = true
tracing-subscriber.workspace = true
```

### Step 2: main.rs에 subscriber 초기화

```rust
// main.rs — #[tokio::main] 직후, Cli::parse() 전
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // stderr 전용 — stdout은 MCP 프로토콜 전용
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("warn"))
        )
        .with_writer(std::io::stderr)
        .with_target(false)
        .init();

    let cli = Cli::parse();
    ...
}
```

> 기본 레벨: `warn`. `RUST_LOG=debug secall recall "query"`로 디버그 가능.

### Step 3: eprintln! 변환 규칙

| 기존 패턴 | tracing 레벨 | 이유 |
|-----------|-------------|------|
| `eprintln!("warn: ...")` | `tracing::warn!` | 경고 |
| `eprintln!("✓ ...")` | `tracing::info!` | 성공 알림 |
| `eprintln!("⚠ ...")` | `tracing::warn!` | 경고 |
| `eprintln!("debug: ...")` | `tracing::debug!` | 디버그 |

예시 변환:

```rust
// vector.rs — 변경 전
eprintln!("warn: vector insert error: {e}");
// 변경 후
tracing::warn!(error = %e, "vector insert error");

// vector.rs — 변경 전
eprintln!("✓ ort ONNX loaded. Local vector search enabled.");
// 변경 후
tracing::info!("ort ONNX loaded, local vector search enabled");

// ingest.rs — 변경 전
eprintln!("warn: vault write failed for {}: {e}", session_path.display());
// 변경 후
tracing::warn!(path = %session_path.display(), error = %e, "vault write failed");
```

### Step 4: MCP 서버 로깅 주의

`mcp/server.rs`의 `eprintln!`도 변환. MCP가 stdio 모드일 때 subscriber는 이미 stderr로 설정되어 있으므로 안전.

```rust
// server.rs — 변경 전
eprintln!("MCP server error: {e}");
// 변경 후
tracing::error!(error = %e, "MCP server error");
```

### Step 5: 로그 출력에 영향 받는 테스트 확인

일부 테스트가 stderr 출력을 검증하지는 않으므로 영향 없음. 단, `ingest.rs:99`의 Summary 출력은 `eprintln!`이 아닌 사용자 대면 출력이므로 **변환하지 않음**:

```rust
// 유지 (사용자 UI 출력)
eprintln!(
    "\nSummary: {} ingested, {} skipped (duplicate), {} errors",
    ingested, skipped, errors
);
```

> Summary는 항상 보여야 하므로 `eprintln!` 유지 또는 `tracing::info!`로 변환 후 기본 레벨에서 표시되도록 설정.

## Dependencies

- 없음 (독립 실행 가능)
- Refactor P1 Task 01 (ingest error)에서 추가된 `eprintln!`도 이 task에서 변환

## Verification

```bash
# 1. 컴파일 확인
cargo check

# 2. 전체 테스트 회귀 없음
cargo test

# 3. 기본 레벨(warn)에서 경고만 출력되는지 확인
secall status 2>&1 >/dev/null | head -5

# 4. debug 레벨에서 상세 로그 출력되는지 확인
RUST_LOG=debug secall status 2>&1 >/dev/null | grep -c "DEBUG\|INFO\|WARN"

# 5. MCP 모드에서 stdout 오염 없는지 확인
echo '{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"capabilities":{}}}' | secall mcp 2>/dev/null | python3 -c "import sys,json; json.load(sys.stdin); print('stdout clean')"
```

> **[Developer 필수]** subtask-done 시그널 전에 위 명령의 실행 결과를 result 문서에 기록하세요. 형식: `✅ 명령 — exit 0` 또는 `❌ 명령 — 에러 내용 (사유)`. 검증 증빙 미제출 시 리뷰에서 conditional 처리됩니다.

## Risks

- **MCP stdout 오염**: subscriber가 stdout에 쓰면 MCP 프로토콜 깨짐. `with_writer(std::io::stderr)` 필수.
- **Summary 출력**: ingest 완료 후 Summary 메시지는 사용자에게 항상 보여야 함. `eprintln!` 유지 또는 `tracing::info!` + 기본 레벨 `info` 고려.
- **tracing 오버헤드**: 무시할 수준. disabled 레벨의 매크로는 no-op으로 컴파일됨.
- **기존 eprintln 의존**: 현재 어떤 테스트도 stderr 출력을 assert하지 않으므로 안전.

## Scope boundary

다음 파일은 이 task에서 수정하지 않음:
- `crates/secall/src/output.rs` — 사용자 대면 출력 (`println!`). 로깅이 아님.
- `crates/secall/src/commands/status.rs` — `println!`만 사용. eprintln 없음.
- `crates/secall/src/commands/get.rs` — `println!`만 사용. eprintln 없음.
