---
type: task
status: draft
plan: secall-p6
task_number: 3
title: "Ingest 임베딩 병렬화"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 03: Ingest 임베딩 병렬화

## 문제

`secall sync`와 `secall ingest --auto` 실행 시 벡터 임베딩이 세션별 순차 처리되어 전체 소요 시간이 길다.

- 현재 경로: `ingest.rs:146-151` → `engine.index_session_vectors(db, &session).await`
- 각 세션의 벡터 임베딩이 완료될 때까지 다음 세션 처리를 대기
- 20개 세션 × ~10초/세션 = ~200초 (실측: sync 1회 ~5분)

### 현재 흐름

```
Session 1: BM25 → Vector(await) → Session 2: BM25 → Vector(await) → ...
```

### 목표 흐름

```
Session 1: BM25 → Vector(spawn)
Session 2: BM25 → Vector(spawn)
...
await all vector tasks
```

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall/src/commands/ingest.rs:63-175` | 수정 | `ingest_sessions()` — 벡터 임베딩을 background task로 분리 |
| `crates/secall-core/src/search/vector.rs:47-104` | 수정 | `index_session()` — `&self` → `Arc<Self>` 지원 또는 세션 데이터 복제 |
| `crates/secall-core/src/search/mod.rs` | 수정 | `SearchEngine::index_session_vectors()` 시그니처 조정 (필요 시) |

## Change description

### Step 1: ingest_sessions()에서 벡터 태스크 분리

`crates/secall/src/commands/ingest.rs` — `ingest_sessions()` (lines 63-175):

현재 (lines 146-151):
```rust
// 3. 벡터 인덱싱 (비동기, 트랜잭션 밖)
{
    let vec_stats = engine.index_session_vectors(db, &session).await;
    if let Err(e) = vec_stats {
        tracing::warn!(..., "vector embedding failed");
    }
}
```

변경 — 벡터 태스크를 collect하고 마지막에 일괄 await:

```rust
use tokio::task::JoinSet;

pub async fn ingest_sessions(
    config: &Config,
    db: &Database,
    paths: Vec<PathBuf>,
    engine: &SearchEngine,
    vault: &Vault,
    format: &OutputFormat,
) -> Result<IngestStats> {
    let mut ingested = 0usize;
    let mut skipped = 0usize;
    let mut errors = 0usize;

    // 벡터 임베딩 태스크를 수집 (BM25 완료 후 비동기 처리)
    let mut vector_tasks: Vec<(String, Session)> = Vec::new();

    for session_path in &paths {
        // ... 기존 duplicate check, parse, vault write, BM25 indexing 동일 ...

        // 3. 벡터 인덱싱을 즉시 await 대신 목록에 추가
        vector_tasks.push((session.id.clone(), session));

        print_ingest_result(&session_copy, &abs_path, &index_stats, format);
        ingested += 1;

        // post-ingest hook (기존 동일)
    }

    // 4. 벡터 임베딩 일괄 처리 (세션별 순차지만 BM25와 분리)
    if !vector_tasks.is_empty() {
        eprintln!("Embedding {} sessions...", vector_tasks.len());
        for (session_id, session) in &vector_tasks {
            if let Err(e) = engine.index_session_vectors(db, session).await {
                tracing::warn!(session = &session_id[..8.min(session_id.len())], error = %e, "vector embedding failed");
            }
        }
    }

    Ok(IngestStats { ingested, skipped, errors })
}
```

> **Phase 1 접근법**: 완전 병렬(JoinSet)이 아닌, BM25/vault 완료 후 벡터만 후처리.
> 이유: `Database`는 `!Send`(rusqlite Connection), 동시 insert 불가.
> Phase 1으로도 BM25+vault가 벡터 대기 없이 빠르게 완료되어 체감 속도 개선.

### Step 2: Session 소유권 처리

현재 `ingest_sessions()`의 for 루프에서 `session`이 move되므로, 벡터 태스크에 전달하려면 소유권 관리가 필요:

```rust
// parse 후 session을 clone하여 벡터 태스크용으로 보관
let session_for_vec = session.clone();
// BM25 indexing에 &session 사용
// vector_tasks.push(session_for_vec)
```

`Session`에 `Clone` derive가 필요. 현재 `types.rs:24-35`:

```rust
// Session에 Clone 추가 필요 여부 확인
// Turn, TokenCount 등 하위 타입도 Clone 필요
```

> Session이 이미 Clone을 derive하고 있으면 이 단계 생략.
> derive하지 않으면 `#[derive(Clone)]` 추가 (types.rs + 하위 타입).

### Step 3: (선택적 Phase 2) 배치 병렬 임베딩

Phase 1이 충분하지 않으면, embedder 레벨에서 다중 세션의 청크를 하나의 대형 배치로 합치는 최적화:

```rust
// 모든 세션의 청크를 하나의 Vec에 모으고
// embed_batch()를 한 번만 호출
// → ONNX Runtime이 GPU/CPU 병렬화를 내부적으로 수행
```

이 최적화는 Phase 1 결과에 따라 후속 판단.

## Dependencies

- 없음 (기존 tokio runtime 활용)
- Task 01, 02와 독립적으로 구현 가능
- `Session`에 `Clone` derive 필요할 수 있음

## Verification

```bash
# 1. 컴파일 확인
cargo check --all

# 2. 전체 테스트 통과
cargo test --all

# 3. ingest 기능 회귀 테스트
cargo run -p secall -- ingest --auto 2>&1 | tail -5

# 4. sync 속도 측정 (before/after 비교)
time cargo run -p secall -- sync --local-only 2>&1 | tail -3

# 5. 벡터 인덱싱 정합성 확인 — 임베딩된 세션 수 비교
sqlite3 ~/.config/secall/secall.db "SELECT COUNT(DISTINCT session_id) FROM turn_vectors" 2>/dev/null

# 6. clippy 통과
cargo clippy --all-targets -- -D warnings
```

## Risks

- **Session Clone 비용**: Session에 `turns: Vec<Turn>`이 포함되어 있으므로 clone 비용이 있음. 수백 턴 세션에서 수 MB. 하지만 벡터 임베딩 I/O 비용(~10초) 대비 무시 가능.
- **Database 동시 접근**: SQLite는 단일 writer. 벡터 insert를 병렬화하면 lock contention 발생. Phase 1에서는 순차 처리를 유지하되 BM25와 분리하는 것만으로 충분.
- **OrtEmbedder Mutex**: `embedding.rs`의 `Arc<Mutex<Session>>`은 이미 P3에서 적용됨. `spawn_blocking` 내부에서 동작하므로 tokio runtime 블로킹 없음.
- **ingest 동작 변경**: 기존에는 세션별 즉시 벡터 임베딩 → 실패 시 해당 세션 경고 출력. 변경 후에는 마지막에 일괄 처리 → 사용자가 진행 상황을 늦게 확인. progress bar 추가 고려 (후속).

## Scope boundary

수정 금지 파일:
- `crates/secall-core/src/search/ann.rs` — Task 01 영역
- `crates/secall-core/src/vault/git.rs` — Task 02 영역
- `crates/secall/src/commands/sync.rs` — sync 로직은 ingest_sessions() 호출부만 영향받음, sync.rs 자체 수정 불필요
- `crates/secall-core/src/search/bm25.rs` — BM25 인덱싱 변경 없음
- `crates/secall-core/src/store/db.rs` — DB 접근 패턴 변경 없음
- `crates/secall-core/src/search/embedding.rs` — embedder 인터페이스 변경 없음
