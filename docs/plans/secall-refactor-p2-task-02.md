---
type: task
status: draft
plan: secall-refactor-p2
task_number: 2
title: "벡터 검색 메모리 최적화"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 02: 벡터 검색 메모리 최적화

## 문제

`vector.rs:286-326`의 `search_vectors()`가 `turn_vectors` 테이블 전체를 메모리에 로드:

```sql
SELECT id, session_id, turn_index, chunk_seq, embedding FROM turn_vectors
-- WHERE 없음, LIMIT 없음
```

100k 행 × 384차원 × 4바이트 = ~153MB embedding BLOB + 오버헤드 ≈ **~400MB**.
모든 행에 cosine_distance 계산 후 truncate.

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/search/vector.rs:286-326` | 수정 | `search_vectors()` — session_id 필터 + 선택적 전체 검색 |
| `crates/secall-core/src/search/hybrid.rs` | 수정 | BM25 결과에서 후보 session_id 추출 → 벡터 검색에 전달 |

## Change description

### 전략: 2단계 검색

```
1단계: BM25 검색 → 상위 N개 세션의 session_id 수집
2단계: 벡터 검색 WHERE session_id IN (...) → 후보 세션만 로드
```

이 전략은 하이브리드 검색(RRF)에서 이미 BM25 결과가 먼저 실행되므로 자연스러운 파이프라인.
벡터 전용 검색(`--vec` 플래그)에서는 전체 검색 유지하되, LIMIT으로 메모리 상한 설정.

### Step 1: search_vectors()에 session_id 필터 추가

```rust
// vector.rs:286 — 변경 전
pub fn search_vectors(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<VectorRow>> {
    let mut stmt = self.conn().prepare(
        "SELECT id, session_id, turn_index, chunk_seq, embedding FROM turn_vectors",
    )?;
    let rows: Vec<...> = stmt.query_map([], |row| { ... })?.filter_map(|r| r.ok()).collect();
    // ... cosine distance for ALL rows ...
}

// 변경 후
pub fn search_vectors(
    &self,
    query_embedding: &[f32],
    limit: usize,
    session_ids: Option<&[String]>,
) -> Result<Vec<VectorRow>> {
    let (sql, params) = if let Some(ids) = session_ids {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        // 동적 IN 절 생성
        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{}", i)).collect();
        let sql = format!(
            "SELECT id, session_id, turn_index, chunk_seq, embedding FROM turn_vectors WHERE session_id IN ({})",
            placeholders.join(",")
        );
        let params: Vec<Box<dyn rusqlite::types::ToSql>> = ids.iter()
            .map(|id| Box::new(id.clone()) as Box<dyn rusqlite::types::ToSql>)
            .collect();
        (sql, params)
    } else {
        // 전체 검색 (벡터 전용 모드)
        let sql = "SELECT id, session_id, turn_index, chunk_seq, embedding FROM turn_vectors".to_string();
        (sql, Vec::new())
    };

    let mut stmt = self.conn().prepare(&sql)?;
    let rows: Vec<(i64, String, u32, u32, Vec<u8>)> = stmt
        .query_map(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get::<_, i64>(2)? as u32,
                    row.get::<_, i64>(3)? as u32,
                    row.get(4)?,
                ))
            },
        )?
        .filter_map(|r| r.ok())
        .collect();

    // cosine distance (동일)
    let mut scored: Vec<(f32, VectorRow)> = rows
        .into_iter()
        .map(|(id, session_id, turn_index, chunk_seq, bytes)| {
            let embedding = bytes_to_floats(&bytes);
            let distance = cosine_distance(query_embedding, &embedding);
            (distance, VectorRow { rowid: id, distance, session_id, turn_index, chunk_seq })
        })
        .collect();

    scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);
    Ok(scored.into_iter().map(|(_, row)| row).collect())
}
```

### Step 2: VectorIndexer의 search 메서드 업데이트

```rust
// vector.rs — VectorIndexer::search() 및 search_with_embedding()
pub async fn search(
    &self,
    db: &Database,
    query: &str,
    limit: usize,
    filters: &SearchFilters,
    candidate_session_ids: Option<&[String]>,  // 새 파라미터
) -> Result<Vec<SearchResult>> {
    let query_embedding = self.embedder.embed(query).await?;
    let rows = db.search_vectors(&query_embedding, limit, candidate_session_ids)?;
    // ... (기존 매핑 동일)
}
```

> `search_with_embedding()`에도 동일하게 `candidate_session_ids` 파라미터 추가.

### Step 3: hybrid.rs에서 BM25 결과 기반 후보 추출

```rust
// hybrid.rs — rrf_search() 또는 search() 메서드
// BM25 검색 실행 후
let bm25_results = self.bm25.search(db, query, limit * 3, filters)?;

// BM25 결과에서 session_id 추출
let candidate_ids: Vec<String> = bm25_results
    .iter()
    .map(|r| r.session_id.clone())
    .collect::<std::collections::HashSet<_>>()
    .into_iter()
    .collect();

// 벡터 검색에 후보 전달
if let Some(ref vector) = self.vector {
    let vec_results = vector.search_with_embedding(
        db, &query_embedding, limit * 3, filters,
        Some(&candidate_ids),
    )?;
    // RRF 합산...
}
```

### Step 4: 벡터 전용 모드 (`--vec`) 처리

`--vec` 플래그 시 BM25 결과가 없으므로 `candidate_session_ids = None` → 전체 검색.
이 경우 메모리 사용량 상한을 위해 SQL에 LIMIT 추가 고려:

```sql
-- 전체 검색 시에도 상한 설정 (예: 10,000행)
SELECT ... FROM turn_vectors ORDER BY RANDOM() LIMIT 10000
```

> 실용적으로 `--vec` 전용 모드에서 100k 전체 로드는 일반적이지 않음. 우선 경고만 출력.

### Step 5: 기존 호출자 업데이트

`search_vectors()`의 시그니처가 변경되므로 모든 호출자 수정:

```rust
// vector.rs:86 — VectorIndexer::search()
let rows = db.search_vectors(&query_embedding, limit, None)?;  // 기존 동작 유지

// vector.rs:123 — search_with_embedding()
let rows = db.search_vectors(embedding, limit, None)?;  // 기존 동작 유지
```

### Step 6: 테스트 수정

```rust
// vector.rs tests — 기존 테스트에 None 파라미터 추가
#[test]
fn test_insert_and_search_vectors() {
    // ...
    let rows = db.search_vectors(&query, 2, None).unwrap();  // None = 전체 검색
    assert_eq!(rows.len(), 2);
}

// 새 테스트: session_id 필터
#[test]
fn test_search_vectors_with_session_filter() {
    let db = Database::open_memory().unwrap();
    db.init_vector_table().unwrap();

    db.insert_vector(&[1.0, 0.0, 0.0], "s1", 0, 0, "test").unwrap();
    db.insert_vector(&[0.0, 1.0, 0.0], "s2", 0, 0, "test").unwrap();

    let query = vec![1.0, 0.1, 0.0];
    // s1만 필터
    let rows = db.search_vectors(&query, 10, Some(&["s1".to_string()])).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].session_id, "s1");
}
```

## Dependencies

- 없음 (독립 실행 가능)

## Verification

```bash
# 1. 컴파일 확인
cargo check -p secall-core

# 2. 벡터 테스트 통과
cargo test -p secall-core vector

# 3. 하이브리드 검색 테스트 통과
cargo test -p secall-core hybrid

# 4. 전체 테스트 회귀 없음
cargo test
```

> **[Developer 필수]** subtask-done 시그널 전에 위 명령의 실행 결과를 result 문서에 기록하세요. 형식: `✅ 명령 — exit 0` 또는 `❌ 명령 — 에러 내용 (사유)`. 검증 증빙 미제출 시 리뷰에서 conditional 처리됩니다.

## Risks

- **BM25 결과 없는 벡터 검색**: 하이브리드 모드에서 BM25가 0건이면 벡터 검색도 0건 반환될 수 있음. fallback으로 `None` (전체 검색) 사용 고려.
- **동적 IN 절 SQL 인젝션**: `rusqlite::params_from_iter`를 사용하면 파라미터 바인딩이므로 안전.
- **대규모 IN 절**: SQLite의 `SQLITE_MAX_VARIABLE_NUMBER` 기본값은 999. 후보 세션이 이를 초과하면 분할 쿼리 필요. 실용적으로 BM25 상위 결과가 999개를 넘지 않음.

## Scope boundary

다음 파일은 이 task에서 수정하지 않음:
- `crates/secall-core/src/search/chunker.rs` — 청킹 로직 변경 없음
- `crates/secall-core/src/search/embedding.rs` — Embedder 인터페이스 변경 없음
- `crates/secall/src/commands/recall.rs` — SearchEngine.search() 호출만 하므로 내부 변경에 영향 없음
