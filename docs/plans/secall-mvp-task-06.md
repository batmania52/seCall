---
type: task
plan: secall-mvp
task_number: 6
title: 한국어 BM25 인덱서
status: draft
parallel_group: 2
depends_on: [2, 3]
updated_at: 2026-04-05
---

# Task 06: 한국어 BM25 인덱서

## Changed Files

- `Cargo.toml` (workspace) — lindera 의존성 추가
- `crates/secall-core/Cargo.toml` — lindera 의존성 추가
- `crates/secall-core/src/lib.rs` — `pub mod search;` 추가
- `crates/secall-core/src/search/mod.rs` — **신규**. Search 모듈
- `crates/secall-core/src/search/tokenizer.rs` — **신규**. Tokenizer trait + LinderaKoTokenizer
- `crates/secall-core/src/search/bm25.rs` — **신규**. BM25 인덱싱 + 검색
- `crates/secall-core/src/store/db.rs` — 턴 INSERT + FTS5 INSERT 메서드 추가

## Change Description

### 1. Tokenizer trait

```rust
pub trait Tokenizer: Send + Sync {
    /// 텍스트를 토큰 목록으로 변환
    fn tokenize(&self, text: &str) -> Vec<String>;

    /// 토큰화된 결과를 공백 구분 문자열로 반환 (FTS5 삽입용)
    fn tokenize_for_fts(&self, text: &str) -> String {
        self.tokenize(text).join(" ")
    }
}
```

### 2. LinderaKoTokenizer (검증된 API — lindera v2.3.4)

```rust
use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::tokenizer::Tokenizer as LinderaInner;
use lindera::token_filter::korean_keep_tags::KoreanKeepTagsTokenFilter;
use lindera::token_filter::BoxTokenFilter;

pub struct LinderaKoTokenizer {
    inner: LinderaInner,
}

impl LinderaKoTokenizer {
    pub fn new() -> Result<Self> {
        // URI 기반 딕셔너리 로딩 — "embedded://ko-dic"
        let dictionary = load_dictionary("embedded://ko-dic")
            .map_err(|e| anyhow::anyhow!("lindera dictionary load failed: {e}"))?;
        let segmenter = Segmenter::new(Mode::Normal, dictionary, None);
        let mut tokenizer = LinderaInner::new(segmenter);

        // 품사 필터: NNG, NNP, NNB, VV, VA, SL만 유지
        let keep_tags = KoreanKeepTagsTokenFilter::new(
            ["NNG", "NNP", "NNB", "VV", "VA", "SL"]
                .iter().map(|s| s.to_string()).collect()
        );
        tokenizer.append_token_filter(BoxTokenFilter::from(keep_tags));

        Ok(Self { inner: tokenizer })
    }
}

impl Tokenizer for LinderaKoTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        let mut tokens = match self.inner.tokenize(text) {
            Ok(t) => t,
            Err(_) => return vec![],
        };
        tokens.iter_mut()
            .map(|t| t.surface.as_ref().to_lowercase())
            .filter(|s| s.len() > 1)  // 1자 토큰 제거
            .collect()
    }
}
```

**Cargo.toml 의존성** (workspace):
```toml
lindera = { version = "2.3.4", features = ["embed-ko-dic"] }
# embed-ko-dic: ko-dic 사전을 바이너리에 번들 (추가 crate 불필요)
# lindera-ko-dic 별도 추가 불필요 — feature flag로 해결
```

**품사 필터링 전략** (ko-dic `token.details()[0]` = `part_of_speech_tag`):
- 포함: NNG(일반명사), NNP(고유명사), NNB(의존명사), VV(동사), VA(형용사), SL(외국어)
- 제외: JKS(주격조사), JKO(목적격조사), EP(선어말어미), EF(종결어미), SF(마침표) 등
- `KoreanKeepTagsTokenFilter`가 필터링 수행 — 수동 필터 불필요
- 영어/숫자: SL(외국어) 태그로 통과, `to_lowercase()` 정규화

**주의사항**:
- `token.details()`는 `&mut self` 필요 — `iter_mut()` 사용 필수
- `token.get("part_of_speech_tag")`로 필드명 접근 가능 (인덱스 0과 동일)
- `append_token_filter()`는 체이닝 가능 — 여러 필터 순차 적용

### 3. FTS5 인덱싱 전략

SQLite FTS5에 커스텀 토크나이저를 Rust에서 등록하는 것은 매우 복잡 (rusqlite vtab API 필요). 대신 **사전 토큰화** 방식:

```
원본 텍스트 → lindera 토큰화 → 공백 구분 토큰 문자열 → FTS5 INSERT
검색 쿼리 → lindera 토큰화 → 공백 구분 토큰 문자열 → FTS5 MATCH
```

FTS5 테이블은 `tokenize='unicode61'`로 생성 (공백 분리만 하면 됨).
삽입되는 내용이 이미 형태소 분석된 토큰 문자열이므로 FTS5는 공백 분리만 담당.

```sql
-- 인덱싱 시
INSERT INTO turns_fts(content, session_id, turn_id)
VALUES ('아키텍처 설계 컴포넌트 분리 rust workspace', 'session_id', 42);

-- 검색 시
SELECT * FROM turns_fts WHERE content MATCH '아키텍처 설계';
```

### 4. BM25 인덱서

```rust
pub struct Bm25Indexer {
    tokenizer: Box<dyn Tokenizer>,
}

impl Bm25Indexer {
    pub fn new(tokenizer: Box<dyn Tokenizer>) -> Self;

    /// 세션의 모든 턴을 FTS5에 인덱싱
    pub fn index_session(&self, db: &Database, session: &Session) -> Result<IndexStats>;

    /// BM25 검색 실행
    pub fn search(
        &self,
        db: &Database,
        query: &str,
        limit: usize,
        filters: &SearchFilters,
    ) -> Result<Vec<SearchResult>>;
}

pub struct SearchFilters {
    pub project: Option<String>,
    pub agent: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
}

pub struct SearchResult {
    pub session_id: String,
    pub turn_index: u32,
    pub score: f64,          // BM25 점수 (정규화 전)
    pub snippet: String,     // 매칭 부분 발췌
    pub metadata: SessionMeta,
}
```

### 5. 점수 정규화

BM25 raw 점수를 0.0~1.0으로 정규화:
```rust
fn normalize_scores(results: &mut [SearchResult]) {
    if results.is_empty() { return; }
    let max = results.iter().map(|r| r.score).fold(f64::NEG_INFINITY, f64::max);
    if max > 0.0 {
        for r in results.iter_mut() {
            r.score /= max;
        }
    }
}
```

### 6. DB 메서드 추가 (store/db.rs)

```rust
impl Database {
    /// 세션 메타데이터 INSERT
    pub fn insert_session(&self, session: &Session) -> Result<()>;

    /// 턴 데이터 INSERT
    pub fn insert_turn(&self, session_id: &str, turn: &Turn) -> Result<i64>;

    /// FTS5 INSERT (토큰화된 텍스트)
    pub fn insert_fts(&self, tokenized_content: &str, session_id: &str, turn_id: i64) -> Result<()>;

    /// BM25 검색
    pub fn search_fts(&self, tokenized_query: &str, limit: usize) -> Result<Vec<FtsRow>>;

    /// 세션 존재 여부 확인
    pub fn session_exists(&self, session_id: &str) -> Result<bool>;
}
```

## Dependencies

- Task 02 (Database, 스키마)
- Task 03 (Session, Turn 타입)
- `lindera` + `lindera-ko-dic` crates

workspace `Cargo.toml` 추가:
```toml
[workspace.dependencies]
lindera = { version = "2.3.4", features = ["embed-ko-dic"] }
# lindera-dictionary 별도 추가 불필요 — lindera 단일 crate에 통합됨
```

secall-core `Cargo.toml`:
```toml
[dependencies]
lindera.workspace = true
```

feature flag는 workspace 레벨의 `embed-ko-dic`으로 제어됨. 별도 feature 선언 불필요.

## Verification

```bash
# 빌드 성공
cd /Users/d9ng/privateProject/seCall && cargo build 2>&1 | tail -5

# 유닛 테스트: 토크나이저
cargo test -p secall-core -- search::tokenizer::tests --nocapture

# 테스트 항목:
# 1. 한국어 텍스트 토큰화: "아키텍처를 설계한다" → ["아키텍처", "설계"]
# 2. 영어 텍스트 토큰화: "Rust workspace" → ["rust", "workspace"]
# 3. 혼합 텍스트: "seCall의 BM25 검색" → ["secall", "bm25", "검색"]
# 4. 빈 텍스트 → 빈 Vec
# 5. 특수문자만 → 빈 Vec

# 유닛 테스트: BM25 인덱싱 + 검색
cargo test -p secall-core -- search::bm25::tests --nocapture

# 테스트 항목:
# 1. 세션 인덱싱 후 검색 결과 반환
# 2. 한국어 쿼리 검색 동작
# 3. 영어 쿼리 검색 동작
# 4. 혼합 쿼리 검색 동작
# 5. 결과 없는 쿼리 → 빈 Vec
# 6. 점수 정규화 (max = 1.0)
# 7. 필터 (project, agent) 동작
```

## Risks

- **lindera API 불안정**: lindera는 major/minor 버전 간 API 변경이 잦음. `Cargo.toml`에서 정확한 버전 고정. 빌드 실패 시 API 변경 확인.
- **ko-dic embed 시 바이너리 크기 증가**: ~15-20MB 증가. 허용 가능하지만, CI 빌드 시간도 증가. `embed-ko-dic`을 feature flag로 분리하여 개발 시에는 런타임 로드 옵션 제공.
- **사전 토큰화 방식의 한계**: FTS5의 proximity search, phrase search가 정확하지 않을 수 있음. 형태소 분석 결과의 토큰 순서가 원본과 다를 수 있기 때문. BM25 점수 계산에는 영향 없음.
- **lindera ko-dic 2018 사전**: 최신 용어 (예: "ChatGPT", "LLM") 미등록. 영어 토큰으로 통과하므로 치명적이지 않지만, 한국어 신조어는 어절 단위로만 인덱싱됨.

## Scope Boundary

- FTS5 커스텀 토크나이저 등록은 하지 않음 (사전 토큰화 방식 사용)
- kiwi-rs 토크나이저는 이 태스크에서 구현하지 않음 (Task 15)
- 벡터 인덱싱은 Task 07
