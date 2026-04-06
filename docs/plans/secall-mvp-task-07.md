---
type: task
plan: secall-mvp
task_number: 7
title: 벡터 인덱서 + 검색
status: draft
parallel_group: 2
depends_on: [2, 3]
updated_at: 2026-04-05
---

# Task 07: 벡터 인덱서 + 검색

## Changed Files

- `Cargo.toml` (workspace) — reqwest 의존성 추가
- `crates/secall-core/Cargo.toml` — reqwest, sqlite-vec 의존성 추가
- `crates/secall-core/src/search/vector.rs` — **신규**. 벡터 인덱서 + 검색
- `crates/secall-core/src/search/embedding.rs` — **신규**. Ollama 임베딩 클라이언트
- `crates/secall-core/src/search/chunker.rs` — **신규**. 턴 기반 청킹
- `crates/secall-core/src/store/db.rs` — 벡터 테이블 생성 + INSERT/SELECT 메서드 추가

## Change Description

### 1. Ollama 임베딩 클라이언트 (embedding.rs)

```rust
pub struct OllamaEmbedder {
    client: reqwest::Client,
    base_url: String,         // 기본: http://localhost:11434
    model: String,            // 기본: bge-m3
}

impl OllamaEmbedder {
    pub fn new(base_url: Option<&str>, model: Option<&str>) -> Self;

    /// Ollama가 실행 중인지 확인
    pub async fn is_available(&self) -> bool;

    /// 단일 텍스트 임베딩
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// 배치 임베딩
    pub async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;

    /// 임베딩 차원 수 조회
    pub async fn dimensions(&self) -> Result<usize>;
}
```

**Ollama API 호출** (검증된 `/api/embed` 엔드포인트):
```
POST http://localhost:11434/api/embed
Request:  {"model": "bge-m3", "input": ["text1", "text2"], "truncate": true}
Response: {"model": "bge-m3", "embeddings": [[0.1, 0.2, ...], [0.3, 0.4, ...]], "total_duration": 14143917, "load_duration": 1019500, "prompt_eval_count": 8}
```
- `input`: 배열(배치) 또는 단일 문자열 모두 가능
- `embeddings`: **복수형** (구버전 `/api/embeddings`는 `embedding` 단수 — 사용하지 않음)
- `truncate`: true(기본) — context 초과 시 자동 잘라냄
- `keep_alive`: "5m"(기본) — 모델 VRAM 유지 시간

```rust
#[derive(Serialize)]
struct EmbedRequest {
    model: String,
    input: Vec<String>,   // 배치 지원
    #[serde(skip_serializing_if = "Option::is_none")]
    truncate: Option<bool>,
}

#[derive(Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,  // 복수형 주의
}
```

### 2. 청킹 (chunker.rs)

qmd의 breakpoint 패턴을 에이전트 세션에 맞게 변환:

```rust
pub struct Chunk {
    pub session_id: String,
    pub turn_index: u32,
    pub seq: u32,             // 턴 내 청크 순번
    pub text: String,
    pub context: String,      // 세션 메타 + 턴 역할/시간
}

pub fn chunk_session(session: &Session) -> Vec<Chunk> {
    // Breakpoint 점수:
    // - 턴 경계 → 100 (항상 새 청크)
    // - 도구 호출 경계 → 80
    // - 코드 블록 경계 → 60
    // - 빈 줄 (문단) → 20
    // - 줄바꿈 → 1
    //
    // 청크 크기: 최대 ~3600자 (900토큰 상당)
    // 오버랩: 15% (~540자)
    //
    // 각 청크에 context 부착:
    //   "Session: {agent} {project} {date} | Turn {N}: {role}"
}
```

턴이 짧으면 (< 500자) 전체가 하나의 청크. 턴이 길면 breakpoint 기반으로 분할.

### 3. sqlite-vec 통합 (검증된 API — sqlite-vec 0.1.9 + rusqlite 0.39)

**프로세스 전역 초기화** (main.rs 또는 Database::open에서 1회):
```rust
use sqlite_vec::sqlite3_vec_init;
use rusqlite::ffi::sqlite3_auto_extension;

// unsafe: 프로세스 전역에 sqlite-vec 확장 등록
// Database::open() 호출 전에 1회 실행
unsafe {
    sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ())));
}
```

**벡터 테이블 생성**:
```rust
impl Database {
    pub fn init_vector_table(&self, dimensions: usize) -> Result<()> {
        // vec0 가상 테이블 — 차원 수는 생성 시 고정
        self.conn.execute(
            &format!("CREATE VIRTUAL TABLE IF NOT EXISTS turn_vectors USING vec0(embedding float[{dimensions}])"),
            [],
        )?;
        // 메타데이터 테이블 (vec0은 rowid만 반환하므로)
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS turn_vector_meta (
                rowid INTEGER PRIMARY KEY,
                session_id TEXT NOT NULL,
                turn_index INTEGER NOT NULL,
                chunk_seq INTEGER NOT NULL,
                model TEXT NOT NULL,
                embedded_at TEXT NOT NULL
            )", [],
        )?;
        Ok(())
    }
}
```

**벡터 INSERT** (zerocopy로 복사 없는 바이너리 변환):
```rust
use zerocopy::AsBytes;

pub fn insert_vector(&self, embedding: &[f32], session_id: &str, turn_index: u32, chunk_seq: u32, model: &str) -> Result<i64> {
    // vec0에 삽입 — embedding은 &[u8]로 전달
    self.conn.execute(
        "INSERT INTO turn_vectors(embedding) VALUES (?)",
        [embedding.as_bytes()],
    )?;
    let rowid = self.conn.last_insert_rowid();

    // 메타데이터 삽입
    self.conn.execute(
        "INSERT INTO turn_vector_meta(rowid, session_id, turn_index, chunk_seq, model, embedded_at)
         VALUES (?, ?, ?, ?, ?, datetime('now'))",
        rusqlite::params![rowid, session_id, turn_index, chunk_seq, model],
    )?;
    Ok(rowid)
}
```

**KNN 검색**:
```rust
pub fn search_vectors(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<VectorRow>> {
    let mut stmt = self.conn.prepare(
        "SELECT v.rowid, v.distance, m.session_id, m.turn_index, m.chunk_seq
         FROM turn_vectors v
         JOIN turn_vector_meta m ON v.rowid = m.rowid
         WHERE v.embedding MATCH ?1
         ORDER BY v.distance
         LIMIT ?2"
    )?;
    let rows = stmt.query_map(
        rusqlite::params![query_embedding.as_bytes(), limit as i64],
        |row| Ok(VectorRow {
            rowid: row.get(0)?,
            distance: row.get(1)?,
            session_id: row.get(2)?,
            turn_index: row.get(3)?,
            chunk_seq: row.get(4)?,
        })
    )?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}
```

**Cargo.toml 의존성** (workspace):
```toml
sqlite-vec = "0.1.9"
rusqlite = { version = "0.39", features = ["bundled"] }  # 0.39 필수
zerocopy = "0.7"    # Vec<f32> → &[u8] 변환 (복사 없음)
```

**주의사항**:
- `sqlite3_auto_extension`은 프로세스 전역 — 테스트에서 병렬 실행 시 한 번만 호출
- `embedding MATCH ?`는 반드시 `ORDER BY distance LIMIT k`와 함께 사용
- 차원 수가 다른 모델로 재임베딩하면 테이블 재생성 필요

### 4. 벡터 인덱서

```rust
pub struct VectorIndexer {
    embedder: OllamaEmbedder,
}

impl VectorIndexer {
    pub fn new(embedder: OllamaEmbedder) -> Self;

    /// 세션의 청크를 임베딩하여 DB에 저장
    pub async fn index_session(&self, db: &Database, session: &Session) -> Result<IndexStats>;

    /// 벡터 검색 실행
    pub async fn search(
        &self,
        db: &Database,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>>;
}
```

### 5. Graceful Degradation

```rust
pub async fn create_vector_indexer() -> Option<VectorIndexer> {
    let embedder = OllamaEmbedder::new(None, None);
    if embedder.is_available().await {
        Some(VectorIndexer::new(embedder))
    } else {
        eprintln!("⚠ Ollama not available. Vector search disabled. BM25-only mode.");
        None
    }
}
```

ingest 시 VectorIndexer가 None이면 벡터 인덱싱 skip. 검색 시에도 BM25-only.

## Dependencies

- Task 02 (Database)
- Task 03 (Session 타입)
- `reqwest` crate (Ollama HTTP API)
- `sqlite-vec` crate (Rust 바인딩)

workspace `Cargo.toml` 추가:
```toml
[workspace.dependencies]
reqwest = { version = "0.12", features = ["json"] }
```

secall-core `Cargo.toml`:
```toml
[dependencies]
reqwest.workspace = true
sqlite-vec = "0.1.9"
zerocopy = "0.7"
```

## Verification

```bash
# 빌드 성공
cd /Users/d9ng/privateProject/seCall && cargo build 2>&1 | tail -5

# 유닛 테스트: 청킹
cargo test -p secall-core -- search::chunker::tests --nocapture

# 테스트 항목 (청킹):
# 1. 짧은 턴 → 단일 청크
# 2. 긴 턴 → breakpoint 기반 분할
# 3. 도구 호출 경계에서 분할
# 4. context 문자열 정확성
# 5. 오버랩 구간 존재 확인

# 유닛 테스트: 벡터 DB (sqlite-vec 없이 테스트 가능한 부분)
cargo test -p secall-core -- search::vector::tests --nocapture

# 통합 테스트 (Ollama 실행 필요):
# SECALL_TEST_OLLAMA=1 cargo test -p secall-core -- search::vector::integration --nocapture
# 테스트 항목:
# 1. Ollama 가용성 확인
# 2. 단일 텍스트 임베딩 반환
# 3. 배치 임베딩 반환
# 4. 임베딩 → DB INSERT → 검색 → 결과 반환
# 5. Ollama 미실행 시 graceful skip
```

## Risks

- **sqlite-vec Rust 바인딩 미성숙**: `sqlite-vec` crate의 Rust 버전이 불안정할 수 있음. 대안: `rusqlite`의 `load_extension`으로 sqlite-vec 공유 라이브러리를 직접 로드. `loadable_extension` feature 활성화 필요.
- **rusqlite bundled + sqlite-vec 호환성**: `rusqlite`의 `bundled` feature가 빌드하는 SQLite와 `sqlite-vec`이 기대하는 SQLite 버전이 불일치할 수 있음. 빌드 시 검증.
- **Ollama 모델 미설치**: bge-m3이 설치되어 있지 않으면 임베딩 실패. 에러 메시지에 `ollama pull bge-m3` 안내 포함.
- **임베딩 차원 불일치**: bge-m3 = 1024차원, nomic-embed-text = 768차원. 모델 변경 시 기존 벡터와 차원 불일치. DB에 `model` 필드를 저장하고, 모델 변경 시 재임베딩 필요하다는 경고 출력.
- **배치 임베딩 메모리**: 대량 세션 임베딩 시 Ollama 메모리 사용량 급증. 배치 크기 제한 (기본 32) + 배치 간 sleep.

## Scope Boundary

- 임베딩 모델은 Ollama API만 지원. ONNX/candle은 Task 14.
- 리랭킹은 이 태스크에서 구현하지 않음 (post-MVP)
- RRF 결합은 Task 08
