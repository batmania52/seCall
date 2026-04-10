---
type: task
status: pass
task_number: 3
plan: secall-p16-knowledge-graph
title: CLI 서브커맨드
depends_on: [1, 2]
parallel_group: C
updated_at: 2026-04-10
---

# Task 03 — CLI 서브커맨드

**Status: PASS — 변경 없음**

v2 설계 수정에서 이 태스크의 변경은 없습니다. 기존 구현을 그대로 유지합니다.

## 구현 완료 내역

1. `crates/secall/src/commands/graph.rs` — run_build, run_stats, run_export 핸들러
2. `crates/secall/src/commands/mod.rs:14` — `pub mod graph;`
3. `crates/secall/src/main.rs` — Commands::Graph variant, GraphAction enum, match arm

## Verification

```bash
cargo check 2>&1 | tail -3
cargo run -p secall -- graph --help 2>&1
```

## Scope boundary

이 태스크의 파일은 수정 금지. Task 2 rework에서 build_graph 시그니처가 변경되지 않으므로
CLI 코드에 영향 없음.
