---
type: task
status: rework
task_number: 4
plan: secall-p16-knowledge-graph
title: Sync 통합 + MCP 확장 (rework)
depends_on: [1, 2, 3]
parallel_group: D
updated_at: 2026-04-10
---

# Task 04 — Sync 통합 + MCP 확장 (rework)

## 실패 원인 요약

| # | 위치 | 결함 | 수정 방향 |
|---|------|------|----------|
| 1 | sync.rs:118 | Phase 3.7이 `build_graph(..., None, false)` 호출 — Task 2 결함으로 증분 빌드 시 관계 엣지가 불완전 | Task 2 rework 후 build_graph 내부가 수정되므로, sync.rs 호출 코드 자체는 변경 불필요. 단, 검증 테스트를 강화 |

Task 2의 build_graph 시그니처는 변경되지 않으므로 sync.rs:118의 호출 코드는 그대로 유효합니다.
MCP graph_query도 마찬가지로 내부 로직 변경 없이 기존 구현 유지합니다.

이 태스크의 rework는 **검증 강화**에 집중합니다.

## Changed files

변경 없음 — 기존 구현을 유지합니다.

검증 대상 파일 (읽기만):
1. `crates/secall/src/commands/sync.rs:115-135` — Phase 3.7 블록
2. `crates/secall-core/src/mcp/server.rs:365-435` — graph_query 도구
3. `crates/secall-core/src/mcp/tools.rs:57-66` — GraphQueryParams
4. `crates/secall-core/src/mcp/instructions.rs` — 그래프 안내

## Change description

코드 변경 없음. Task 2 rework로 build_graph 내부의 3가지 결함이 수정되면,
sync Phase 3.7과 MCP graph_query는 자동으로 올바른 결과를 반환합니다.

### 검증 포인트

1. **sync Phase 3.7이 build_graph를 올바르게 호출하는지 확인**
   - sync.rs:118-122에서 `build_graph(&db, &config.vault.path, None, false)` 호출
   - `since: None` → 노드 upsert는 미처리 세션만 (이미 처리된 건 is_new=false)
   - `force: false` → 기존 그래프 유지, 관계 엣지만 재계산
   - 이 호출 패턴이 Task 2 rework 후에도 올바른지 확인

2. **MCP graph_query가 올바른 이웃을 반환하는지 확인**
   - 빈 DB에서 graph_query 호출 시 빈 결과 반환 (기존 동작 유지)
   - depth=1, depth=2 BFS가 올바르게 작동하는지 (기존 테스트로 검증)

3. **dry-run 출력에 Phase 3.7 안내가 포함되는지 확인**
   - sync.rs의 dry_run 블록에서 Phase 3.7 메시지 출력 확인

## Dependencies

- Task 1 (DB 스키마) — 완료
- Task 2 (Graph 코어 rework) — 완료 필수
- Task 3 (CLI) — 완료

## Verification

```bash
# 1. 타입 체크 (전체)
cargo check 2>&1 | tail -3

# 2. 기존 MCP 테스트 통과
cargo test -p secall-core -- mcp 2>&1 | tail -10

# 3. sync dry-run으로 Phase 3.7 출력 확인
cargo run -p secall -- sync --dry-run 2>&1 | grep -i "graph"

# 4. graph build 실행 후 graph stats 확인
cargo run -p secall -- graph build 2>&1
cargo run -p secall -- graph stats 2>&1

# 5. graph build 증분 (2회 실행 — 2회차에서 sessions_skipped이 증가해야 함)
cargo run -p secall -- graph build 2>&1 | grep "skipped"

# 6. MCP 서버 도구 목록에 graph_query 포함 확인
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | cargo run -p secall -- mcp 2>/dev/null | grep -o '"graph_query"'

# 7. 전체 테스트
cargo test 2>&1 | tail -10
```

## Risks

- **Task 2 rework 미완료 시**: build_graph 내부가 수정되지 않으면 기존 결함이 그대로 남음. 반드시 Task 2 완료 후 검증
- **sync 흐름에서 graph build 실패 시**: match + Err arm에서 경고만 출력하고 계속 진행 (기존 동작 유지, 변경 불필요)
- **MCP BFS depth 폭발**: depth 최대 3 제한 (기존 동작 유지)

## Scope boundary — 수정 금지 파일

- `crates/secall-core/src/store/**` — Task 1 영역
- `crates/secall-core/src/graph/**` — Task 2 영역
- `crates/secall/src/commands/graph.rs` — Task 3 영역
- `crates/secall/src/main.rs` — Task 3 영역
- `crates/secall-core/src/search/**` — 검색 엔진 수정 금지
- `crates/secall-core/src/ingest/**` — 파서 수정 금지
