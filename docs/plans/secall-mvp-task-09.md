---
type: task
plan: secall-mvp
task_number: 9
title: CLI 완성
status: draft
parallel_group: 3
depends_on: [5, 8]
updated_at: 2026-04-05
---

# Task 09: CLI 완성

## Changed Files

- `crates/secall/src/main.rs` — clap 스켈레톤 → 실제 서브커맨드 구현
- `crates/secall/src/commands/mod.rs` — **신규**. 커맨드 모듈
- `crates/secall/src/commands/ingest.rs` — **신규**. `secall ingest` 구현
- `crates/secall/src/commands/recall.rs` — **신규**. `secall recall` 구현
- `crates/secall/src/commands/get.rs` — **신규**. `secall get` 구현
- `crates/secall/src/commands/status.rs` — **신규**. `secall status` 구현
- `crates/secall/src/commands/embed.rs` — **신규**. `secall embed` 구현
- `crates/secall/src/commands/init.rs` — **신규**. `secall init` 구현
- `crates/secall/src/output.rs` — **신규**. 출력 포맷팅 (텍스트/JSON)

## Change Description

### 1. CLI 구조 (clap derive)

```rust
#[derive(Parser)]
#[command(name = "secall", version, about = "Agent session search engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output format
    #[arg(long, global = true, default_value = "text")]
    format: OutputFormat,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize vault and config
    Init {
        /// Vault path
        #[arg(short, long)]
        vault: Option<PathBuf>,
    },

    /// Ingest agent session logs
    Ingest {
        /// Session file path, session ID, or --auto
        path: Option<String>,

        /// Auto-detect new sessions from ~/.claude/projects/
        #[arg(long)]
        auto: bool,

        /// Filter by project directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Search session history
    Recall {
        /// Search query
        query: Vec<String>,   // 여러 단어를 공백으로 합침

        /// Temporal filter
        #[arg(long)]
        since: Option<String>,  // "yesterday", "2026-04-01", etc.

        /// Filter by project
        #[arg(long, short)]
        project: Option<String>,

        /// Filter by agent
        #[arg(long, short)]
        agent: Option<String>,

        /// Max results
        #[arg(long, short = 'n', default_value = "10")]
        limit: usize,

        /// BM25-only (skip vector search)
        #[arg(long)]
        lex: bool,

        /// Vector-only (skip BM25)
        #[arg(long)]
        vec: bool,
    },

    /// Get a specific session or turn
    Get {
        /// Session ID or session_id:turn_index
        id: String,

        /// Show full content (default: summary)
        #[arg(long)]
        full: bool,
    },

    /// Show index status
    Status,

    /// Generate vector embeddings for un-embedded sessions
    Embed {
        /// Re-embed all sessions
        #[arg(long)]
        all: bool,
    },

    /// Start MCP server (Task 10)
    Mcp {
        // ... (Task 10에서 구현)
    },
}

#[derive(Clone, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}
```

### 2. 서브커맨드 구현

**`secall init`**:
1. `~/.config/secall/config.toml` 생성 (vault 경로 설정)
2. `vault::init_vault()` 호출
3. DB 생성 + 스키마 적용
4. 성공 메시지 출력

**`secall ingest <path>`**:
1. Config 로드 → DB 열기 → Vault 초기화 확인
2. 경로 분석:
   - 파일 경로 → 직접 파싱
   - 세션 ID → `~/.claude/projects/`에서 탐색
   - `--auto` → 미처리 세션 자동 탐지 (DB에 없는 세션)
3. `SessionParser::parse()` → `Session`
4. `Vault::write_session()` → MD 파일 생성
5. `SearchEngine::index_session()` → BM25 + 벡터 인덱싱
6. `DB::insert_session()` → 메타데이터 저장
7. 결과 출력 (파일 경로, 턴 수, 인덱싱 통계)

**`secall recall <query>`**:
1. Config 로드 → DB 열기
2. `SearchFilters` 구성 (--since, --project, --agent)
3. `SearchEngine::search()` 호출
4. 결과 포맷팅:
   - text: 번호, 점수, 세션정보, snippet
   - json: SearchResult 배열

**`secall get <id>`**:
1. `id` 파싱: `session_id` 또는 `session_id:turn_index`
2. DB에서 세션 메타 조회
3. `--full`: vault에서 MD 파일 읽어서 출력
4. 기본: 메타데이터 + 턴 목록 요약

**`secall status`**:
1. DB 통계: 세션 수, 턴 수, FTS 인덱스 크기
2. 벡터 상태: 임베딩된 세션 수, Ollama 가용성
3. Vault 상태: 경로, 파일 수
4. 최근 ingest 로그

**`secall embed`**:
1. Ollama 가용성 확인
2. `--all`: 전체 재임베딩
3. 기본: 아직 임베딩 안 된 세션만
4. 진행률 표시 (N/M sessions)

### 3. 출력 포맷팅 (output.rs)

```rust
pub fn print_search_results(results: &[SearchResult], format: &OutputFormat) {
    match format {
        OutputFormat::Text => {
            for (i, r) in results.iter().enumerate() {
                println!("{}. [{}] {} — {} (score: {:.2})",
                    i + 1,
                    r.metadata.agent,
                    r.metadata.project.as_deref().unwrap_or("?"),
                    r.metadata.date,
                    r.score
                );
                println!("   Turn {}: {}", r.turn_index, r.snippet);
                if let Some(path) = &r.metadata.vault_path {
                    println!("   → {}", path);
                }
                println!();
            }
        },
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(results).unwrap());
        },
    }
}
```

### 4. 에러 처리

- Config 미설정 → "Run `secall init` first" 안내
- DB 없음 → 자동 생성 + 경고
- 세션 파일 없음 → 구체적 에러 메시지
- Ollama 미실행 → 경고 + BM25-only 모드 계속

## Dependencies

- Task 05 (Vault)
- Task 08 (SearchEngine)
- `clap` v4 (이미 Task 01에서 추가)

## Verification

```bash
# 빌드 성공
cd /Users/d9ng/privateProject/seCall && cargo build 2>&1 | tail -3

# CLI 도움말 출력
cargo run -- --help
cargo run -- ingest --help
cargo run -- recall --help

# init 동작
cargo run -- init --vault /tmp/secall-test-vault
ls -la /tmp/secall-test-vault/raw/sessions/
ls -la /tmp/secall-test-vault/SCHEMA.md
cat ~/.config/secall/config.toml

# status 동작 (빈 DB)
cargo run -- status

# ingest 동작 (실제 Claude Code 세션이 있다면)
cargo run -- ingest --auto 2>&1 | head -20

# recall 동작
cargo run -- recall "아키텍처 설계" --limit 5
cargo run -- recall "아키텍처 설계" --format json | python3 -m json.tool | head -30

# get 동작
cargo run -- get <session_id_from_recall>

# 전체 E2E: ingest → recall → get
SECALL_VAULT_PATH=/tmp/secall-e2e cargo run -- init --vault /tmp/secall-e2e && \
cargo run -- ingest --auto && \
cargo run -- recall "test" --limit 3
```

## Risks

- **`--auto` 모드에서 대량 세션 발견**: `~/.claude/projects/`에 수백 개 세션이 있으면 첫 실행이 매우 오래 걸림. `--auto --limit 10` 옵션 또는 확인 프롬프트 추가.
- **config.toml 경로가 OS별로 다름**: macOS는 `~/Library/Application Support/`, Linux는 `~/.config/`. `dirs` crate의 `config_dir()` 사용으로 해결하되, 문서에 양쪽 경로 명시.
- **클립보드/파이프 호환성**: `--format json`은 파이프로 전달 가능해야 함. stdout에 JSON만 출력, stderr에 로그/진행률.

## Scope Boundary

- MCP 서버 구현은 Task 10 (`Mcp` 서브커맨드는 stub으로 남겨둠)
- 인터랙티브 프롬프트 (fzf 스타일 선택)는 구현하지 않음
- 쉘 자동완성 생성은 구현하지 않음 (post-MVP)
