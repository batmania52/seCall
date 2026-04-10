---
type: task
status: draft
task_number: 1
plan: secall-p17-cli-git-branch
title: git branch 하드코딩 제거
depends_on: []
parallel_group: A
updated_at: 2026-04-10
---

# Task 01 — git branch 하드코딩 제거

## Changed files

1. `crates/secall-core/src/vault/config.rs:34-38` — `VaultConfig`에 `branch` 필드 추가
2. `crates/secall-core/src/vault/config.rs:126-135` — `Default for VaultConfig`에 `branch: "main"` 기본값
3. `crates/secall-core/src/vault/git.rs:8-11` — `VaultGit`에 `branch` 필드 추가, 생성자 수정
4. `crates/secall-core/src/vault/git.rs:71` — `symbolic-ref HEAD refs/heads/main` → `refs/heads/{branch}`
5. `crates/secall-core/src/vault/git.rs:99` — `pull --rebase origin main` → `origin {branch}`
6. `crates/secall-core/src/vault/git.rs:168` — `push origin main` → `origin {branch}`
7. `crates/secall-core/src/vault/git.rs:103` — `"Current branch main is up to date"` 문자열 수정
8. `crates/secall/src/commands/sync.rs` — `VaultGit::new` 호출부에 branch 전달
9. `crates/secall/src/commands/init.rs` — `VaultGit::new` 호출부에 branch 전달

## Change description

### Step 1: VaultConfig에 branch 필드 추가

`config.rs:34-38`의 `VaultConfig` struct에 추가:

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VaultConfig {
    pub path: PathBuf,
    #[serde(default)]
    pub git_remote: Option<String>,
    #[serde(default = "default_branch")]
    pub branch: String,
}

fn default_branch() -> String {
    "main".to_string()
}
```

`Default for VaultConfig`(126-135)에도 `branch: "main".to_string()` 추가.

### Step 2: VaultGit에 branch 전달

`git.rs`의 `VaultGit` struct에 `branch: String` 필드 추가. 생성자를 수정:

```rust
pub fn new(vault_path: &'a Path, branch: &str) -> Self {
    Self { vault_path, branch: branch.to_string() }
}
```

### Step 3: 하드코딩 3곳 교체

- `init()` 71행: `self.run_git(&["symbolic-ref", "HEAD", &format!("refs/heads/{}", self.branch)])?;`
- `pull()` 99행: `self.run_git(&["pull", "--rebase", "origin", &self.branch])?;`
- `push()` 168행: `self.run_git(&["push", "origin", &self.branch])?;`
- `pull()` 103행: `stdout.contains(&format!("Current branch {} is up to date", self.branch))`

### Step 4: 호출부 수정

`VaultGit::new(&vault_path)` 호출을 모두 `VaultGit::new(&vault_path, &config.vault.branch)`로 변경:

- `sync.rs` — `VaultGit::new` 호출 (여러 곳)
- `init.rs:35` — `VaultGit::new` 호출

### Step 5: 기존 config.toml 호환성

`serde(default = "default_branch")`로 처리하여 기존 config.toml에 `branch` 키가 없으면 `"main"` 기본값 적용. 기존 사용자는 변경 불필요.

## Dependencies

- 없음 (독립 태스크)

## Verification

```bash
# 1. 타입 체크
cargo check 2>&1 | tail -3

# 2. 전체 테스트 통과
cargo test 2>&1 | tail -10

# 3. config.toml에 branch 없는 경우 기본값 확인
cargo run -p secall -- status 2>&1 | head -5

# 4. config set으로 branch 변경 (Task 02 완료 후)
# Manual: config.toml에 branch = "master" 추가 후 sync --dry-run 실행 → "master" 브랜치 사용 확인

# 5. grep으로 하드코딩 "main" 제거 확인
grep -n '"main"' crates/secall-core/src/vault/git.rs | grep -v test | grep -v "// "
# 결과가 없어야 함 (기본값 설정 외에 "main" 직접 참조 없음)
```

## Risks

- **기존 VaultGit::new 호출부 누락**: `VaultGit::new`를 호출하는 모든 곳을 수정해야 함. 컴파일러가 시그니처 변경으로 잡아줌
- **기존 config.toml 호환성**: `serde(default)` 처리로 기존 파일에 `branch` 키가 없어도 정상 동작
- **pull 결과 파싱**: `"Current branch main is up to date"` 문자열이 브랜치 이름에 따라 달라지므로 동적 비교 필요

## Scope boundary — 수정 금지 파일

- `crates/secall-core/src/store/**` — DB 영역
- `crates/secall-core/src/search/**` — 검색 엔진
- `crates/secall-core/src/graph/**` — 그래프 영역
- `crates/secall-core/src/ingest/**` — 파서 영역
