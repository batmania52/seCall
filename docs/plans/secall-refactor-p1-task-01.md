---
type: task
status: draft
plan: secall-refactor-p1
task_number: 1
title: "ingest.rs 에러 전파"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 01: ingest.rs 에러 전파

## 문제

`crates/secall/src/commands/ingest.rs`에 9개 에러 삼킴 패턴이 있어 DB 장애 시 사용자가 인지하지 못한 채 데이터가 누락됨.

| 라인 | 패턴 | 영향 |
|------|-------|------|
| 55 | `session_exists().unwrap_or(false)` | DB 장애 시 중복 체크 건너뜀 → 중복 인덱싱 |
| 63 | `session_exists().unwrap_or(false)` | 동일 |
| 79 | `index_session().unwrap_or_default()` | 인덱싱 실패 무시 → 검색 누락 |
| 83 | `let _ = update_session_vault_path()` | vault_path 미저장 → `get --full` 실패 |
| 89 | `let _ = run_post_ingest_hook()` | hook 실패 무시 |

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall/src/commands/ingest.rs:48-96` | 수정 | ingest 루프 내 에러 처리 패턴 변경 |

## Change description

### 원칙

- **중복 체크 실패** (line 55, 63): DB 에러 시 해당 세션을 skip + 경고. 인덱싱하지 않음 (안전 우선).
- **인덱싱 실패** (line 79): 에러 시 경고 + errors 카운터 증가. vault 파일은 이미 작성되었으므로 삭제하지 않음 (Task 03에서 트랜잭션으로 해결).
- **vault_path 저장 실패** (line 83): 경고 출력 (치명적이지 않으나 사용자 인지 필요).
- **hook 실패** (line 89): 경고 출력 (외부 프로세스이므로 전파하지 않음).

### Step 1: 중복 체크 에러 전파 (line 55, 63)

```rust
// 변경 전 (line 55)
if db.session_exists(session_id_hint).unwrap_or(false) {
    skipped += 1;
    continue;
}

// 변경 후
match db.session_exists(session_id_hint) {
    Ok(true) => {
        skipped += 1;
        continue;
    }
    Ok(false) => {} // proceed
    Err(e) => {
        eprintln!("warn: DB check failed for {}, skipping: {e}", session_path.display());
        errors += 1;
        continue;
    }
}
```

> line 63도 동일 패턴 적용 (`session.id` 대상).

### Step 2: 인덱싱 실패 경고 (line 79)

```rust
// 변경 전
let stats = engine.index_session(&db, &session).await.unwrap_or_default();

// 변경 후
let stats = match engine.index_session(&db, &session).await {
    Ok(s) => s,
    Err(e) => {
        eprintln!("warn: indexing failed for {}: {e}", session_path.display());
        errors += 1;
        continue;  // vault 파일은 남겨둠 (재인덱싱 가능)
    }
};
```

### Step 3: vault_path 저장 실패 경고 (line 83)

```rust
// 변경 전
let _ = db.update_session_vault_path(&session.id, &vault_path_str);

// 변경 후
if let Err(e) = db.update_session_vault_path(&session.id, &vault_path_str) {
    eprintln!("warn: failed to store vault_path for {}: {e}", &session.id[..8]);
}
```

### Step 4: hook 실패 경고 (line 89)

```rust
// 변경 전
let _ = run_post_ingest_hook(&config, &session, &md_path);

// 변경 후
if let Err(e) = run_post_ingest_hook(&config, &session, &md_path) {
    eprintln!("warn: post-ingest hook failed for {}: {e}", &session.id[..8]);
}
```

### Step 5: 에러 카운트에 따른 exit code (선택)

현재 `run()` 함수가 `Result<()>`를 반환. errors > 0이어도 Ok(())를 반환하므로 exit code는 0.
이 동작은 유지 — Summary 메시지에 errors 카운트가 출력되므로 사용자가 인지 가능.

## Dependencies

- 없음 (독립 실행 가능)
- Task 02 (db.rs Result 반환)와 병렬 실행 가능하나, db.rs 메서드 시그니처 변경 시 이 task의 코드도 조정 필요할 수 있음. 실행 순서 유연.

## Verification

```bash
# 1. 컴파일 확인
cargo check -p secall

# 2. 전체 테스트 회귀 없음
cargo test

# 3. 수동 확인: ingest 실행 후 Summary에 에러 카운트 표시
# secall ingest --auto 2>&1 | grep -E "Summary|warn"
```

> **[Developer 필수]** subtask-done 시그널 전에 위 명령의 실행 결과를 result 문서에 기록하세요. 형식: `✅ 명령 — exit 0` 또는 `❌ 명령 — 에러 내용 (사유)`. 검증 증빙 미제출 시 리뷰에서 conditional 처리됩니다.

## Risks

- **인덱싱 실패 시 vault 파일 잔존**: Step 2에서 `continue` 후 vault 파일이 남음. 이는 Task 03 (트랜잭션)에서 근본 해결. 현 task에서는 사용자 인지만 보장.
- **eprintln 형식**: 현재 `warn:` prefix를 사용 중. P2에서 tracing 도입 시 일괄 변환 예정이므로 형식은 기존 패턴 유지.

## Scope boundary

다음 파일은 이 task에서 수정하지 않음:
- `crates/secall-core/src/search/bm25.rs` — `index_session()` 내부 에러 처리는 이미 `Result` 반환
- `crates/secall-core/src/hooks/mod.rs` — hook 내부 에러 처리는 별도 이슈
- `crates/secall/src/commands/ingest.rs:108-165` — `collect_paths()`, `find_session_by_id()` 등 파일 탐색 로직은 이 task 범위 외
