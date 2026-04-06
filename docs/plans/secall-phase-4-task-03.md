---
type: task
status: draft
plan: secall-phase-4
task_number: 3
title: "MCP HTTP transport"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 03: MCP HTTP transport

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/mcp/server.rs:233-240` | 수정 | HTTP 서버 시작 함수 추가 |
| `crates/secall/src/commands/mcp.rs` | 수정 | `--http` 옵션 추가 |
| `crates/secall/src/main.rs` | 수정 | Mcp 서브커맨드에 http 필드 추가 |
| `Cargo.toml` | 수정 | rmcp features에 `transport-sse` 추가 |

## Change description

### 1. rmcp HTTP transport

현재 rmcp 1.3.0의 Cargo.toml features:
- `transport-io` (현재 사용) — stdio 기반
- `transport-sse` — SSE (Server-Sent Events) 기반 HTTP
- `transport-streamable-http` — Streamable HTTP

MCP 스펙상 HTTP transport는 SSE 기반. `transport-sse` feature를 사용.

### 2. Cargo.toml 수정

```toml
rmcp = { version = "1.3.0", features = ["server", "macros", "schemars", "transport-io", "transport-sse"] }
```

### 3. server.rs 수정

기존 `start_mcp_server()`는 stdio 전용. HTTP 서버 함수 추가:

```rust
pub async fn start_mcp_server(db: Database, search: SearchEngine) -> anyhow::Result<()> {
    let server = SeCallMcpServer::new(Arc::new(Mutex::new(db)), Arc::new(search));
    let (stdin, stdout) = rmcp::transport::io::stdio();
    let service = server.serve((stdin, stdout)).await?;
    service.waiting().await?;
    Ok(())
}

/// Start MCP server with HTTP/SSE transport
pub async fn start_mcp_http_server(
    db: Database,
    search: SearchEngine,
    bind_addr: &str,
) -> anyhow::Result<()> {
    use rmcp::transport::sse::SseServer;

    let server = SeCallMcpServer::new(Arc::new(Mutex::new(db)), Arc::new(search));

    eprintln!("✓ MCP HTTP server listening on {bind_addr}");
    eprintln!("  Connect: http://{bind_addr}/sse");

    let sse_server = SseServer::serve(bind_addr.parse()?)
        .await?;

    let service = server.serve(sse_server).await?;
    service.waiting().await?;
    Ok(())
}
```

주의: rmcp 1.3.0의 SSE API가 위와 정확히 일치하지 않을 수 있음. 실제 API를 확인해야 함.

### 4. CLI 수정

`main.rs` Mcp 서브커맨드 확장:

```rust
/// Start MCP server
Mcp {
    /// Start HTTP server instead of stdio (e.g., --http 127.0.0.1:8080)
    #[arg(long)]
    http: Option<String>,
},
```

`commands/mcp.rs`:
```rust
pub async fn run(http: Option<String>) -> Result<()> {
    let db_path = get_default_db_path();
    let db = Database::open(&db_path)?;

    let config = Config::load_or_default();
    let tok = create_tokenizer(&config.search.tokenizer)?;
    let bm25 = Bm25Indexer::new(tok);
    let vector = create_vector_indexer(&config).await;
    let search = SearchEngine::new(bm25, vector);

    match http {
        Some(addr) => start_mcp_http_server(db, search, &addr).await,
        None => start_mcp_server(db, search).await,
    }
}
```

참고: 현재 `commands/mcp.rs:9-18`는 `LinderaKoTokenizer`를 하드코딩하고 벡터 검색이 비활성화 상태. 이 task에서 config 기반으로 수정해야 MCP 서버가 전체 검색 엔진을 활용할 수 있음.

### 5. mcp.rs 개선 (기존 문제 수정)

현재 `commands/mcp.rs`는:
- `LinderaKoTokenizer` 하드코딩 → `create_tokenizer(&config.search.tokenizer)` 사용
- 벡터 인덱서 없음 (`SearchEngine::new(bm25, None)`) → `create_vector_indexer(&config).await` 사용

이 수정은 HTTP transport 추가와 함께 자연스럽게 포함됨.

## Dependencies

- 없음 (다른 task와 독립)
- rmcp 1.3.0의 `transport-sse` feature API 확인 필요

## Verification

```bash
# 타입 체크
cargo check

# CLI 옵션 등록 확인
cargo run -p secall -- mcp --help

# stdio 모드 회귀 (기존 동작 유지)
# Manual: secall mcp → Claude Code에서 MCP 도구 호출

# HTTP 모드 테스트
# Manual: cargo run -p secall -- mcp --http 127.0.0.1:8080
# Manual: curl http://127.0.0.1:8080/sse (SSE 연결 확인)

# 전체 테스트 회귀
cargo test
```

## Risks

- **rmcp SSE API 불확실**: rmcp 1.3.0의 `transport-sse` API가 문서화되지 않았을 수 있음. crate 소스 확인 필요. API가 다르면 구현 조정
- **보안 없음**: HTTP transport에 인증/TLS 없음. localhost 전용으로 문서화. 외부 노출 금지 경고 출력
- **동시 접속**: SSE는 클라이언트별 연결 유지. 다중 클라이언트 접속 시 리소스 사용 확인 필요
- **mcp.rs 하드코딩 수정**: 기존 stdio 모드에서 `LinderaKoTokenizer` 하드코딩 → config 기반으로 변경. 기존 사용자에게 default config("lindera")이므로 동작 동일

## Scope Boundary

수정 금지 파일:
- `mcp/tools.rs` — MCP 도구 정의는 변경하지 않음
- `mcp/instructions.rs` — 변경 불필요
- `search/*` — 검색 모듈은 이 task에서 직접 변경하지 않음 (mcp.rs에서 호출 방식만 변경)
- `ingest/*` — 파서 변경 금지
