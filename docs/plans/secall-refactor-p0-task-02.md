---
type: task
status: draft
plan: secall-refactor-p0
task_number: 2
title: "vault_path 상대경로 전환"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 02: vault_path 상대경로 전환

## 문제

`vault::Vault::write_session()` (vault/mod.rs:59)이 절대경로(`abs_path`)를 반환.
`ingest.rs:82`에서 이 절대경로를 DB `sessions.vault_path`에 그대로 저장.
`get.rs:32`에서 `config.vault.path.join(vault_path)` → 절대경로에 vault root를 다시 join.

**현재 동작**: Rust의 `Path::join()`이 절대경로를 인자로 받으면 기존 경로를 무시하므로 *우연히* 동작하지만, vault 경로가 바뀌면 DB에 저장된 절대경로가 무효화됨. 설계상 명확한 결함.

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/vault/mod.rs:37-60` | 수정 | `write_session()` — 반환값을 `rel_path`로 변경 |
| `crates/secall/src/commands/ingest.rs:69-83` | 수정 | 반환된 상대경로를 DB에 저장, 출력용 절대경로는 별도 계산 |
| `crates/secall/src/commands/get.rs:28-38` | 수정 | join 로직 그대로 유지 (상대경로 join이 정상 동작) |
| `crates/secall-core/src/store/db.rs` | 추가 | `migrate_vault_paths_to_relative()` 마이그레이션 함수 |
| `crates/secall-core/src/ingest/lint.rs:96-119` | 수정 | L001 검사에서 상대경로 → 절대경로 변환 추가 |

## Change description

### Step 1: write_session() 반환값 변경

```rust
// vault/mod.rs:37-60 — 변경 전
pub fn write_session(&self, session: &Session) -> Result<PathBuf> {
    let rel_path = session_vault_path(session);
    let abs_path = self.path.join(&rel_path);
    // ... write file ...
    Ok(abs_path)    // ← 절대경로 반환
}

// 변경 후
pub fn write_session(&self, session: &Session) -> Result<PathBuf> {
    let rel_path = session_vault_path(session);
    let abs_path = self.path.join(&rel_path);
    // ... write file (동일) ...
    Ok(rel_path)    // ← 상대경로 반환
}
```

> `rel_path`는 이미 `session_vault_path()`가 생성하는 값: `raw/sessions/2026-04-06/claude-code_seCall_a1b2c3d4.md`

### Step 2: ingest.rs에서 상대경로 저장 + 출력용 절대경로 분리

```rust
// ingest.rs:69-85 — 변경 전
let md_path = match vault.write_session(&session) {
    Ok(p) => p,     // abs_path
    Err(e) => { ... }
};
let vault_path_str = md_path.to_string_lossy();
let _ = db.update_session_vault_path(&session.id, &vault_path_str);
print_ingest_result(&session, &md_path, &stats, format);

// 변경 후
let rel_path = match vault.write_session(&session) {
    Ok(p) => p,     // rel_path
    Err(e) => { ... }
};
let vault_path_str = rel_path.to_string_lossy();
let _ = db.update_session_vault_path(&session.id, &vault_path_str);
let abs_path = config.vault.path.join(&rel_path);
print_ingest_result(&session, &abs_path, &stats, format);
```

> `print_ingest_result()`과 `run_post_ingest_hook()`에는 절대경로를 전달해야 함 (파일 시스템 접근용).

### Step 3: post-ingest hook에도 절대경로 전달

```rust
// ingest.rs:89 — 변경 전
let _ = run_post_ingest_hook(&config, &session, &md_path);

// 변경 후
let _ = run_post_ingest_hook(&config, &session, &abs_path);
```

### Step 4: get.rs는 변경 없음 확인

```rust
// get.rs:32 — 변경 불필요
let abs_path = config.vault.path.join(vault_path);
// vault_path가 "raw/sessions/2026-04-06/..." (상대경로)이면 정상 동작
```

> 기존 코드가 이미 `config.vault.path.join(vault_path)` 패턴. 상대경로로 전환되면 의도대로 동작.

### Step 5: L001 lint 검사에서 상대경로 처리

```rust
// lint.rs:96-119 — 변경 전
for (session_id, vault_path) in db.list_session_vault_paths() {
    match vault_path {
        Some(ref path) => {
            if !std::path::Path::new(path).exists() {  // ← 절대경로 가정
                // L001 finding
            }
        }
        ...
    }
}

// 변경 후
for (session_id, vault_path) in db.list_session_vault_paths() {
    match vault_path {
        Some(ref path) => {
            let check_path = if Path::new(path).is_absolute() {
                PathBuf::from(path)       // 마이그레이션 전 레거시 경로
            } else {
                config.vault.path.join(path)  // 새 상대경로
            };
            if !check_path.exists() {
                // L001 finding
            }
        }
        ...
    }
}
```

> L001은 `config` 파라미터가 `_config`로 무시되고 있음 → `config`로 변경하여 vault path resolve에 사용.

### Step 6: DB 마이그레이션 함수 추가

```rust
// db.rs — 새 메서드 추가
impl Database {
    /// 기존 절대경로 vault_path를 상대경로로 변환
    pub fn migrate_vault_paths_to_relative(&self, vault_root: &Path) -> Result<usize> {
        let vault_root_str = vault_root.to_string_lossy();
        let prefix = format!("{}/", vault_root_str.trim_end_matches('/'));

        let mut stmt = self.conn().prepare(
            "SELECT id, vault_path FROM sessions WHERE vault_path IS NOT NULL"
        )?;

        let rows: Vec<(String, String)> = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?.filter_map(|r| r.ok()).collect();

        let mut migrated = 0;
        for (session_id, vault_path) in &rows {
            if vault_path.starts_with(&prefix) {
                let relative = &vault_path[prefix.len()..];
                self.conn().execute(
                    "UPDATE sessions SET vault_path = ?1 WHERE id = ?2",
                    rusqlite::params![relative, session_id],
                )?;
                migrated += 1;
            }
        }
        Ok(migrated)
    }
}
```

> 이 함수는 `secall lint --fix` 또는 별도 마이그레이션 커맨드에서 호출.
> `ingest.rs`의 ingest 루프 시작 전에 한 번 호출하는 것도 고려 가능.

### Step 7: vault/mod.rs 테스트 수정

기존 `test_write_session_creates_file` 테스트에서 반환값이 상대경로로 바뀌므로:

```rust
#[test]
fn test_write_session_returns_relative_path() {
    let dir = TempDir::new().unwrap();
    let vault = Vault::new(dir.path().to_path_buf());
    vault.init().unwrap();
    let session = make_session();
    let rel_path = vault.write_session(&session).unwrap();

    // 상대경로 확인
    assert!(rel_path.is_relative());
    assert!(rel_path.starts_with("raw/sessions/"));

    // 절대경로로 합성 시 파일 존재 확인
    let abs_path = dir.path().join(&rel_path);
    assert!(abs_path.exists());
}
```

## Dependencies

- 없음 (독립 실행 가능)

## Verification

```bash
# 1. 컴파일 확인
cargo check

# 2. vault 테스트 통과
cargo test -p secall-core vault

# 3. lint 테스트 통과
cargo test -p secall-core lint

# 4. 전체 테스트 회귀 없음
cargo test

# 5. 수동 round-trip 확인
# secall ingest --auto → secall get <session_id> --full → 파일 내용 출력 확인
```

## Risks

- **기존 DB 절대경로**: `migrate_vault_paths_to_relative()` 실행 전까지 기존 데이터의 `secall get --full`은 현재와 동일하게 동작 (Path::join이 절대경로를 그대로 사용). 마이그레이션 후에야 vault root 변경에 강건해짐.
- **print_ingest_result 경로**: 출력에 절대경로를 표시해야 사용자가 파일 위치를 알 수 있음. `rel_path`를 출력하면 혼란 → Step 2에서 `abs_path`를 별도 계산하여 전달.
- **hooks**: `run_post_ingest_hook()`에 전달하는 경로도 절대경로여야 함. Step 3에서 처리.

## Scope boundary

다음 파일은 영향을 받을 수 있으나 이 task에서 수정하지 않음:
- `crates/secall-core/src/vault/index.rs` — index.md에 기록하는 경로는 이미 `rel_path` 사용
- `crates/secall-core/src/vault/log.rs` — log.md에 기록하는 경로도 이미 `rel_path` 사용
- `crates/secall-core/src/mcp/tools.rs` — SessionMeta.vault_path를 표시만 함 (join 로직 없음)
