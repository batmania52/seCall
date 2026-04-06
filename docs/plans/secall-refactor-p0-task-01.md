---
type: task
status: draft
plan: secall-refactor-p0
task_number: 1
title: "BM25 turn_index 수정"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 01: BM25 turn_index 수정

## 문제

`insert_turn()` → `last_insert_rowid()` → `insert_fts(turn_id=rowid)` 순서로 FTS에 저장.
검색 시 `search_fts()`가 이 rowid를 반환하고, `bm25.rs:138`에서 `row.turn_id as u32`로 캐스팅하여 `SearchResult.turn_index`에 설정.

**결과**: DB 전역 rowid(1, 2, 3, ...)가 세션 내 턴 순서(0, 1, 2, ...)로 표시됨.
세션이 여러 개 인덱싱되면 rowid와 turn_index가 일치하지 않음.

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/search/bm25.rs:64-98` | 수정 | `index_session()` — insert_fts에 turn.index 전달 |
| `crates/secall-core/src/search/bm25.rs:100-151` | 수정 | `search()` — FtsRow.turn_id → turn_index 필드명 변경 |
| `crates/secall-core/src/search/bm25.rs:46-52` | 수정 | `FtsRow` 구조체 필드명 변경 |
| `crates/secall-core/src/search/bm25.rs:279-285` | 수정 | `insert_fts()` — 파라미터를 turn_index로 변경 |
| `crates/secall-core/src/search/bm25.rs:287-317` | 수정 | `search_fts()` — SELECT 컬럼명 확인 |
| `crates/secall-core/src/search/bm25.rs:348-452` | 수정 | 테스트 — turn_index 검증 테스트 추가 |

## Change description

### Step 1: FtsRow 구조체 필드명 정리

```rust
// bm25.rs:46-52 — 변경 전
pub struct FtsRow {
    pub session_id: String,
    pub turn_id: i64,       // ← rowid
    pub content: String,
    pub score: f64,
}

// 변경 후
pub struct FtsRow {
    pub session_id: String,
    pub turn_index: u32,    // ← 실제 turn_index
    pub content: String,
    pub score: f64,
}
```

### Step 2: insert_fts() 시그니처 + FTS INSERT 수정

```rust
// bm25.rs:279-285 — 변경 전
pub fn insert_fts(&self, tokenized_content: &str, session_id: &str, turn_id: i64) -> Result<()> {
    self.conn().execute(
        "INSERT INTO turns_fts(content, session_id, turn_id) VALUES (?1, ?2, ?3)",
        rusqlite::params![tokenized_content, session_id, turn_id],
    )?;
    Ok(())
}

// 변경 후
pub fn insert_fts(&self, tokenized_content: &str, session_id: &str, turn_index: u32) -> Result<()> {
    self.conn().execute(
        "INSERT INTO turns_fts(content, session_id, turn_id) VALUES (?1, ?2, ?3)",
        rusqlite::params![tokenized_content, session_id, turn_index as i64],
    )?;
    Ok(())
}
```

> FTS5 테이블의 컬럼명 `turn_id`는 유지 (스키마 변경 최소화). 저장하는 **값**만 turn_index로 변경.

### Step 3: index_session()에서 turn.index 전달

```rust
// bm25.rs:81-84 — 변경 전
match db.insert_turn(&session.id, turn) {
    Ok(turn_id) => {
        if let Err(e) = db.insert_fts(&full_text, &session.id, turn_id) {

// 변경 후
match db.insert_turn(&session.id, turn) {
    Ok(_turn_rowid) => {
        if let Err(e) = db.insert_fts(&full_text, &session.id, turn.index) {
```

### Step 4: search_fts() 결과 매핑 수정

```rust
// bm25.rs:305-312 — 변경 전
|row| {
    Ok(FtsRow {
        session_id: row.get(0)?,
        turn_id: row.get(1)?,
        ...
    })
}

// 변경 후
|row| {
    Ok(FtsRow {
        session_id: row.get(0)?,
        turn_index: row.get::<_, i64>(1)? as u32,
        ...
    })
}
```

### Step 5: search() 결과 매핑 수정

```rust
// bm25.rs:136-144 — 변경 전
Some(SearchResult {
    session_id: row.session_id,
    turn_index: row.turn_id as u32,  // ← rowid 캐스팅
    ...
})

// 변경 후
Some(SearchResult {
    session_id: row.session_id,
    turn_index: row.turn_index,      // ← 이미 올바른 값
    ...
})
```

### Step 6: 테스트 추가 — rowid ≠ turn_index 시나리오

```rust
#[test]
fn test_turn_index_not_rowid() {
    // 두 세션을 인덱싱하여 rowid가 turn_index와 다른 상황 재현
    let db = Database::open_memory().unwrap();
    let tok = LinderaKoTokenizer::new().unwrap();
    let indexer = Bm25Indexer::new(Box::new(tok));

    // Session 1: 3 turns (turn_index 0, 1, 2)
    let session1 = Session {
        id: "s-first".to_string(),
        turns: vec![
            Turn { index: 0, content: "첫번째 세션 첫턴".to_string(), .. },
            Turn { index: 1, content: "첫번째 세션 두번째턴".to_string(), .. },
            Turn { index: 2, content: "아키텍처 설계".to_string(), .. },
        ],
        ..
    };
    indexer.index_session(&db, &session1).unwrap();

    // Session 2: 2 turns (turn_index 0, 1)
    // DB rowid는 이 시점에서 4, 5
    let session2 = Session {
        id: "s-second".to_string(),
        turns: vec![
            Turn { index: 0, content: "두번째 세션 아키텍처".to_string(), .. },
            Turn { index: 1, content: "두번째 세션 마지막".to_string(), .. },
        ],
        ..
    };
    indexer.index_session(&db, &session2).unwrap();

    // "아키텍처"로 검색 — session2의 결과가 turn_index=0이어야 함 (rowid=4가 아님)
    let results = indexer.search(&db, "아키텍처", 10, &SearchFilters::default()).unwrap();
    for r in &results {
        if r.session_id == "s-second" {
            assert_eq!(r.turn_index, 0, "session2의 turn_index는 0이어야 하나 rowid가 반환됨");
        }
        if r.session_id == "s-first" {
            assert_eq!(r.turn_index, 2, "session1의 turn_index는 2이어야 함");
        }
    }
}
```

## Dependencies

- 없음 (독립 실행 가능)

## Verification

```bash
# 1. 컴파일 확인
cargo check -p secall-core

# 2. 기존 BM25 테스트 통과
cargo test -p secall-core bm25

# 3. 새 테스트(turn_index_not_rowid) 통과
cargo test -p secall-core test_turn_index_not_rowid

# 4. 전체 테스트 회귀 없음
cargo test
```

## Risks

- **기존 FTS 데이터 불일치**: 이미 인덱싱된 데이터에는 rowid가 turn_id로 저장되어 있음. `secall reindex` 명령으로 FTS를 재생성해야 정확한 turn_index가 저장됨. 사용자에게 재인덱싱 안내 필요.
- **FTS5 컬럼명 turn_id 유지**: 스키마 변경을 피하기 위해 FTS 테이블의 컬럼명은 `turn_id`를 유지하되, 저장하는 값만 변경. 혼동 방지를 위해 코드 주석으로 "이 컬럼에는 실제 turn_index 값이 저장됨"을 명시.

## Scope boundary

다음 파일은 영향을 받을 수 있으나 이 task에서 수정하지 않음:
- `crates/secall-core/src/search/hybrid.rs` — SearchResult를 사용하나 turn_index 필드를 직접 조작하지 않음
- `crates/secall-core/src/store/schema.rs` — FTS5 테이블 DDL 변경 없음
- `crates/secall-core/src/mcp/tools.rs` — SearchResult를 직렬화만 함
