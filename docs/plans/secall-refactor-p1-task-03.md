---
type: task
status: draft
plan: secall-refactor-p1
task_number: 3
title: "ingest 트랜잭션 래핑"
parallel_group: B
depends_on: [1, 2]
updated_at: 2026-04-06
---

# Task 03: ingest 트랜잭션 래핑

## 문제

`ingest.rs:68-89`에서 3개 작업이 독립 실행됨:
1. `vault.write_session()` → 파일시스템 쓰기
2. `engine.index_session()` → DB INSERT (sessions, turns, turns_fts, turn_vectors)
3. `db.update_session_vault_path()` → DB UPDATE

작업 2 또는 3이 실패하면:
- vault 파일은 남아있으나 DB에 레코드 없음 → orphan vault file
- sessions 레코드는 있으나 vault_path 없음 → `secall get --full` 실패

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/store/db.rs` | 추가 | `with_transaction()` 헬퍼 메서드 |
| `crates/secall-core/src/search/bm25.rs:64-98` | 수정 | `index_session()` — `&Database` 대신 `&Connection` 또는 트랜잭션 지원 |
| `crates/secall/src/commands/ingest.rs:48-96` | 수정 | ingest 루프에 트랜잭션 래핑 |

## Change description

### 설계 결정

**파일시스템은 트랜잭션 밖에 둔다.** SQLite 트랜잭션은 DB 작업만 커버. vault 파일 쓰기는 트랜잭션 전에 실행하고, DB 작업 실패 시 vault 파일을 cleanup.

```
vault.write_session() → 파일 생성
BEGIN TRANSACTION
  engine.index_session()           ← sessions, turns, turns_fts, turn_vectors INSERT
  db.update_session_vault_path()   ← sessions UPDATE
COMMIT
성공 → 완료
실패 → ROLLBACK + vault 파일 삭제
```

### Step 1: Database에 트랜잭션 헬퍼 추가

```rust
// db.rs — 새 메서드
impl Database {
    /// Execute a closure within a SQLite transaction.
    /// Commits on Ok, rolls back on Err.
    pub fn with_transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        self.conn.execute_batch("BEGIN")?;
        match f() {
            Ok(val) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(val)
            }
            Err(e) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }
}
```

> rusqlite의 `Transaction` 타입은 `&mut Connection`을 요구하지만 현재 `Database`는 `&self`만 노출. 수동 `BEGIN/COMMIT/ROLLBACK`으로 우회.
> 향후 `Database`가 `&mut self`를 노출하면 rusqlite `Transaction`으로 전환 가능.

### Step 2: ingest 루프 수정

```rust
// ingest.rs:68-96 — 변경 전
let md_path = match vault.write_session(&session) { ... };
let stats = engine.index_session(&db, &session).await.unwrap_or_default();
let _ = db.update_session_vault_path(&session.id, &vault_path_str);

// 변경 후
// 1. vault 파일 쓰기 (트랜잭션 밖)
let md_path = match vault.write_session(&session) {
    Ok(p) => p,
    Err(e) => {
        eprintln!("warn: vault write failed for {}: {e}", session_path.display());
        errors += 1;
        continue;
    }
};

let vault_path_str = md_path.to_string_lossy().to_string();

// 2. DB 작업을 트랜잭션으로 래핑
let index_result = db.with_transaction(|| {
    let stats = engine.index_session_sync(&db, &session)?;
    db.update_session_vault_path(&session.id, &vault_path_str)?;
    Ok(stats)
});

match index_result {
    Ok(stats) => {
        print_ingest_result(&session, &md_path, &stats, format);
        ingested += 1;
    }
    Err(e) => {
        eprintln!("warn: indexing failed for {}, rolling back: {e}", session_path.display());
        // Cleanup: vault 파일 삭제
        if let Err(rm_err) = std::fs::remove_file(&md_path) {
            eprintln!("warn: failed to cleanup vault file {}: {rm_err}", md_path.display());
        }
        errors += 1;
        continue;
    }
}

// 3. post-ingest hook (트랜잭션 밖, 비치명적)
if let Err(e) = run_post_ingest_hook(&config, &session, &md_path) {
    eprintln!("warn: post-ingest hook failed: {e}");
}
```

### Step 3: index_session의 async 문제 해결

현재 `engine.index_session()`은 `async fn` (벡터 임베딩이 async). 트랜잭션 클로저 내에서 `.await`를 사용할 수 없음.

**해결 방안 A** (권장): BM25 인덱싱과 벡터 인덱싱을 분리
```rust
// BM25 인덱싱 (동기) — 트랜잭션 안
let bm25_stats = db.with_transaction(|| {
    let stats = engine.bm25_indexer.index_session(&db, &session)?;
    db.update_session_vault_path(&session.id, &vault_path_str)?;
    Ok(stats)
})?;

// 벡터 인덱싱 (비동기) — 트랜잭션 밖
if let Some(ref vector_indexer) = engine.vector_indexer {
    let embed_stats = vector_indexer.index_session(&db, &session).await;
    if let Err(e) = embed_stats {
        eprintln!("warn: vector embedding failed for {}: {e}", &session.id[..8]);
    }
}
```

> 벡터 임베딩 실패는 검색 품질 저하일 뿐 데이터 정합성 문제가 아님. `secall embed`로 나중에 재실행 가능.

**해결 방안 B**: 트랜잭션을 `block_on` 내부에서 사용 (복잡, 비권장)

### Step 4: SearchEngine에 BM25 전용 메서드 노출

```rust
// hybrid.rs 또는 SearchEngine impl
impl SearchEngine {
    /// BM25 인덱싱만 수행 (동기, 트랜잭션 호환)
    pub fn index_session_bm25(&self, db: &Database, session: &Session) -> Result<IndexStats> {
        self.bm25.index_session(db, session)
    }

    /// 벡터 인덱싱만 수행 (비동기, 트랜잭션 밖에서 호출)
    pub async fn index_session_vectors(&self, db: &Database, session: &Session) -> Result<IndexStats> {
        if let Some(ref v) = self.vector {
            v.index_session(db, session).await
        } else {
            Ok(IndexStats::default())
        }
    }
}
```

## Dependencies

- **Task 01** (ingest.rs 에러 전파): 에러 처리 패턴이 이미 정리된 상태에서 트랜잭션 래핑 적용
- **Task 02** (db.rs Result 반환): `update_session_vault_path()`가 이미 `Result` 반환이므로 직접 의존 없으나, db.rs의 다른 메서드 시그니처 변경이 완료된 상태가 깔끔

## Verification

```bash
# 1. 컴파일 확인
cargo check

# 2. 전체 테스트 회귀 없음
cargo test

# 3. 트랜잭션 롤백 확인 (수동)
# 의도적으로 DB를 읽기 전용으로 설정한 뒤 ingest 실행
# → vault 파일이 cleanup 되는지 확인
# chmod 444 ~/.local/share/secall/secall.db
# secall ingest --auto 2>&1 | grep "rolling back"
# chmod 644 ~/.local/share/secall/secall.db
```

> **[Developer 필수]** subtask-done 시그널 전에 위 명령의 실행 결과를 result 문서에 기록하세요. 형식: `✅ 명령 — exit 0` 또는 `❌ 명령 — 에러 내용 (사유)`. 검증 증빙 미제출 시 리뷰에서 conditional 처리됩니다.

## Risks

- **async/sync 경계**: `index_session()`이 async이므로 트랜잭션 클로저 내에서 직접 호출 불가. BM25/벡터 분리가 필수. SearchEngine의 공개 API가 변경됨.
- **vault cleanup 실패**: 트랜잭션 롤백 후 `remove_file()` 실패 시 orphan vault 파일 잔존. `secall lint` L002로 탐지 가능.
- **중첩 트랜잭션**: 현재 `with_transaction()`은 SAVEPOINT를 사용하지 않음. index_session 내부에서 별도 트랜잭션을 시작하면 안 됨.

## Scope boundary

다음 파일은 영향을 받을 수 있으나 이 task에서 수정하지 않음:
- `crates/secall-core/src/search/vector.rs` — 벡터 인덱싱 내부 로직. 트랜잭션 밖에서 실행되므로 변경 불필요.
- `crates/secall/src/commands/embed.rs` — 독립적인 `secall embed` 커맨드. 별도 트랜잭션 요구사항 없음.
