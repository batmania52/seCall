---
type: task
status: draft
task_number: 4
plan: secall-p17-cli-git-branch
title: status 설정 요약 표시
depends_on: [2]
parallel_group: D
updated_at: 2026-04-10
---

# Task 04 — status 설정 요약 표시

## Changed files

1. `crates/secall/src/commands/status.rs:7-55` — 설정 요약 섹션 추가

## Change description

### Step 1: status.rs에 설정 요약 추가

기존 `run()` 함수의 출력에 설정 요약 섹션을 추가한다. 기존 "seCall Status" 헤더 직후, Index Statistics 이전에 삽입:

```
seCall Status
=============
Config: ~/.config/secall/config.toml
DB:     ~/.cache/secall/index.sqlite
Vault:  /Users/user/obsidian-vault/seCall

Settings:
  tokenizer  = lindera
  embedding  = ollama (bge-m3)
  branch     = main
  timezone   = Asia/Seoul

Index Statistics:
  Sessions:      127
  Turns:         3842
  Embedded:      3842

Vault Files:     127 session markdown files

Recent Ingests (last 5):
  ...
```

구현:

```rust
// Settings summary (after vault path, before index stats)
println!("Settings:");
println!("  tokenizer  = {}", config.search.tokenizer);

let embedding_detail = match config.embedding.backend.as_str() {
    "ollama" => {
        let model = config.embedding.ollama_model
            .as_deref()
            .unwrap_or("bge-m3");
        format!("ollama ({})", model)
    }
    "ort" => "ort (local ONNX)".to_string(),
    "openai" => {
        let model = config.embedding.openai_model
            .as_deref()
            .unwrap_or("text-embedding-3-large");
        format!("openai ({})", model)
    }
    other => other.to_string(),
};
println!("  embedding  = {}", embedding_detail);

if config.vault.git_remote.is_some() {
    println!("  branch     = {}", config.vault.branch);
}
println!("  timezone   = {}", config.output.timezone);
println!();
```

### Step 2: git_remote 유무에 따른 조건부 표시

- `git_remote`이 설정된 경우만 `branch` 표시 (git 미사용 시 불필요)
- `embedding.backend = "none"`이면 "(벡터 검색 비활성화)" 표시

## Dependencies

- Task 02 (`secall config` 서브커맨드 — 직접 의존은 없으나, config show와 status의 출력이 일관되어야 함)
- Task 01 (VaultConfig에 branch 필드 존재해야 참조 가능)

## Verification

```bash
# 1. 타입 체크
cargo check 2>&1 | tail -3

# 2. status 출력에 Settings 섹션 포함 확인
cargo run -p secall -- status 2>&1 | grep -A 5 "Settings:"
# 출력에 tokenizer, embedding, timezone 포함

# 3. 전체 테스트
cargo test 2>&1 | tail -10
```

## Risks

- **최소 변경**: 기존 status.rs에 println 추가만으로 구현. 기존 출력 순서에 영향 없음
- **Config 로드 실패**: `Config::load_or_default()` 사용으로 config 없어도 기본값 표시

## Scope boundary — 수정 금지 파일

- `crates/secall-core/src/vault/config.rs` — Task 01 영역
- `crates/secall/src/commands/config.rs` — Task 02 영역
- `crates/secall/src/commands/init.rs` — Task 03 영역
- `crates/secall-core/src/store/**` — DB 영역
- `crates/secall-core/src/search/**` — 검색 엔진
