# Review Report: seCall P6 — 안정성 + 성능 개선 — Round 1

> Verdict: fail
> Reviewer: 
> Date: 2026-04-06 19:15
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall-core/src/vault/git.rs:47 — `VaultGit::pull()`가 여전히 더러운 작업 트리에서 바로 `git pull --rebase`를 실행합니다. Task 02 문서가 요구한 `pull()` 수준의 안전장치가 없어, `sync.rs` 외의 호출 경로에서는 동일한 pull 실패가 재발할 수 있습니다.
2. crates/secall/src/commands/ingest.rs:173 — 벡터 임베딩이 `for` 루프 안에서 각 세션별로 순차 `await`됩니다. Task 03의 목표였던 병렬/배치 임베딩이 구현되지 않아 성능 병목이 그대로 남아 있습니다.

## Recommendations

1. Task 02는 `sync.rs` 선행 호출과 별개로 `VaultGit::pull()` 내부도 self-contained 하게 보호해 task 계약과 실제 안전성을 맞추는 편이 낫습니다.
2. Task 03는 실제 병렬화가 어렵다면 plan/task 문서를 하향 조정하지 말고, 최소한 `JoinSet` 또는 embed batch 단위 병렬 처리처럼 측정 가능한 개선이 있는 형태로 재작업하는 것이 맞습니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | ANN reserve 사전 할당 | ✅ done |
| 2 | Sync pull 안전성 | ✅ done |
| 3 | Ingest 임베딩 병렬화 | ✅ done |

