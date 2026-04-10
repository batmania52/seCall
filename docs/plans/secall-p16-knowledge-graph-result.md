# Implementation Result: seCall P16 — Knowledge Graph 빌드 (설계 수정)

> Developer: unknown
> Branch: N/A
> Date: 2026-04-10 14:28
> Plan Revision: 0

---

## Summary

---

## Rework Verification 결과

```
Verification results for Rework (build.rs:110 fix):
✅ `cargo check -p secall-core` — exit 0
✅ `cargo test -p secall-core -- graph::build` — 6 passed (기존 5 + 신규 1)
✅ `cargo test -p secall-core` — 184 passed, 0 failed
```

---

## 수정 내용 요약

**결함 (build.rs:110)**: `include_in_relations = session_is_new || (!force && already)` 조건이 fresh DB + `--since` 조합에서 since 이전 세션을 `all_frontmatters`에서 제외 → 관계 계산이 전체 vault를 대상으로 하지 않는 계약 위반.

**수정**:
- `include_in_relations` 조건 제거 → 모든 세션을 `all_frontmatters`에 포함
- `needs_minimal_node` 플래그 추가: `!session_is_new && !already` → DB에 없고 is_new도 아닌 세션
- 트랜잭션 내에서 `needs_minimal_node=true` 세션의 session 노드를 최소 형태로 먼저 삽입 → FK 제약 충족
- `extract_session_relations(&all_frontmatters)` 는 그대로 전체 vault 대상 호출

**신규 테스트 `test_since_includes_prior_sessions_in_relations_fresh_db`**: fresh DB + `--since` 시나리오에서 since 이전 세션이 관계 계산에 참여하는지 명시적으로 검증.

## Subtask Results

### 1. ---

## Rework Verification 결과

```
Verification results for Rework (build.rs:110 fix):
✅ `cargo check -p secall-core` — exit 0
✅ `cargo test -p secall-core -- graph::build` — 6 passed (기존 5 + 신규 1)
✅ `cargo test -p secall-core` — 184 passed, 0 failed
```

---

## 수정 내용 요약

**결함 (build.rs:110)**: `include_in_relations = session_is_new || (!force && already)` 조건이 fresh DB + `--since` 조합에서 since 이전 세션을 `all_frontmatters`에서 제외 → 관계 계산이 전체 vault를 대상으로 하지 않는 계약 위반.

**수정**:
- `include_in_relations` 조

