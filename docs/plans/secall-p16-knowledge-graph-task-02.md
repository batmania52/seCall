---
type: task
status: rework
task_number: 2
plan: secall-p16-knowledge-graph
title: Graph 코어 모듈 (rework)
depends_on: [1]
parallel_group: B
updated_at: 2026-04-10
---

# Task 02 — Graph 코어 모듈 (rework)

## 실패 원인 요약

| # | 위치 | 결함 | 수정 방향 |
|---|------|------|----------|
| 1 | build.rs:60 | `--since` 필터가 경로의 모든 10글자 세그먼트를 날짜로 간주. `session001` 같은 파일명도 매칭 | 부모 디렉토리명만 검사 + YYYY-MM-DD 정규식 검증 |
| 2 | build.rs:122 | INSERT OR IGNORE만 사용. 중간 세션 B 추가 시 기존 A→C 엣지 미삭제 | 관계 엣지(same_project, same_day) 전체 DELETE 후 재계산 |
| 3 | build.rs:53-66 | since 필터 적용 시 범위 밖 세션이 관계 계산에서 제외 | since는 노드 upsert에만 적용, 관계 계산은 전체 vault 대상 |

## Changed files

1. `crates/secall-core/src/graph/build.rs` — 전면 수정 (3개 결함 수정 + 테스트 추가)
2. `crates/secall-core/src/store/graph_repo.rs` — `delete_relation_edges()` 메서드 1개 추가 (line 201 이후)

extract.rs, export.rs, mod.rs는 변경 없음.

## Change description

### 1. graph_repo.rs — `delete_relation_edges()` 추가 (line 201 이후)

```rust
/// 특정 relation 타입의 엣지를 전체 삭제.
/// 증분 빌드에서 same_project/same_day를 전체 재계산할 때 사용.
pub fn delete_relation_edges(&self, relations: &[&str]) -> Result<usize> {
    let placeholders: Vec<String> = relations.iter().enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect();
    let sql = format!(
        "DELETE FROM graph_edges WHERE relation IN ({})",
        placeholders.join(", ")
    );
    let params: Vec<&dyn rusqlite::types::ToSql> = relations.iter()
        .map(|r| r as &dyn rusqlite::types::ToSql)
        .collect();
    let deleted = self.conn().execute(&sql, params.as_slice())?;
    Ok(deleted)
}
```

### 2. build.rs — 전면 수정

**수정 1: `--since` 필터 (line 56-67 대체)**

기존:
```rust
let passes = path_str
    .split('/')
    .any(|part| part.len() == 10 && part >= since_date);
```

수정:
```rust
// 부모 디렉토리명만 검사 (raw/sessions/YYYY-MM-DD/파일.md)
fn is_date_dir(name: &str) -> bool {
    name.len() == 10
        && name.as_bytes()[4] == b'-'
        && name.as_bytes()[7] == b'-'
        && name[..4].chars().all(|c| c.is_ascii_digit())
        && name[5..7].chars().all(|c| c.is_ascii_digit())
        && name[8..10].chars().all(|c| c.is_ascii_digit())
}

// since 필터: 부모 디렉토리명이 YYYY-MM-DD 형식이고 since_date 이상인지만 확인
if let Some(since_date) = since {
    let parent_name = path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if is_date_dir(parent_name) && parent_name < since_date {
        // since 이전 세션 → 노드 upsert 대상에서는 제외하지만,
        // all_frontmatters에는 포함하여 관계 계산에 참여
        // (아래 수정 3에서 처리)
    }
}
```

**수정 2: 관계 엣지 전체 재계산 (line 96-126 대체)**

기존: INSERT OR IGNORE만 사용
수정: 트랜잭션 시작 시 same_project/same_day 엣지를 전체 삭제 후 재계산

```rust
db.with_transaction(|| {
    // [핵심 수정] 관계 엣지를 전체 삭제 후 재계산
    // 이유: 인접 관계(A→B→C)는 전체 순서에 의존하므로 부분 갱신 불가.
    // 중간 세션 B 추가 시 기존 A→C를 삭제하고 A→B, B→C로 교체해야 함.
    db.delete_relation_edges(&["same_project", "same_day"])?;

    // 개별 노드/엣지: 신규 세션만 upsert (belongs_to, by_agent, uses_tool)
    for (fm, &new_session) in all_frontmatters.iter().zip(is_new.iter()) {
        if !new_session { continue; }
        let result = extract_from_frontmatter(fm);
        for node in &result.nodes { ... }
        for edge in &result.edges { ... }
    }

    // 세션 간 관계 엣지: 전체 세션 대상으로 재계산 후 삽입
    let relation_edges = extract_session_relations(&all_frontmatters);
    for edge in &relation_edges { ... }

    Ok(())
})?;
```

**수정 3: since 필터를 노드 upsert에만 적용 (line 47-94 대체)**

기존: since 이전 세션을 `all_frontmatters`에서 완전히 제외
수정: **모든 세션을 `all_frontmatters`에 수집**하되, `is_new` 플래그에서 since 조건을 반영

```rust
// 2단계 수집: 전체 vault → all_frontmatters (관계 계산용)
// is_new: since + 미처리 조건을 모두 만족하는 세션만 true
let mut all_frontmatters = Vec::new();
let mut is_new: Vec<bool> = Vec::new();
let mut skipped = 0usize;

for entry in &md_files {
    let path = entry.path();
    let content = match std::fs::read_to_string(path) { ... };
    let fm = match parse_session_frontmatter(&content) { ... };

    // is_new 판정: force이거나, (미처리 && since 범위 내)
    let already = already_graphed.contains(&fm.session_id);

    let in_since_range = if let Some(since_date) = since {
        let parent_name = path.parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("");
        !is_date_dir(parent_name) || parent_name >= since_date
    } else {
        true
    };

    let session_is_new = force || (!already && in_since_range);

    if !session_is_new && !force {
        skipped += 1;
    }

    // 관계 계산을 위해 모든 세션을 수집 (since 이전 포함)
    all_frontmatters.push(fm);
    is_new.push(session_is_new);
}
```

### 전체 build_graph 흐름 (수정 후)

```
1. force이면 clear_graph()
2. 전체 vault MD 순회 → all_frontmatters 수집 (since 무관)
3. is_new 계산: force || (!already && in_since_range)
4. 트랜잭션 시작
   4a. delete_relation_edges(["same_project", "same_day"])
   4b. 신규 세션(is_new=true)의 노드/엣지 upsert (belongs_to, by_agent, uses_tool)
   4c. extract_session_relations(all_frontmatters) → 관계 엣지 전체 삽입
5. 트랜잭션 커밋
```

## Dependencies

- Task 1 (DB 스키마) 완료 필수
- graph_repo.rs에 `delete_relation_edges()` 추가 (이 태스크에서 직접 추가)

## Verification

```bash
# 1. 타입 체크
cargo check -p secall-core 2>&1 | tail -3

# 2. 기존 extract 테스트 통과 (변경 없음이므로 반드시 통과)
cargo test -p secall-core -- extract 2>&1 | tail -10

# 3. 기존 build 테스트 통과
cargo test -p secall-core -- test_build_graph_incremental 2>&1 | tail -5

# 4. 기존 cross-session 테스트 통과
cargo test -p secall-core -- test_incremental_build_creates_cross_session_relations 2>&1 | tail -5

# 5. [신규] since 필터 정확성 테스트
cargo test -p secall-core -- test_since_filter_only_matches_date_dirs 2>&1 | tail -5

# 6. [신규] 중간 세션 추가 시 인접 엣지 교체 테스트
cargo test -p secall-core -- test_incremental_replaces_adjacency_edges 2>&1 | tail -5

# 7. [신규] since가 관계 계산 범위를 제한하지 않는지 테스트
cargo test -p secall-core -- test_since_does_not_limit_relation_scope 2>&1 | tail -5

# 8. graph_repo 테스트 (delete_relation_edges 포함)
cargo test -p secall-core -- test_delete_relation_edges 2>&1 | tail -5

# 9. 전체 secall-core 테스트
cargo test -p secall-core 2>&1 | tail -10
```

### 신규 테스트 상세

**test_since_filter_only_matches_date_dirs:**
```
vault 구조:
  raw/sessions/2026-04-09/session001.md  (파일명이 10글자)
  raw/sessions/2026-04-10/session002.md

since = "2026-04-10"으로 빌드.
session001은 날짜 디렉토리 2026-04-09 < since이므로 is_new=false.
기존 코드(결함)에서는 "session001"이 10글자 && >= "2026-04-10"이면 오탐.
수정 후: 부모 디렉토리 "2026-04-09" < "2026-04-10"이므로 정확히 제외.

검증: r.sessions_processed == 1 (session002만)
```

**test_incremental_replaces_adjacency_edges:**
```
1회차: session A(04-09), session C(04-11) — 같은 project
  → same_project 엣지: A→C (1개)

2회차: session B(04-10) 추가 — 같은 project
  → same_project 엣지: A→B, B→C (2개)
  → 기존 A→C는 삭제되어야 함

검증:
  - graph_edges에서 relation='same_project' 조회
  - A→C 엣지가 없음
  - A→B, B→C 엣지가 있음
```

**test_since_does_not_limit_relation_scope:**
```
1회차: session OLD(04-08, proj1) — 빌드 완료

2회차: session NEW(04-10, proj1) — since="2026-04-10"으로 빌드
  → NEW의 노드/엣지만 upsert (is_new=true)
  → BUT 관계 계산은 OLD+NEW 전체 대상
  → same_project 엣지: OLD→NEW 존재

검증:
  - db.get_neighbors("session:OLD") 중 session:NEW가 same_project로 연결
```

**test_delete_relation_edges:**
```
graph_repo.rs에 추가.
same_project 엣지 3개, belongs_to 엣지 2개 삽입.
delete_relation_edges(["same_project"]) 호출.
same_project 0개, belongs_to 2개 확인.
```

## Risks

- **관계 엣지 전체 재계산 성능**: 960 세션 기준 project 그룹핑 + 날짜 그룹핑은 O(n log n). DELETE + INSERT 합쳐도 수천 행 수준이므로 1초 미만. 수만 세션이면 재고려 필요하지만 현재 규모에서는 문제없음
- **DELETE 후 INSERT 사이 crash**: `with_transaction()` 내에서 수행하므로 원자적. crash 시 롤백
- **is_date_dir 오탐**: `2026-99-99` 같은 유효하지 않은 날짜도 통과하지만, 실제 vault 디렉토리명은 ingest에서 생성하므로 항상 유효한 날짜

## Scope boundary — 수정 금지 파일

- `crates/secall-core/src/graph/extract.rs` — 추출 로직 변경 없음
- `crates/secall-core/src/graph/export.rs` — 내보내기 변경 없음
- `crates/secall-core/src/graph/mod.rs` — 모듈 구조 변경 없음
- `crates/secall-core/src/store/schema.rs` — 스키마 변경 없음
- `crates/secall-core/src/store/db.rs` — 마이그레이션 변경 없음
- `crates/secall-core/src/ingest/**` — 파서 수정 금지
- `crates/secall/src/**` — CLI/sync (Task 3, 4 영역)
- `crates/secall-core/src/mcp/**` — MCP (Task 4 영역)

수정 허용: `crates/secall-core/src/store/graph_repo.rs` (delete_relation_edges 1개 메서드 추가만)
