# Implementation Result: P28 — Graph Semantic CLI 플래그 확장 (#30)

> Developer: unknown
> Branch: N/A
> Date: 2026-04-16 12:52
> Plan Revision: 0

---

## Summary

---

Verification results for Task 04 (Rework):

```
✅ `cargo check -p secall-core` — exit 0
✅ `cargo test -p secall-core test_graph_env_override` — 4 passed (0 failed)
```

### 수정 내용 요약

**변경 파일**: `crates/secall-core/src/vault/config.rs`

4건 모두 동일한 패턴으로 수정:

1. `tests` 모듈 상단에 `static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());` 추가
2. 각 env 테스트 시작 시 `let _guard = ENV_MUTEX.lock().unwrap();`로 뮤텍스 획득
3. **`remove_var`를 `apply_env_overrides()` 직후로 이동** — panic 발생 시에도 cleanup이 되도록 assert를 마지막에 배치 (guard drop 전에 cleanup 완료)

동일 프로세스 내 병렬 테스트 스레드들이 ENV_MUTEX를 경쟁적으로 획득하므로 환경변수 접근이 완전히 직렬화됩니다.

## Subtask Results

### 1. ---

Verification results for Task 04 (Rework):

```
✅ `cargo check -p secall-core` — exit 0
✅ `cargo test -p secall-core test_graph_env_override` — 4 passed (0 failed)
```

### 수정 내용 요약

**변경 파일**: `crates/secall-core/src/vault/config.rs`

4건 모두 동일한 패턴으로 수정:

1. `tests` 모듈 상단에 `static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());` 추가
2. 각 env 테스트 시작 시 `let _guard = ENV_MUTEX.lock().unwrap();`로 뮤텍스 획득
3. **`remove_var`를 `apply_env_overrides()` 직후로 이동** — panic 발생 시에도 cleanup이 되도록

