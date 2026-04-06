# Review Report: seCall Refactor P1 — 에러 처리 + 데이터 정합성 — Round 1

> Verdict: fail
> Reviewer: 
> Date: 2026-04-06 10:18
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall-core/src/search/bm25.rs:81 — `insert_turn()`/`insert_fts()` 실패를 `Err`로 전파하지 않고 `stats.errors += 1` 후 계속 진행합니다. 그래서 [ingest.rs](/Users/d9ng/privateProject/seCall/crates/secall/src/commands/ingest.rs#L97) 의 `with_transaction()` 클로저는 성공으로 끝나 `COMMIT` 되고, Task 03이 요구한 "DB write 실패 시 rollback + vault cleanup"이 보장되지 않습니다. 예를 들어 turn/FTS insert 일부가 실패해도 `sessions` row와 `vault_path` update가 커밋되어 부분 반영 상태가 남습니다.

## Recommendations

1. [docs/plans/secall-refactor-p1-result.md](/Users/d9ng/privateProject/seCall/docs/plans/secall-refactor-p1-result.md#L18) 에는 Task 01의 수동 검증 명령 결과가 없고, Task 03도 task 파일에 적힌 수동 명령 대신 서술형 설명만 있습니다. 재작업 시 task별 Verification 항목을 문서에 그대로 대응시켜 남기는 편이 안전합니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | ingest.rs 에러 전파 | ✅ done |
| 2 | db.rs Result 반환 전환 | ✅ done |
| 3 | ingest 트랜잭션 래핑 | ✅ done |
| 4 | Codex/Gemini 타임스탬프 복원 | ✅ done |

