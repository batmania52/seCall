---
type: task
status: draft
plan: secall-refactor-p2
task_number: 4
title: "BLOB 검증 + CLI/MCP 테스트"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 04: BLOB 검증 + CLI/MCP 테스트

## 문제

### BLOB 검증 없음
- `insert_vector()` (vector.rs:269-284): embedding 차원 검증 없이 BLOB 저장. 384차원과 1024차원이 같은 테이블에 혼재 가능.
- `bytes_to_floats()` (vector.rs:337-342): `chunks_exact(4)` 사용. BLOB 길이가 4의 배수가 아니면 뒤 바이트 무시 (silent truncation).

### 테스트 부재
- `crates/secall/src/` (CLI): `#[test]` 0건
- `crates/secall-core/src/mcp/` (MCP 서버): `#[cfg(test)]` 0건

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/search/vector.rs:269-284` | 수정 | `insert_vector()`에 차원 검증 추가 |
| `crates/secall-core/src/search/vector.rs:337-342` | 수정 | `bytes_to_floats()`에 길이 체크 추가 |
| `crates/secall-core/src/search/vector.rs` (tests) | 추가 | 차원 불일치, BLOB 길이 에러 테스트 |
| `crates/secall-core/src/mcp/server.rs` | 추가 | `#[cfg(test)]` 모듈 — tool 단위 테스트 |
| `crates/secall/tests/cli_smoke.rs` | 신규 | CLI 통합 테스트 (init, status, lint) |

## Change description

### Part A: BLOB 차원 검증

#### Step 1: insert_vector()에 차원 검증

```rust
// vector.rs:269-284 — 변경 전
pub fn insert_vector(
    &self,
    embedding: &[f32],
    session_id: &str,
    turn_index: u32,
    chunk_seq: u32,
    model: &str,
) -> Result<i64> {
    let bytes = floats_to_bytes(embedding);
    self.conn().execute(...)?;
    Ok(self.conn().last_insert_rowid())
}

// 변경 후
pub fn insert_vector(
    &self,
    embedding: &[f32],
    session_id: &str,
    turn_index: u32,
    chunk_seq: u32,
    model: &str,
) -> Result<i64> {
    // 차원 검증: 빈 임베딩 방지
    if embedding.is_empty() {
        anyhow::bail!("empty embedding for session={session_id} turn={turn_index}");
    }

    // 기존 데이터와 차원 일치 확인 (첫 삽입 시 건너뜀)
    let existing_dim: Option<usize> = self.conn().query_row(
        "SELECT LENGTH(embedding) FROM turn_vectors LIMIT 1",
        [],
        |row| row.get::<_, i64>(0).map(|n| n as usize / 4),
    ).ok();

    if let Some(dim) = existing_dim {
        if embedding.len() != dim {
            anyhow::bail!(
                "embedding dimension mismatch: expected {dim}, got {} (session={session_id})",
                embedding.len()
            );
        }
    }

    let bytes = floats_to_bytes(embedding);
    self.conn().execute(
        "INSERT INTO turn_vectors(session_id, turn_index, chunk_seq, model, embedded_at, embedding)
         VALUES (?1, ?2, ?3, ?4, datetime('now'), ?5)",
        rusqlite::params![session_id, turn_index as i64, chunk_seq as i64, model, bytes],
    )?;
    Ok(self.conn().last_insert_rowid())
}
```

> 차원 검증 쿼리는 첫 행 하나만 읽으므로 성능 영향 없음.

#### Step 2: bytes_to_floats() 방어 코드

```rust
// vector.rs:337-342 — 변경 전
fn bytes_to_floats(bytes: &[u8]) -> Vec<f32> {
    bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect()
}

// 변경 후
fn bytes_to_floats(bytes: &[u8]) -> Vec<f32> {
    if bytes.len() % 4 != 0 {
        // 손상된 BLOB — 빈 벡터 반환 (cosine_distance가 1.0 반환)
        tracing::warn!(blob_len = bytes.len(), "corrupt vector BLOB (not multiple of 4 bytes)");
        return Vec::new();
    }
    bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect()
}
```

#### Step 3: BLOB 검증 테스트

```rust
// vector.rs tests — 추가
#[test]
fn test_insert_vector_empty_rejected() {
    let db = Database::open_memory().unwrap();
    db.init_vector_table().unwrap();
    let result = db.insert_vector(&[], "s1", 0, 0, "test");
    assert!(result.is_err());
}

#[test]
fn test_insert_vector_dimension_mismatch() {
    let db = Database::open_memory().unwrap();
    db.init_vector_table().unwrap();

    // 첫 삽입: 3차원
    db.insert_vector(&[1.0, 0.0, 0.0], "s1", 0, 0, "test").unwrap();

    // 두 번째 삽입: 2차원 → 에러
    let result = db.insert_vector(&[1.0, 0.0], "s2", 0, 0, "test");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("dimension mismatch"));
}

#[test]
fn test_bytes_to_floats_corrupt_blob() {
    // 5 bytes (not multiple of 4)
    let result = bytes_to_floats(&[0, 0, 0, 0, 0]);
    assert!(result.is_empty());
}
```

### Part B: MCP 서버 테스트

#### Step 4: mcp/server.rs에 테스트 모듈 추가

MCP 서버의 tool 메서드(recall, get, status)를 직접 호출하는 단위 테스트.

```rust
// mcp/server.rs — 파일 하단에 추가
#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::bm25::Bm25Indexer;
    use crate::search::tokenizer::LinderaKoTokenizer;
    use crate::store::db::Database;

    fn make_server() -> SeCallMcpServer {
        let db = Database::open_memory().unwrap();
        let tok = LinderaKoTokenizer::new().unwrap();
        let engine = SearchEngine::new(Bm25Indexer::new(Box::new(tok)), None);
        SeCallMcpServer::new(Arc::new(Mutex::new(db)), Arc::new(engine))
    }

    #[tokio::test]
    async fn test_status_tool() {
        let server = make_server();
        let params = StatusParams {};
        let result = server.status(Parameters(params)).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        // status 출력에 "Sessions:" 또는 유사 내용 포함
        let text = call_result.content.first()
            .and_then(|c| if let Content::Text(t) = c { Some(t.text.as_str()) } else { None })
            .unwrap_or("");
        assert!(text.contains("Session") || text.contains("session"));
    }

    #[tokio::test]
    async fn test_recall_empty_db() {
        let server = make_server();
        let params = RecallParams {
            query: "테스트 검색어".to_string(),
            query_type: None,
            limit: Some(5),
            project: None,
            agent: None,
            since: None,
        };
        let result = server.recall(Parameters(params)).await;
        assert!(result.is_ok());
    }
}
```

> `SeCallMcpServer`의 tool 메서드가 `&self` + `Parameters<T>`를 받으므로 직접 호출 가능. rmcp 서버 시작 없이 로직만 테스트.

### Part C: CLI 통합 테스트

#### Step 5: crates/secall/tests/cli_smoke.rs 생성

```rust
// crates/secall/tests/cli_smoke.rs — 신규
use std::process::Command;

fn secall_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_secall"))
}

#[test]
fn test_cli_help() {
    let output = secall_cmd().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("secall"));
}

#[test]
fn test_cli_version() {
    let output = secall_cmd().arg("--version").output().unwrap();
    assert!(output.status.success());
}

#[test]
fn test_cli_status_without_db() {
    // DB가 없는 상태에서 status 실행 → 패닉하지 않아야 함
    let output = secall_cmd()
        .arg("status")
        .env("SECALL_DB_PATH", "/tmp/secall-test-nonexistent.db")
        .output()
        .unwrap();
    // exit code 0 또는 비 패닉 에러
    // (DB가 없으면 "Run `secall init` first" 메시지 출력)
    assert!(!String::from_utf8_lossy(&output.stderr).contains("panic"));
}

#[test]
fn test_cli_lint_without_db() {
    let output = secall_cmd()
        .arg("lint")
        .env("SECALL_DB_PATH", "/tmp/secall-test-nonexistent.db")
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&output.stderr).contains("panic"));
}
```

> `CARGO_BIN_EXE_secall`은 cargo test가 자동으로 빌드한 바이너리 경로를 제공.
> `SECALL_DB_PATH` 환경변수 지원은 현재 코드에 없을 수 있음 → `get_default_db_path()`가 환경변수를 지원하는지 확인 필요. 미지원 시 해당 테스트 조건부 스킵.

## Dependencies

- 없음 (독립 실행 가능)
- Task 01 (tracing)이 완료된 후라면 `bytes_to_floats()`의 경고가 `tracing::warn!`으로 출력됨. 그전이면 `eprintln!` 사용.

## Verification

```bash
# 1. 컴파일 확인
cargo check

# 2. 벡터 BLOB 테스트
cargo test -p secall-core vector

# 3. MCP 서버 테스트
cargo test -p secall-core mcp

# 4. CLI 통합 테스트
cargo test -p secall cli_smoke

# 5. 전체 테스트 회귀 없음
cargo test
```

> **[Developer 필수]** subtask-done 시그널 전에 위 명령의 실행 결과를 result 문서에 기록하세요. 형식: `✅ 명령 — exit 0` 또는 `❌ 명령 — 에러 내용 (사유)`. 검증 증빙 미제출 시 리뷰에서 conditional 처리됩니다.

## Risks

- **차원 검증 성능**: 매 insert마다 1행 SELECT. 벡터 테이블이 비어있으면 스킵. 배치 임베딩 시 반복 호출되나, SQLite 메모리 쿼리이므로 μs 수준.
- **MCP 테스트 의존성**: `SeCallMcpServer::new()`가 `LinderaKoTokenizer`를 요구. lindera 초기화가 느림 → 테스트 시간 증가. `#[ignore]` 태그 고려.
- **CLI 테스트 환경 의존**: `env!("CARGO_BIN_EXE_secall")`는 `cargo test`로만 동작. IDE 개별 테스트 실행 시 실패 가능.
- **SECALL_DB_PATH**: 현재 `get_default_db_path()`가 이 환경변수를 지원하지 않으면 CLI 테스트에서 실제 사용자 DB를 건드릴 위험. tempdir 기반으로 전환하거나 환경변수 지원 추가 필요.

## Scope boundary

다음 파일은 이 task에서 수정하지 않음:
- `crates/secall-core/src/search/embedding.rs` — Embedder trait. 차원 정보는 `dimensions()` 메서드로 이미 제공됨.
- `crates/secall-core/src/store/schema.rs` — turn_vectors 테이블 DDL 변경 없음.
- `.github/workflows/` — CI 파이프라인은 이 task 범위 외.
