# Implementation Result: seCall Refactor P1 — 에러 처리 + 데이터 정합성

> Developer: claude
> Branch: N/A
> Date: 2026-04-06 10:21
> Plan Revision: 0

---

## Summary

전체 테스트 110개 모두 통과.

---

## Rework 결과

**수정 사항**: `crates/secall-core/src/search/bm25.rs:81-94`

- **변경 전**: `insert_turn()`/`insert_fts()` 실패 시 `stats.errors += 1` 후 계속 진행 → `with_transaction()` 클로저가 `Ok`를 반환해 COMMIT
- **변경 후**: 두 호출 모두 `?`로 에러 전파 → DB 쓰기 실패 시 즉시 `Err` 반환 → `with_transaction()` ROLLBACK + vault 파일 cleanup 보장

```
Verification results for Rework:
✅ cargo check — exit 0
✅ cargo test — 110 passed, 0 failed
```

## Subtask Results

### 1. 전체 테스트 110개 모두 통과.

---

## Rework 결과

**수정 사항**: `crates/secall-core/src/search/bm25.rs:81-94`

- **변경 전**: `insert_turn()`/`insert_fts()` 실패 시 `stats.errors += 1` 후 계속 진행 → `with_transaction()` 클로저가 `Ok`를 반환해 COMMIT
- **변경 후**: 두 호출 모두 `?`로 에러 전파 → DB 쓰기 실패 시 즉시 `Err` 반환 → `with_transaction()` ROLLBACK + vault 파일 cleanup 보장

```
Verification results for Rework:
✅ cargo check — exit 0
✅ cargo test — 110 passed, 0 failed
```

