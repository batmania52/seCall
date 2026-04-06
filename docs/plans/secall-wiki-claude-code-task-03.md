---
type: task
status: draft
plan: secall-wiki-claude-code
task_number: 3
title: "secall wiki CLI 커맨드"
parallel_group: B
depends_on: [1, 2]
updated_at: 2026-04-06
---

# Task 03: secall wiki CLI 커맨드

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall/src/commands/wiki.rs` | **신규 생성** | wiki 서브커맨드 핸들러 |
| `crates/secall/src/commands/mod.rs` | 수정 | `pub mod wiki;` 추가 |
| `crates/secall/src/main.rs` | 수정 | `Wiki` 서브커맨드 추가 |

## Change description

### 1. CLI 서브커맨드 정의 (main.rs)

```rust
/// Manage wiki generation via Claude Code meta-agent
Wiki {
    #[command(subcommand)]
    action: WikiAction,
},

#[derive(Subcommand)]
enum WikiAction {
    /// Run wiki update using Claude Code as meta-agent
    Update {
        /// Model: opus or sonnet
        #[arg(long, default_value = "sonnet")]
        model: String,

        /// Only process sessions since this date (YYYY-MM-DD)
        #[arg(long)]
        since: Option<String>,

        /// Incremental mode: update for a specific session
        #[arg(long)]
        session: Option<String>,

        /// Print the prompt without executing Claude Code
        #[arg(long)]
        dry_run: bool,
    },

    /// Show wiki status (page count, last update)
    Status,
}
```

### 2. wiki update 실행 흐름 (commands/wiki.rs)

```rust
pub async fn run_update(
    model: &str,
    since: Option<&str>,
    session: Option<&str>,
    dry_run: bool,
) -> Result<()> {
    // 1. wiki/ 디렉토리 존재 확인
    let config = Config::load_or_default();
    let wiki_dir = config.vault.path.join("wiki");
    if !wiki_dir.exists() {
        anyhow::bail!("wiki/ directory not found. Run `secall init` first.");
    }

    // 2. 프롬프트 로드
    let prompt = if let Some(sid) = session {
        load_incremental_prompt(sid)?
    } else {
        load_batch_prompt(since)?
    };

    // 3. dry-run이면 프롬프트만 출력하고 종료
    if dry_run {
        println!("{prompt}");
        return Ok(());
    }

    // 4. claude CLI 존재 확인
    if !command_exists("claude") {
        anyhow::bail!(
            "Claude Code CLI not found in PATH. \
             Install: https://docs.anthropic.com/claude-code"
        );
    }

    // 5. Claude Code 실행
    let model_id = match model {
        "opus" => "claude-opus-4-6",
        _ => "claude-sonnet-4-6",
    };

    let status = std::process::Command::new("claude")
        .args(["-p", &prompt])
        .args(["--model", model_id])
        .arg("--allowedTools")
        .arg("mcp__secall__recall,mcp__secall__get,mcp__secall__status,Read,Write,Edit,Glob,Grep")
        .status()?;

    if status.success() {
        eprintln!("✓ Wiki update complete.");
    } else {
        eprintln!("⚠ Claude Code exited with code: {:?}", status.code());
    }

    Ok(())
}
```

### 3. 프롬프트 로드 순서

1. `$SECALL_PROMPTS_DIR/wiki-update.md` (환경변수)
2. `~/.config/secall/prompts/wiki-update.md` (사용자 커스텀)
3. 바이너리 내장 기본 프롬프트 (`include_str!("../../../docs/prompts/wiki-update.md")`)

사용자가 프롬프트를 커스텀할 수 있도록 파일 시스템 우선:

```rust
fn load_batch_prompt(since: Option<&str>) -> Result<String> {
    let custom_path = prompt_dir().join("wiki-update.md");
    let mut prompt = if custom_path.exists() {
        std::fs::read_to_string(&custom_path)?
    } else {
        include_str!("../../../docs/prompts/wiki-update.md").to_string()
    };

    if let Some(since) = since {
        prompt.push_str(&format!(
            "\n\n## 추가 조건\n- `--since {since}` 이후 세션만 검색하세요.\n"
        ));
    }

    Ok(prompt)
}

fn load_incremental_prompt(session_id: &str) -> Result<String> {
    let custom_path = prompt_dir().join("wiki-incremental.md");
    let template = if custom_path.exists() {
        std::fs::read_to_string(&custom_path)?
    } else {
        include_str!("../../../docs/prompts/wiki-incremental.md").to_string()
    };

    // 환경변수 치환
    Ok(template
        .replace("{SECALL_SESSION_ID}", session_id)
        .replace("{SECALL_AGENT}", &std::env::var("SECALL_AGENT").unwrap_or_default())
        .replace("{SECALL_PROJECT}", &std::env::var("SECALL_PROJECT").unwrap_or_default())
        .replace("{SECALL_DATE}", &std::env::var("SECALL_DATE").unwrap_or_default()))
}

fn prompt_dir() -> PathBuf {
    if let Ok(p) = std::env::var("SECALL_PROMPTS_DIR") {
        return PathBuf::from(p);
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("secall")
        .join("prompts")
}
```

### 4. wiki status 서브커맨드

```rust
pub fn run_status() -> Result<()> {
    let config = Config::load_or_default();
    let wiki_dir = config.vault.path.join("wiki");

    if !wiki_dir.exists() {
        println!("Wiki not initialized. Run `secall init`.");
        return Ok(());
    }

    let mut page_count = 0;
    for entry in walkdir::WalkDir::new(&wiki_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().extension().map(|e| e == "md").unwrap_or(false) {
            page_count += 1;
        }
    }

    println!("Wiki: {}", wiki_dir.display());
    println!("Pages: {page_count}");
    Ok(())
}
```

### 5. command_exists 헬퍼

```rust
fn command_exists(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
```

## Dependencies

- Task 01 (wiki/ 디렉토리 존재해야 함)
- Task 02 (프롬프트 파일 존재해야 함)

## Verification

```bash
# 타입 체크
cargo check

# CLI 서브커맨드 등록 확인
cargo run -p secall -- wiki --help
cargo run -p secall -- wiki update --help
cargo run -p secall -- wiki status

# dry-run 테스트 (Claude Code 실행 안 함)
cargo run -p secall -- wiki update --dry-run

# 전체 테스트 회귀
cargo test
```

## Risks

- **Claude Code CLI 인터페이스 변경**: `claude -p`, `--model`, `--allowedTools` 플래그가 버전에 따라 변경될 수 있음
- **include_str! 경로**: cargo workspace 루트 기준 상대 경로. 빌드 환경에 따라 확인 필요
- **장시간 실행**: Opus로 대량 세션 처리 시 수 분~수십 분 소요 가능

## Scope Boundary

수정 금지 파일:
- `commands/ingest.rs` — ingest 로직 변경 금지
- `mcp/*` — MCP 서버 변경 금지
- `search/*` — 검색 모듈 변경 금지
- `vault/init.rs` — Task 01 영역
