---
type: plan
status: draft
version: "1.0"
updated_at: 2026-04-06
slug: secall-refactor-p1
title: "seCall Refactor P1 — 에러 처리 + 데이터 정합성"
---

# seCall Refactor P1 — 에러 처리 + 데이터 정합성

## Description

에러를 무시하는 코드 패턴을 수정하여 데이터 정합성을 보장한다:
1. `ingest.rs`의 9개 에러 삼킴 패턴 → 적절한 에러 전파 + 사용자 경고
2. `db.rs`의 35+ `unwrap_or` 패턴 → Result 반환 전환 (읽기: graceful, 쓰기: propagate)
3. ingest 파이프라인에 트랜잭션 도입 → vault+DB+경로 갱신의 원자성 보장
4. Codex/Gemini 파서의 타임스탬프 복원

## Expected Outcome

- DB 쓰기 실패 시 사용자에게 명시적 경고 출력
- `db.rs`의 쓰기 메서드가 `Result`를 반환하여 호출자가 에러 처리 결정
- ingest 중 부분 실패 시 일관된 롤백 (orphan vault 파일/DB 레코드 방지)
- Gemini 세션의 실제 생성 시간이 검색 필터에 반영

## Architecture

```
[에러 흐름 — 현재]
ingest.rs:55  session_exists() → unwrap_or(false) → 중복체크 실패 무시
ingest.rs:79  index_session() → unwrap_or_default() → 인덱싱 실패 무시
ingest.rs:83  update_vault_path() → let _ = → 경로 저장 실패 무시
db.rs:82      count query → unwrap_or(0) → 통계 오류 무시

[에러 흐름 — 수정 후]
ingest.rs:55  session_exists() → ? → 실패 시 전체 세션 skip + 경고
ingest.rs:79  index_session() → ? → 실패 시 vault cleanup + 경고
ingest.rs:83  update_vault_path() → ? → 실패 시 경고
db.rs:82      count query → Result<i64> → 호출자가 결정

[타임스탬프 흐름 — 현재]
Gemini: gs.create_time = Some("2026-04-05T10:00:00Z") → 무시 → Utc::now()
Codex:  JSON에 timestamp 필드 없음 → Utc::now()

[타임스탬프 흐름 — 수정 후]
Gemini: gs.create_time → parse_rfc3339() → start_time (fallback: Utc::now())
Codex:  file metadata → modified() → start_time (fallback: Utc::now())
```

## Subtasks

1. **ingest.rs 에러 전파** — 에러 삼킴 패턴을 적절한 전파/경고로 변환
   - parallel_group: A
   - depends_on: —

2. **db.rs Result 반환 전환** — lint 헬퍼 메서드의 반환 타입을 Result로 변경
   - parallel_group: A
   - depends_on: —

3. **ingest 트랜잭션 래핑** — DB 쓰기를 트랜잭션으로 감싸기
   - parallel_group: B
   - depends_on: [Task 01, Task 02]

4. **Codex/Gemini 타임스탬프 복원** — 실제 시간 정보 추출
   - parallel_group: A
   - depends_on: —

## Dependency Graph

```
Task 01 (ingest error)  ─┐
Task 02 (db.rs Result)  ─┤─→ Task 03 (transaction)
Task 04 (timestamp)      │   (depends_on: 01, 02)
                          │
        [Group A 병렬]    [Group B]
```

## Constraints

- `db.rs` 변환은 한 메서드씩 점진적으로 진행 (한번에 전체 변경 금지)
- 기존 CLI 동작(exit code, 출력 형식)은 유지
- 타임스탬프 변경은 새로 ingest하는 세션에만 적용 (기존 DB 데이터 마이그레이션 불필요)

## Non-goals

- `db.rs`를 ORM으로 전환
- 커스텀 에러 타입 계층 설계 (`anyhow::Error` 유지)
- 로깅 프레임워크 도입 (Refactor P2에서 처리)
- `eprintln!` → `tracing` 전환 (Refactor P2에서 처리)

## Risks

- **db.rs 호출자 범위**: `count_sessions()`, `agent_counts()` 등의 반환 타입 변경 시 `lint.rs`, `status.rs` 등 호출자도 업데이트 필요. 변경 범위 넓음.
- **트랜잭션과 vault 파일**: DB 트랜잭션은 파일시스템 쓰기를 커버하지 못함. vault 파일 쓰기 후 DB 실패 시 파일 cleanup 로직 필요.
- **Codex 타임스탬프 한계**: JSON 구조에 타임스탬프 필드가 없어 file mtime만 대안. mtime이 변경된 경우 부정확할 수 있음.
