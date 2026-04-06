---
type: task
plan: secall-mvp
task_number: 8
title: 하이브리드 검색 (RRF)
status: draft
parallel_group: 2
depends_on: [6, 7]
updated_at: 2026-04-05
---

# Task 08: 하이브리드 검색 (RRF)

## Changed Files

- `crates/secall-core/src/search/hybrid.rs` — **신규**. RRF 결합 + 통합 검색 인터페이스
- `crates/secall-core/src/search/mod.rs` — `HybridSearcher` 공개, 통합 `SearchEngine` 구조체

## Change Description

### 1. RRF (Reciprocal Rank Fusion)

```rust
/// RRF 점수 계산: score = 1 / (k + rank)
/// k = 60 (qmd와 동일, 표준값)
const RRF_K: f64 = 60.0;

pub fn reciprocal_rank_fusion(
    bm25_results: &[SearchResult],
    vector_results: &[SearchResult],
    k: f64,
) -> Vec<SearchResult> {
    // 1. 각 결과에 RRF 점수 부여
    //    bm25: score = 1/(k + rank_in_bm25)
    //    vector: score = 1/(k + rank_in_vector)
    //
    // 2. (session_id, turn_index) 기준으로 합산
    //    같은 턴이 BM25와 벡터 모두에서 등장하면 점수 합산
    //
    // 3. 합산 점수 내림차순 정렬
    //
    // 4. 0.0~1.0 정규화
}
```

### 2. 통합 검색 엔진

```rust
pub struct SearchEngine {
    bm25: Bm25Indexer,
    vector: Option<VectorIndexer>,  // Ollama 없으면 None
}

impl SearchEngine {
    pub fn new(bm25: Bm25Indexer, vector: Option<VectorIndexer>) -> Self;

    /// 하이브리드 검색 (BM25 + 벡터 RRF)
    /// vector가 None이면 BM25-only
    pub async fn search(
        &self,
        db: &Database,
        query: &str,
        filters: &SearchFilters,
        limit: usize,
    ) -> Result<Vec<SearchResult>>;

    /// BM25-only 검색
    pub fn search_bm25(
        &self,
        db: &Database,
        query: &str,
        filters: &SearchFilters,
        limit: usize,
    ) -> Result<Vec<SearchResult>>;

    /// 벡터-only 검색
    pub async fn search_vector(
        &self,
        db: &Database,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>>;

    /// 세션 인덱싱 (BM25 + 벡터)
    pub async fn index_session(&self, db: &Database, session: &Session) -> Result<IndexStats>;
}
```

### 3. 검색 흐름

```
search(query, filters, limit)
  │
  ├─ BM25: tokenize(query) → FTS5 MATCH → 상위 limit*3개
  │
  ├─ Vector (있으면): embed(query) → vec0 MATCH → 상위 limit*3개
  │
  └─ RRF 결합 → 상위 limit개 반환
```

BM25와 벡터 각각에서 `limit * 3`개를 가져오는 이유: RRF 결합 시 충분한 후보 확보.

### 4. SearchResult 최종 형태

```rust
#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub session_id: String,
    pub turn_index: u32,
    pub score: f64,              // 0.0~1.0 (RRF 정규화)
    pub bm25_score: Option<f64>, // BM25 개별 점수
    pub vector_score: Option<f64>, // 벡터 개별 점수
    pub snippet: String,         // 매칭 부분 발췌 (~200자)
    pub metadata: SessionMeta,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionMeta {
    pub agent: String,
    pub model: Option<String>,
    pub project: Option<String>,
    pub date: String,
    pub vault_path: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct IndexStats {
    pub turns_indexed: usize,
    pub chunks_embedded: usize,
    pub errors: usize,
}
```

### 5. Temporal 필터 (기본)

```rust
impl SearchFilters {
    pub fn parse_temporal(input: &str) -> Option<SearchFilters> {
        match input.to_lowercase().as_str() {
            "today" => Some(/* since: today 00:00 */),
            "yesterday" => Some(/* since: yesterday 00:00, until: today 00:00 */),
            "last week" | "this week" => Some(/* since: 7 days ago */),
            s if s.starts_with("since ") => {
                // "since 2026-04-01" → parse date
            },
            _ => None,
        }
    }
}
```

## Dependencies

- Task 06 (Bm25Indexer)
- Task 07 (VectorIndexer)
- 추가 crate 없음

## Verification

```bash
# 빌드 성공
cd /Users/d9ng/privateProject/seCall && cargo build 2>&1 | tail -3

# 유닛 테스트: RRF
cargo test -p secall-core -- search::hybrid::tests --nocapture

# 테스트 항목:
# 1. RRF 기본 동작: BM25 [A, B, C] + Vector [B, C, D] → 합산 순서
# 2. B가 양쪽에 있으므로 점수 합산 → 1위
# 3. BM25-only (vector=empty) → BM25 결과 그대로
# 4. Vector-only (bm25=empty) → Vector 결과 그대로
# 5. 빈 결과 + 빈 결과 → 빈 Vec
# 6. 점수 정규화: 1위 = 1.0
# 7. temporal 필터 파싱: "today", "yesterday", "since 2026-04-01"

# 통합 테스트: SearchEngine
cargo test -p secall-core -- search::hybrid::integration --nocapture
# 1. BM25-only 모드에서 검색 동작
# 2. 필터 적용 동작 (project, agent, temporal)
# 3. index_session 후 search 결과 반환
```

## Risks

- **RRF k값 튜닝**: k=60은 표준이지만, 세션 검색에 최적인지 미검증. 설정으로 노출하되 기본값 60 유지. 실사용 후 조정.
- **BM25와 벡터의 후보 수 불균형**: BM25가 100개 반환하고 벡터가 5개 반환하면 BM25 쪽으로 편향. `limit * 3` 상한으로 완화하지만 완벽하지 않음. 후보 수가 10 미만이면 경고 로그.
- **snippet 추출 품질**: 단순 substring 추출은 문맥이 부족. 매칭 토큰 주변 ±100자 추출 방식 사용.

## Scope Boundary

- 리랭킹은 구현하지 않음 (post-MVP)
- 쿼리 확장 (hyde, query expansion)은 구현하지 않음 (post-MVP)
- temporal 자연어 확장 ("지난 금요일")은 Task 16
