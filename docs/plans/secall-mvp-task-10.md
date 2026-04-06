---
type: task
plan: secall-mvp
task_number: 10
title: MCP 서버
status: draft
parallel_group: 3
depends_on: [8]
updated_at: 2026-04-05
---

# Task 10: MCP 서버

## Changed Files

- `Cargo.toml` (workspace) — MCP 관련 의존성 추가
- `crates/secall-core/src/lib.rs` — `pub mod mcp;` 추가
- `crates/secall-core/src/mcp/mod.rs` — **신규**. MCP 모듈
- `crates/secall-core/src/mcp/server.rs` — **신규**. MCP 서버 구현
- `crates/secall-core/src/mcp/tools.rs` — **신규**. 도구 정의 (recall, get, status)
- `crates/secall-core/src/mcp/instructions.rs` — **신규**. 동적 시스템 프롬프트
- `crates/secall/src/commands/mcp.rs` — **신규**. `secall mcp` 커맨드 구현

## Change Description

### 1. MCP 프로토콜 구현 — rmcp v1.3.0 (검증 완료)

**결정**: `rmcp` v1.3.0 사용. 공식 Rust MCP SDK (`github.com/modelcontextprotocol/rust-sdk`).

**Cargo.toml** (workspace):
```toml
rmcp = { version = "1.3.0", features = ["server", "macros", "schemars", "transport-io"] }
schemars = "1.0"   # tool 파라미터 JsonSchema 생성용
```

**구현 패턴** (`#[tool_router]` + `#[tool_handler]` + `#[tool]` 매크로):

```rust
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo, CallToolResult, Content},
    schemars, tool, tool_handler, tool_router,
    ErrorData as McpError,
    transport::stdio,
};
use serde::Deserialize;

// 파라미터 구조체 — Deserialize + JsonSchema 필수
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RecallParams {
    #[schemars(description = "Search queries array")]
    pub queries: Vec<QueryItem>,
    pub project: Option<String>,
    pub agent: Option<String>,
    pub limit: Option<usize>,
}

// 서버 구조체
#[derive(Clone)]
pub struct SeCallMcpServer {
    tool_router: ToolRouter<Self>,
    db: Arc<Database>,
    search: Arc<SearchEngine>,
}

// #[tool_router] — tool 메서드 자동 등록
#[tool_router]
impl SeCallMcpServer {
    pub fn new(db: Arc<Database>, search: Arc<SearchEngine>) -> Self {
        Self { tool_router: Self::tool_router(), db, search }
    }

    #[tool(description = "Search agent session history")]
    async fn recall(&self, Parameters(params): Parameters<RecallParams>) -> Result<CallToolResult, McpError> {
        let results = self.search.search(&self.db, ...).await.map_err(|e| ...)?;
        Ok(CallToolResult::success(vec![Content::text(serde_json::to_string_pretty(&results)?)]))
    }

    #[tool(description = "Get a specific session or turn")]
    fn get(&self, Parameters(params): Parameters<GetParams>) -> Result<CallToolResult, McpError> { ... }

    #[tool(description = "Show index health")]
    fn status(&self) -> String { ... }  // String → 자동 text content 변환
}

// #[tool_handler] — ServerHandler에 tool 라우팅 주입
#[tool_handler]
impl ServerHandler for SeCallMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(build_instructions(&self.db))
    }
}

// stdio 서버 시작
pub async fn start_mcp_server(db: Database, search: SearchEngine) -> anyhow::Result<()> {
    let server = SeCallMcpServer::new(Arc::new(db), Arc::new(search));
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
```

**핵심 규칙**:
- `Parameters<T>` 래퍼로 파라미터 수신 — `T`는 `Deserialize + JsonSchema` 필수
- `#[tool_router]`가 `Self::tool_router()` 정적 메서드 자동 생성
- `#[tool_handler]`가 `ServerHandler`의 `call_tool`/`list_tools` 자동 구현
- tool 반환: `String`, `CallToolResult`, `Result<CallToolResult, McpError>` 모두 가능
- **stdout은 MCP 전용** — 모든 로그/에러는 `eprintln!`으로 stderr 출력

### 2. MCP 도구 정의

**`recall` 도구**:
```json
{
  "name": "recall",
  "description": "Search agent session history. Use keyword queries for exact terms, semantic queries for conceptual search, or temporal queries for time-based filtering.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "queries": {
        "type": "array",
        "description": "Search queries. Each query has a type and query string.",
        "items": {
          "type": "object",
          "properties": {
            "type": {
              "type": "string",
              "enum": ["keyword", "semantic", "temporal"],
              "description": "keyword: BM25 exact match. semantic: vector similarity. temporal: date filter (today, yesterday, since YYYY-MM-DD)"
            },
            "query": { "type": "string" }
          },
          "required": ["type", "query"]
        }
      },
      "project": {
        "type": "string",
        "description": "Filter by project name"
      },
      "agent": {
        "type": "string",
        "description": "Filter by agent: claude-code, codex, gemini-cli"
      },
      "limit": {
        "type": "integer",
        "description": "Max results (default 10)",
        "default": 10
      }
    },
    "required": ["queries"]
  }
}
```

**`get` 도구**:
```json
{
  "name": "get",
  "description": "Retrieve a specific session or turn. Use session_id for full session, session_id:N for specific turn.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "id": {
        "type": "string",
        "description": "Session ID or session_id:turn_index"
      },
      "full": {
        "type": "boolean",
        "description": "Return full markdown content (default: metadata + summary)",
        "default": false
      }
    },
    "required": ["id"]
  }
}
```

**`status` 도구**:
```json
{
  "name": "status",
  "description": "Show index health: session count, embedding status, recent ingests.",
  "inputSchema": {
    "type": "object",
    "properties": {}
  }
}
```

### 3. 동적 시스템 프롬프트 (instructions.rs)

qmd 패턴 차용. `initialize` 응답에 주입:

```rust
pub fn build_instructions(db: &Database) -> String {
    // 세션 통계
    let session_count = db.count_sessions();
    let projects = db.list_projects();  // ["seCall", "myapp", ...]
    let agents = db.list_agents();      // ["claude-code"]
    let has_embeddings = db.has_embeddings();

    format!(r#"
seCall — Agent Session Search Engine

Index contains {session_count} sessions across {} projects.
Projects: {}
Agents: {}
Vector search: {}

## Usage Tips
- Use `recall` with keyword type for exact term matches (BM25)
- Use `recall` with semantic type for conceptual search (requires embeddings)
- Combine keyword + semantic queries for best results
- Use `get` with session_id:N to read a specific turn
- Filter by project or agent when searching across many sessions

## Example Queries
- Keyword: {{"queries": [{{"type": "keyword", "query": "SQLite FTS5"}}]}}
- Semantic: {{"queries": [{{"type": "semantic", "query": "how to design database schema"}}]}}
- Combined: {{"queries": [{{"type": "keyword", "query": "kiwi-rs"}}, {{"type": "semantic", "query": "Korean tokenizer comparison"}}]}}
- Temporal: {{"queries": [{{"type": "temporal", "query": "yesterday"}}, {{"type": "keyword", "query": "bugfix"}}]}}
"#,
        projects.len(),
        projects.join(", "),
        agents.join(", "),
        if has_embeddings { "enabled" } else { "disabled (run `secall embed`)" },
    )
}
```

### 4. stdio 트랜스포트

```rust
pub async fn start_mcp_server(db: Database, search: SearchEngine) -> Result<()> {
    // stdin/stdout JSON-RPC 2.0
    // 1. 클라이언트에서 "initialize" 수신 → capabilities + instructions 응답
    // 2. "tools/list" → 도구 3개 반환
    // 3. "tools/call" → 도구 실행 + 결과 반환
    // 4. "ping" → "pong"
    // 5. stdin EOF → 종료
}
```

### 5. 클라이언트 설정 안내

`secall init` 완료 시 또는 `secall mcp --setup` 시 출력:

**Claude Code** (`~/.claude/settings.json`):
```json
{
  "mcpServers": {
    "secall": {
      "command": "secall",
      "args": ["mcp"]
    }
  }
}
```

**Claude Desktop** (`~/Library/Application Support/Claude/claude_desktop_config.json`):
```json
{
  "mcpServers": {
    "secall": {
      "command": "/path/to/secall",
      "args": ["mcp"]
    }
  }
}
```

## Dependencies

- Task 08 (SearchEngine)
- `rmcp` v1.3.0 (features: `server`, `macros`, `schemars`, `transport-io`)
- `schemars` v1.0 (tool 파라미터 JsonSchema 생성)
- `serde_json` (이미 있음)

## Verification

```bash
# 빌드 성공
cd /Users/d9ng/privateProject/seCall && cargo build 2>&1 | tail -3

# MCP 서버 초기화 테스트 (stdin에 initialize 전송)
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' | cargo run -- mcp 2>/dev/null | python3 -m json.tool | head -20

# tools/list 테스트
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}\n{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}\n' | cargo run -- mcp 2>/dev/null | tail -1 | python3 -m json.tool

# tools/call recall 테스트
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}\n{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"recall","arguments":{"queries":[{"type":"keyword","query":"test"}]}}}\n' | cargo run -- mcp 2>/dev/null | tail -1 | python3 -m json.tool

# tools/call status 테스트
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}\n{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"status","arguments":{}}}\n' | cargo run -- mcp 2>/dev/null | tail -1 | python3 -m json.tool
```

## Risks

- **rmcp crate 불안정**: Rust MCP 생태계가 초기 단계. rmcp가 빌드 실패하거나 API가 맞지 않으면 직접 구현. 직접 구현은 ~200줄 수준 (initialize + tools/list + tools/call).
- **MCP 프로토콜 버전**: 현재 `2024-11-05`. 이후 버전에서 breaking change 가능. protocolVersion 필드로 감지하고 미지원 버전은 에러 반환.
- **stderr/stdout 혼선**: MCP는 stdout만 사용. 로그/에러는 반드시 stderr로 출력. `println!` 대신 `eprintln!` 사용 주의.
- **DB 동시 접근**: MCP 서버가 장기 실행되는 동안 CLI에서 ingest 실행 시 SQLite 잠금 충돌. `WAL` 모드 활성화로 완화 (`PRAGMA journal_mode=WAL`).

## Scope Boundary

- HTTP 트랜스포트는 이 태스크에서 구현하지 않음 (Task 17)
- MCP resources (URI 기반 문서 접근)는 post-MVP
- 도구 3개만 (recall, get, status). multi_get은 post-MVP
