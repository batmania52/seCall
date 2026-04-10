---
type: task
status: pass
task_number: 1
plan: secall-p16-knowledge-graph
title: DB 스키마 + 마이그레이션
depends_on: []
parallel_group: A
updated_at: 2026-04-10
---

# Task 01 — DB 스키마 + 마이그레이션

**Status: PASS — 변경 없음**

v2 설계 수정에서 이 태스크의 변경은 없습니다. 기존 구현을 그대로 유지합니다.

## 구현 완료 내역

1. `crates/secall-core/src/store/schema.rs` — graph_nodes, graph_edges 테이블 DDL, CURRENT_SCHEMA_VERSION = 3
2. `crates/secall-core/src/store/db.rs` — v3 마이그레이션 블록
3. `crates/secall-core/src/store/graph_repo.rs` — CRUD: upsert_graph_node, upsert_graph_edge, get_neighbors, graph_stats, list_graph_nodes, clear_graph, delete_graph_for_session, list_graphed_session_ids
4. `crates/secall-core/src/store/mod.rs` — `pub mod graph_repo;`

## 추가 필요 메서드 (Task 2 rework에서 사용)

Task 2 rework에서 `delete_relation_edges()` 메서드가 필요합니다. **이 메서드는 Task 2에서 graph_repo.rs에 추가**합니다 (Task 1 영역이지만, rework 범위 내에서 허용).

```rust
/// same_project / same_day 등 관계 엣지만 전체 삭제
pub fn delete_relation_edges(&self, relations: &[&str]) -> Result<usize>
```

## Verification

```bash
cargo test -p secall-core -- store 2>&1 | tail -10
cargo test -p secall-core -- test_schema_version 2>&1 | tail -5
cargo test -p secall-core -- test_graph 2>&1 | tail -10
```

## Scope boundary

이 태스크의 파일은 Task 2 rework에서 `delete_relation_edges()` 1개 메서드만 추가 허용. 그 외 수정 금지.
