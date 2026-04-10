---
type: task
status: draft
task_number: 2
plan: secall-p17-cli-git-branch
title: secall config 서브커맨드
depends_on: [1]
parallel_group: B
updated_at: 2026-04-10
---

# Task 02 — `secall config` 서브커맨드

## Changed files

1. `crates/secall/src/commands/config.rs` — **신규 파일**. `run_show()`, `run_set()`, `run_path()` 핸들러
2. `crates/secall/src/commands/mod.rs:1` — `pub mod config;` 추가
3. `crates/secall/src/main.rs:20-179` — `Commands::Config` variant + `ConfigAction` enum + match arm 추가

## Change description

### Step 1: main.rs에 Config 서브커맨드 등록

`Commands` enum에 추가:

```rust
/// View or modify configuration
Config {
    #[command(subcommand)]
    action: ConfigAction,
},
```

`ConfigAction` enum:

```rust
#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set a configuration value
    Set {
        /// Config key (e.g. search.tokenizer, embedding.backend)
        key: String,
        /// New value
        value: String,
    },
    /// Show config file path
    Path,
}
```

match arm:

```rust
Commands::Config { action } => match action {
    ConfigAction::Show => commands::config::run_show()?,
    ConfigAction::Set { key, value } => commands::config::run_set(&key, &value)?,
    ConfigAction::Path => commands::config::run_path()?,
},
```

### Step 2: config.rs 핸들러 구현

**`run_show()`**: Config::load_or_default()로 로드 후 보기 좋게 출력:

```
seCall Configuration
====================
Config file: ~/.config/secall/config.toml

[vault]
  path       = /Users/user/obsidian-vault/seCall
  git_remote = https://github.com/user/vault.git
  branch     = main

[search]
  tokenizer     = lindera
  default_limit = 10

[embedding]
  backend      = ollama
  ollama_model = bge-m3

[output]
  timezone = UTC
```

**`run_set(key, value)`**: 지원 키와 유효성 검증:

| Key | 유효 값 | 검증 |
|-----|--------|------|
| `vault.path` | 경로 문자열 | 디렉토리 존재 확인 (경고만, 차단 안함) |
| `vault.git_remote` | URL 문자열 | 없음 |
| `vault.branch` | 브랜치 이름 | 없음 |
| `search.tokenizer` | `lindera`, `kiwi` | 값 목록 검증. Windows에서 `kiwi` 선택 시 경고 |
| `search.default_limit` | 양수 정수 | 파싱 검증 |
| `embedding.backend` | `ollama`, `ort`, `openai`, `none` | 값 목록 검증 |
| `embedding.ollama_url` | URL 문자열 | 없음 |
| `embedding.ollama_model` | 모델 이름 | 없음 |
| `output.timezone` | IANA timezone | `chrono_tz` 파싱 검증 |

잘못된 키 입력 시:

```
Error: unknown config key "search.foo"

Available keys:
  vault.path, vault.git_remote, vault.branch
  search.tokenizer, search.default_limit
  embedding.backend, embedding.ollama_url, embedding.ollama_model
  output.timezone
```

구현 접근법:

```rust
pub fn run_set(key: &str, value: &str) -> Result<()> {
    let mut config = Config::load_or_default();

    match key {
        "vault.path" => {
            let path = PathBuf::from(shellexpand::tilde(value).to_string());
            if !path.exists() {
                eprintln!("Warning: directory does not exist: {}", path.display());
            }
            config.vault.path = path;
        }
        "vault.git_remote" => config.vault.git_remote = Some(value.to_string()),
        "vault.branch" => config.vault.branch = value.to_string(),
        "search.tokenizer" => {
            if !["lindera", "kiwi"].contains(&value) {
                anyhow::bail!("invalid tokenizer: '{}'. Valid: lindera, kiwi", value);
            }
            config.search.tokenizer = value.to_string();
        }
        "search.default_limit" => {
            let n: usize = value.parse().context("default_limit must be a positive integer")?;
            config.search.default_limit = n;
        }
        "embedding.backend" => {
            if !["ollama", "ort", "openai", "none"].contains(&value) {
                anyhow::bail!("invalid backend: '{}'. Valid: ollama, ort, openai, none", value);
            }
            config.embedding.backend = value.to_string();
        }
        "embedding.ollama_url" => config.embedding.ollama_url = Some(value.to_string()),
        "embedding.ollama_model" => config.embedding.ollama_model = Some(value.to_string()),
        "output.timezone" => {
            value.parse::<chrono_tz::Tz>()
                .map_err(|_| anyhow::anyhow!("invalid timezone: '{}'. Use IANA format (e.g. Asia/Seoul)", value))?;
            config.output.timezone = value.to_string();
        }
        _ => {
            anyhow::bail!(
                "unknown config key: '{}'\n\nAvailable keys:\n  \
                vault.path, vault.git_remote, vault.branch\n  \
                search.tokenizer, search.default_limit\n  \
                embedding.backend, embedding.ollama_url, embedding.ollama_model\n  \
                output.timezone",
                key
            );
        }
    }

    config.save()?;
    println!("✓ Set {} = {}", key, value);
    Ok(())
}
```

**`run_path()`**: `Config::config_path()` 출력.

### Step 3: mod.rs에 모듈 등록

`commands/mod.rs`에 `pub mod config;` 추가.

## Dependencies

- Task 01 (git branch 필드가 VaultConfig에 추가되어야 `config set vault.branch` 가능)

## Verification

```bash
# 1. 타입 체크
cargo check 2>&1 | tail -3

# 2. config show 실행
cargo run -p secall -- config show 2>&1

# 3. config path 실행
cargo run -p secall -- config path 2>&1

# 4. config set 성공
cargo run -p secall -- config set output.timezone Asia/Seoul 2>&1
# 출력: ✓ Set output.timezone = Asia/Seoul

# 5. config set 실패 — 잘못된 키
cargo run -p secall -- config set foo.bar baz 2>&1
# 출력에 "unknown config key" 포함

# 6. config set 실패 — 잘못된 값
cargo run -p secall -- config set search.tokenizer invalid 2>&1
# 출력에 "invalid tokenizer" 포함

# 7. config set 후 show에서 반영 확인
cargo run -p secall -- config show 2>&1 | grep timezone
# 출력: timezone = Asia/Seoul

# 8. 전체 테스트
cargo test 2>&1 | tail -10
```

## Risks

- **Config::save() 포맷 변경**: `toml::to_string_pretty`가 기존 주석을 제거함. seCall은 주석 없는 config.toml 사용 중이므로 문제없음
- **shellexpand 의존성**: `secall` crate에 이미 `shellexpand = "3"` 있음 (Cargo.toml:17). `secall-core`에는 없으므로 핸들러에서만 사용
- **chrono-tz 의존성**: `secall-core`에 이미 있음. `secall` crate에서는 `secall_core` 재수출 사용

## Scope boundary — 수정 금지 파일

- `crates/secall-core/src/vault/config.rs` — Task 01에서 수정, 이 태스크에서는 읽기만
- `crates/secall-core/src/vault/git.rs` — Task 01 영역
- `crates/secall-core/src/store/**` — DB 영역
- `crates/secall-core/src/search/**` — 검색 엔진
- `crates/secall/src/commands/init.rs` — Task 03 영역
