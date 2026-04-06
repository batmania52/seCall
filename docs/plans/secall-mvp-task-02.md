---
type: task
plan: secall-mvp
task_number: 2
title: SQLite 스키마 설계 + 초기화
status: draft
parallel_group: 0
depends_on: [1]
updated_at: 2026-04-05
---

# Task 02: SQLite 스키마 설계 + 초기화

## Changed Files

- `crates/secall-core/Cargo.toml` — rusqlite 의존성 추가
- `crates/secall-core/src/lib.rs` — `pub mod store;` 추가
- `crates/secall-core/src/store/mod.rs` — **신규**. Store 모듈 entrypoint
- `crates/secall-core/src/store/db.rs` — **신규**. DB 초기화 + 마이그레이션
- `crates/secall-core/src/store/schema.rs` — **신규**. SQL DDL 상수
- `Cargo.toml` (workspace) — rusqlite를 workspace dependency에 추가

## Change Description

### 1. 의존성 추가

workspace `Cargo.toml`:
```toml
[workspace.dependencies]
rusqlite = { version = "0.39", features = ["bundled"] }
# 0.39 필수: sqlite-vec 0.1.9와 호환. "bundled"는 SQLite 소스 빌드 (FTS5 보장).
# "vtab" feature는 0.39에서 기본 포함.
```

secall-core `Cargo.toml`:
```toml
[dependencies]
rusqlite.workspace = true
```

`bundled` feature는 SQLite를 소스에서 빌드 (시스템 SQLite에 의존하지 않음, FTS5 보장).

### 2. 스키마 설계

```sql
-- 세션 메타데이터
CREATE TABLE IF NOT EXISTS sessions (
    id          TEXT PRIMARY KEY,   -- session UUID
    agent       TEXT NOT NULL,      -- 'claude-code' | 'codex' | 'gemini-cli'
    model       TEXT,
    project     TEXT,
    cwd         TEXT,
    git_branch  TEXT,
    start_time  TEXT NOT NULL,      -- ISO8601
    end_time    TEXT,
    turn_count  INTEGER DEFAULT 0,
    tokens_in   INTEGER DEFAULT 0,
    tokens_out  INTEGER DEFAULT 0,
    tools_used  TEXT,               -- JSON array
    tags        TEXT,               -- JSON array
    vault_path  TEXT,               -- relative path in vault
    ingested_at TEXT NOT NULL,      -- ISO8601
    status      TEXT DEFAULT 'raw'  -- 'raw' | 'indexed' | 'wiki-integrated'
);

-- 턴 데이터
CREATE TABLE IF NOT EXISTS turns (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id  TEXT NOT NULL REFERENCES sessions(id),
    turn_index  INTEGER NOT NULL,
    role        TEXT NOT NULL,      -- 'user' | 'assistant' | 'system'
    timestamp   TEXT,
    content     TEXT NOT NULL,      -- 렌더링된 텍스트
    has_tool    INTEGER DEFAULT 0,
    tool_names  TEXT,               -- JSON array
    thinking    TEXT,               -- thinking block (nullable)
    tokens_in   INTEGER DEFAULT 0,
    tokens_out  INTEGER DEFAULT 0,
    UNIQUE(session_id, turn_index)
);

-- BM25 전문검색 (FTS5)
CREATE VIRTUAL TABLE IF NOT EXISTS turns_fts USING fts5(
    content,
    session_id UNINDEXED,
    turn_id UNINDEXED,
    tokenize='unicode61'
);
-- 주의: 초기에는 unicode61. Task 06에서 lindera 커스텀 토크나이저로 교체.

-- 벡터 임베딩 (Task 07에서 sqlite-vec으로 생성)
-- CREATE VIRTUAL TABLE IF NOT EXISTS turn_vectors USING vec0(...);
-- 이 테이블은 Task 07에서 sqlite-vec 확장 로드 후 생성.

-- ingest 로그
CREATE TABLE IF NOT EXISTS ingest_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id  TEXT NOT NULL,
    action      TEXT NOT NULL,      -- 'ingest' | 'reindex' | 'delete'
    timestamp   TEXT NOT NULL,
    details     TEXT                -- JSON (files created, turns indexed, etc.)
);

-- 설정
CREATE TABLE IF NOT EXISTS config (
    key   TEXT PRIMARY KEY,
    value TEXT
);

-- 스키마 버전 (마이그레이션용)
-- config에 'schema_version' = '1' 로 저장

-- 인덱스
CREATE INDEX IF NOT EXISTS idx_turns_session ON turns(session_id);
CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project);
CREATE INDEX IF NOT EXISTS idx_sessions_agent ON sessions(agent);
CREATE INDEX IF NOT EXISTS idx_sessions_date ON sessions(start_time);
```

### 3. DB 초기화 모듈 (store/db.rs)

```rust
pub struct Database {
    conn: rusqlite::Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self>;       // 파일 DB
    pub fn open_memory() -> Result<Self>;            // 테스트용
    pub fn migrate(&self) -> Result<()>;             // 스키마 버전 체크 + 마이그레이션
    pub fn conn(&self) -> &rusqlite::Connection;
}
```

마이그레이션 전략: `config` 테이블의 `schema_version` 값을 읽고, 현재 코드의 `CURRENT_SCHEMA_VERSION`과 비교. 차이가 있으면 순차 마이그레이션 실행. v1은 초기 스키마 전체 생성.

### 4. DB 파일 경로

기본값: `~/.cache/secall/index.sqlite`
환경변수 오버라이드: `SECALL_DB_PATH`
테스트: `:memory:`

`store/mod.rs`에서 `get_default_db_path()` 함수 제공:
```rust
pub fn get_default_db_path() -> PathBuf {
    if let Ok(p) = std::env::var("SECALL_DB_PATH") {
        return PathBuf::from(p);
    }
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("secall")
        .join("index.sqlite")
}
```

## Dependencies

- Task 01 (workspace 존재해야 함)
- `rusqlite` crate with `bundled` feature
- `dirs` crate (경로 해석용) — workspace dependency 추가 필요

## Verification

```bash
# 빌드 성공
cd /Users/d9ng/privateProject/seCall && cargo build 2>&1 | tail -3

# 유닛 테스트: DB 생성 + 스키마 적용
cargo test -p secall-core -- store::db::tests --nocapture

# 테스트 내용:
# 1. open_memory() 성공
# 2. migrate() 후 sessions 테이블 존재
# 3. migrate() 후 turns_fts 테이블 존재
# 4. schema_version이 config에 저장됨
# 5. 두 번 migrate() 호출해도 에러 없음 (idempotent)
```

## Risks

- **rusqlite `bundled` feature 빌드 시간**: SQLite를 소스에서 컴파일하므로 첫 빌드 ~30초 추가. CI에서는 캐시로 완화.
- **FTS5 tokenize 제한**: 초기 `unicode61`은 한국어 어절 단위 분리만 됨. 형태소 분석은 Task 06에서 외부 토크나이저를 통해 토큰화된 텍스트를 FTS5에 삽입하는 방식으로 해결 (FTS5 커스텀 토크나이저는 Rust에서 구현 복잡).
- **sqlite-vec 호환성**: Task 07에서 `sqlite-vec`을 로드할 때 `bundled` SQLite와 호환되는지 확인 필요. 일반적으로 호환되지만, 버전 불일치 가능성 있음.

## Scope Boundary

- `turns_fts`의 토크나이저는 이 태스크에서 `unicode61`로 생성. lindera 통합은 Task 06.
- 벡터 테이블(`turn_vectors`)은 이 태스크에서 생성하지 않음. Task 07에서 sqlite-vec과 함께 생성.
- Store에 데이터 삽입/조회 메서드는 이 태스크에서 구현하지 않음. 스키마 + 초기화만.
