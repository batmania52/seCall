---
type: task
plan: secall-mvp
task_number: 1
title: Rust workspace 초기화
status: draft
parallel_group: 0
depends_on: []
updated_at: 2026-04-05
---

# Task 01: Rust workspace 초기화

## Changed Files

- `Cargo.toml` — **신규**. Workspace root manifest
- `crates/secall-core/Cargo.toml` — **신규**. Core library crate
- `crates/secall-core/src/lib.rs` — **신규**. Library entrypoint (re-exports)
- `crates/secall/Cargo.toml` — **신규**. Binary crate
- `crates/secall/src/main.rs` — **신규**. CLI entrypoint with clap
- `.gitignore` — **신규**. Rust + IDE ignores
- `rust-toolchain.toml` — **신규**. MSRV pinning

## Change Description

### 1. Workspace 구조

```
seCall/
├── Cargo.toml              # [workspace] members
├── rust-toolchain.toml     # channel = "stable", msrv
├── .gitignore
├── crates/
│   ├── secall-core/        # 라이브러리: 파서, 인덱서, 검색, store
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   └── secall/             # 바이너리: CLI + MCP
│       ├── Cargo.toml
│       └── src/
│           └── main.rs
└── docs/
```

### 2. Root Cargo.toml

```toml
[workspace]
members = ["crates/secall-core", "crates/secall"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.75"
license = "MIT"
repository = "https://github.com/<user>/seCall"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
tokio = { version = "1", features = ["full"] }
anyhow = "1"
thiserror = "2"
# 이후 태스크에서 추가될 workspace dependencies (여기서는 선언하지 않음):
# rusqlite = { version = "0.39", features = ["bundled"] }  — Task 02
# lindera = { version = "2.3.4", features = ["embed-ko-dic"] }  — Task 06
# sqlite-vec = "0.1.9"  — Task 07
# zerocopy = "0.7"  — Task 07
# reqwest = { version = "0.12", features = ["json"] }  — Task 07
# rmcp = { version = "1.3.0", features = ["server", "macros", "schemars", "transport-io"] }  — Task 10
# schemars = "1.0"  — Task 10
# dirs = "6"  — Task 02
# toml = "0.8"  — Task 05
# walkdir = "2"  — Task 03
```

### 3. secall-core Cargo.toml

```toml
[package]
name = "secall-core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
anyhow.workspace = true
thiserror.workspace = true
```

`lib.rs`: 빈 모듈 선언 (추후 `pub mod ingest;`, `pub mod search;` 등 추가)

### 4. secall Cargo.toml

```toml
[package]
name = "secall"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
secall-core = { path = "../secall-core" }
clap = { version = "4", features = ["derive"] }
tokio.workspace = true
anyhow.workspace = true
```

### 5. main.rs — clap 스켈레톤

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "secall", version, about = "Agent session search engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Ingest agent session logs
    Ingest { path: String },
    /// Search session history
    Recall { query: String },
    /// Get a specific session or turn
    Get { id: String },
    /// Show index status
    Status,
    /// Start MCP server
    Mcp,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Ingest { path } => println!("ingest: {path}"),
        Commands::Recall { query } => println!("recall: {query}"),
        Commands::Get { id } => println!("get: {id}"),
        Commands::Status => println!("status"),
        Commands::Mcp => println!("mcp"),
    }
    Ok(())
}
```

### 6. .gitignore

```
/target
*.swp
*.swo
.DS_Store
```

### 7. rust-toolchain.toml

```toml
[toolchain]
channel = "stable"
```

## Dependencies

- 없음 (첫 번째 태스크)

## Verification

```bash
# 빌드 성공 확인
cd /Users/d9ng/privateProject/seCall && cargo build 2>&1 | tail -5

# 바이너리 실행 확인
cargo run -- --version

# 서브커맨드 동작 확인
cargo run -- status

# workspace 멤버 확인
cargo metadata --format-version=1 | python3 -c "import sys,json; pkgs=json.load(sys.stdin)['packages']; names=[p['name'] for p in pkgs if p['source'] is None]; print(sorted(names)); assert 'secall' in names and 'secall-core' in names"
```

## Risks

- **workspace.dependencies 버전 충돌**: `tokio`, `serde` 등 major 버전을 workspace 레벨에서 고정하면 하위 crate에서 다른 버전 필요 시 충돌. 현재 모든 의존성이 최신 major이므로 리스크 낮음.
- **MSRV 1.75가 너무 높거나 낮음**: `workspace.edition = "2021"`은 Rust 1.56+에서 지원하지만, `resolver = "2"` 등 기능은 1.75면 충분. lindera/rusqlite MSRV 확인 필요 (Task 02, 06에서 검증).

## Scope Boundary

- `docs/` 디렉토리 — 수정하지 않음
- `CLAUDE.md` — 수정하지 않음 (프로젝트 상태 업데이트는 전체 MVP 완료 시)
