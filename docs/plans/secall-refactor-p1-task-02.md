---
type: task
status: draft
plan: secall-refactor-p1
task_number: 2
title: "db.rs Result 반환 전환"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 02: db.rs Result 반환 전환

## 문제

`crates/secall-core/src/store/db.rs`에 35+ `unwrap_or`/`.ok()` 패턴이 있어 DB 에러가 무조건 기본값으로 대체됨. 호출자가 에러를 인지할 방법이 없음.

### 영향 받는 메서드 (12개)

| 메서드 | 현재 반환 | 에러 패턴 수 | 호출자 |
|--------|-----------|-------------|--------|
| `get_stats()` | `Result<DbStats>` (내부에서 삼킴) | 5+ | `status.rs:25` |
| `count_sessions()` | `i64` | 1 | `lint.rs:70` |
| `list_projects()` | `Vec<String>` | 3 | (미사용/status) |
| `list_agents()` | `Vec<String>` | 3 | (미사용/status) |
| `has_embeddings()` | `bool` | 2 | `status.rs` |
| `list_session_vault_paths()` | `Vec<(...)>` | 3 | `lint.rs:96` |
| `count_fts_rows()` | `i64` | 1 | `lint.rs:202` |
| `count_turns()` | `i64` | 1 | `lint.rs:201` |
| `find_sessions_without_vectors()` | `Vec<String>` | 5 | `lint.rs:186` |
| `find_orphan_vectors()` | `Vec<(...)>` | 4 | `lint.rs:221` |
| `agent_counts()` | `HashMap<...>` | 3 | `lint.rs:67` |
| `list_all_session_ids()` | `Vec<String>` | 3 | `lint.rs:340` |
| `find_duplicate_ingest_entries()` | `Vec<(...)>` | 3 | `lint.rs:169` |

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/store/db.rs:156-341` | 수정 | 12개 메서드 반환 타입 → `Result<T>` |
| `crates/secall-core/src/ingest/lint.rs:54-87,96,169,186,201-202,221,340` | 수정 | 호출자 에러 처리 추가 |
| `crates/secall/src/commands/status.rs:25-30` | 수정 | `get_stats()` 호출 에러 처리 |

## Change description

### 원칙

- **쓰기 메서드** (`insert_session`, `update_session_vault_path` 등): 이미 `Result` 반환 → 변경 없음
- **읽기 메서드** (lint 헬퍼): `Result<T>` 반환으로 변경. 호출자가 `?` 전파 또는 기본값 결정
- **get_stats()**: 내부 `unwrap_or(0)` → `?` 전파. 이미 `Result<DbStats>` 반환이므로 시그니처 변경 없음

### Step 1: 단순 카운트 메서드 (count_sessions, count_turns, count_fts_rows)

```rust
// 변경 전 (db.rs:156-159)
pub fn count_sessions(&self) -> i64 {
    self.conn
        .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
        .unwrap_or(0)
}

// 변경 후
pub fn count_sessions(&self) -> Result<i64> {
    let count = self.conn
        .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))?;
    Ok(count)
}
```

> `count_turns()` (line 228-232), `count_fts_rows()` (line 221-225)도 동일 패턴.

### Step 2: 리스트 메서드 (list_projects, list_agents, list_session_vault_paths, list_all_session_ids)

```rust
// 변경 전 (db.rs:162-171)
pub fn list_projects(&self) -> Vec<String> {
    let mut stmt = match self.conn.prepare("SELECT DISTINCT project FROM sessions WHERE project IS NOT NULL") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    stmt.query_map([], |r| r.get(0))
        .ok()
        .map(|iter| iter.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}

// 변경 후
pub fn list_projects(&self) -> Result<Vec<String>> {
    let mut stmt = self.conn.prepare(
        "SELECT DISTINCT project FROM sessions WHERE project IS NOT NULL"
    )?;
    let rows = stmt.query_map([], |r| r.get(0))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}
```

> 개별 row 파싱 실패는 `filter_map(|r| r.ok())`로 계속 스킵 (row 단위 에러는 치명적이지 않음).
> prepare/query_map 실패는 `?`로 전파 (DB 연결 문제 = 치명적).

### Step 3: has_embeddings()

```rust
// 변경 전 (db.rs:184-201)
pub fn has_embeddings(&self) -> bool {
    let exists: i64 = self.conn.query_row(...).unwrap_or(0);
    if exists == 0 { return false; }
    let count: i64 = self.conn.query_row(...).unwrap_or(0);
    count > 0
}

// 변경 후
pub fn has_embeddings(&self) -> Result<bool> {
    let exists: i64 = self.conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='turn_vectors'",
        [], |r| r.get(0),
    )?;
    if exists == 0 {
        return Ok(false);
    }
    let count: i64 = self.conn.query_row(
        "SELECT COUNT(*) FROM turn_vectors", [], |r| r.get(0),
    )?;
    Ok(count > 0)
}
```

### Step 4: 복합 메서드 (find_sessions_without_vectors, find_orphan_vectors)

```rust
// 변경 전 (db.rs:235-269)
pub fn find_sessions_without_vectors(&self) -> Vec<String> {
    let table_exists: i64 = self.conn.query_row(...).unwrap_or(0);
    if table_exists == 0 {
        let mut stmt = match self.conn.prepare(...) { ... };
        return stmt.query_map(...).ok()...unwrap_or_default();
    }
    ...
}

// 변경 후
pub fn find_sessions_without_vectors(&self) -> Result<Vec<String>> {
    let table_exists: i64 = self.conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='turn_vectors'",
        [], |r| r.get(0),
    )?;

    let query = if table_exists == 0 {
        "SELECT id FROM sessions"
    } else {
        "SELECT id FROM sessions WHERE id NOT IN (SELECT DISTINCT session_id FROM turn_vectors)"
    };

    let mut stmt = self.conn.prepare(query)?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}
```

### Step 5: agent_counts()

```rust
// 변경 전 (db.rs:299-315)
pub fn agent_counts(&self) -> std::collections::HashMap<String, usize> {
    let mut stmt = match ... { Err(_) => return HashMap::new() };
    ...
}

// 변경 후
pub fn agent_counts(&self) -> Result<std::collections::HashMap<String, usize>> {
    let mut stmt = self.conn.prepare(
        "SELECT agent, COUNT(*) FROM sessions GROUP BY agent"
    )?;
    let rows = stmt.query_map([], |row| {
        let agent: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        Ok((agent, count as usize))
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}
```

### Step 6: get_stats() 내부 에러 전파

```rust
// 변경 전 (db.rs:78-138) — 이미 Result<DbStats> 반환이지만 내부에서 삼킴
let session_count: i64 = self.conn.query_row(...).unwrap_or(0);
let turn_count: i64 = self.conn.query_row(...).unwrap_or(0);

// 변경 후
let session_count: i64 = self.conn.query_row(
    "SELECT COUNT(*) FROM sessions", [], |r| r.get(0)
)?;
let turn_count: i64 = self.conn.query_row(
    "SELECT COUNT(*) FROM turns", [], |r| r.get(0)
)?;
```

> `recent_ingests` 쿼리의 `.ok()` + `.unwrap_or_default()` 패턴도 `?` 전파로 변환.

### Step 7: lint.rs 호출자 수정

```rust
// 변경 전 (lint.rs:67)
let agents = db.agent_counts();

// 변경 후
let agents = db.agent_counts()?;

// 변경 전 (lint.rs:70)
total_sessions: db.count_sessions(),

// 변경 후
total_sessions: db.count_sessions()?,
```

> `lint.rs`의 `run_lint()` 함수가 이미 `Result<LintReport>`를 반환하므로 `?` 전파 가능.

### Step 8: status.rs 호출자 수정

```rust
// status.rs:25 — 이미 Result 사용 중
let stats = db.get_stats()?;
// get_stats 내부만 변경되므로 status.rs는 변경 불필요할 수 있음.
// has_embeddings() 호출이 있다면 확인 필요.
```

## Dependencies

- 없음 (독립 실행 가능)
- Task 01 (ingest error)과 병렬 실행 가능

## Verification

```bash
# 1. 컴파일 확인 (반환 타입 변경 후 호출자 전체 검증)
cargo check

# 2. db.rs 테스트
cargo test -p secall-core -- db

# 3. lint 테스트
cargo test -p secall-core lint

# 4. 전체 테스트 회귀 없음
cargo test
```

> **[Developer 필수]** subtask-done 시그널 전에 위 명령의 실행 결과를 result 문서에 기록하세요. 형식: `✅ 명령 — exit 0` 또는 `❌ 명령 — 에러 내용 (사유)`. 검증 증빙 미제출 시 리뷰에서 conditional 처리됩니다.

## Risks

- **호출자 변경 범위**: 12개 메서드의 반환 타입 변경 시 lint.rs, status.rs 등 모든 호출자를 업데이트해야 함. `cargo check`가 누락된 호출자를 잡아줌.
- **점진적 변환 권장**: 한 메서드씩 변환 → `cargo check` → 다음 메서드. 일괄 변환 시 컴파일 에러 폭주 가능.
- **find_duplicate_ingest_entries**: `lint.rs:169`에서 호출. 반환 타입 변경 시 `?` 추가 필요.

## Scope boundary

다음 파일은 영향을 받을 수 있으나 이 task에서 수정하지 않음:
- `crates/secall-core/src/search/bm25.rs:196-346` — `Database impl` 블록의 insert/search 메서드. 이미 `Result` 반환.
- `crates/secall-core/src/search/vector.rs` — `VectorIndexer` 메서드. 이미 `Result` 반환.
- `crates/secall/src/commands/get.rs` — `db.session_exists()`, `db.get_session_meta()` 이미 `Result` 반환.
