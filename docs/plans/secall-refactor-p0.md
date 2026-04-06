---
type: plan
status: draft
version: "1.0"
updated_at: 2026-04-06
slug: secall-refactor-p0
title: "seCall Refactor P0 — 검색 정확성 결함 수정"
---

# seCall Refactor P0 — 검색 정확성 결함 수정

## Description

검색 결과의 신뢰성을 직접 훼손하는 3개 결함을 수정한다:
1. BM25가 DB rowid를 turn_index로 반환 → 검색 결과 컨텍스트 위치 오류
2. vault_path에 절대경로를 저장 → `secall get --full` 경로 해석 불안정
3. Lint L002가 파일명 stem을 session_id로 조회 → 모든 vault 파일이 orphan으로 오탐

## Expected Outcome

- `secall recall` 검색 결과의 `turn_index`가 실제 세션 내 턴 순서와 일치
- `secall get --full`이 vault 파일을 정확히 읽어옴
- `secall lint`의 L002가 false positive 없이 실제 orphan만 보고

## Architecture

```
[BM25 검색 흐름 — Task 01]
insert_turn() → rowid ← last_insert_rowid()
insert_fts(turn_id=rowid) ← ❌ rowid 저장
search_fts() → turn_id(=rowid) → turn_index 캐스팅 ← ❌ 잘못된 값

수정 후:
insert_fts(turn_index=turn.index) ← ✅ 실제 turn_index 저장
search_fts() → turn_index → SearchResult.turn_index ← ✅ 정확한 값

[vault_path 흐름 — Task 02]
write_session() → abs_path(절대) → DB 저장 ← ❌ 절대경로
get --full → config.vault.path.join(abs_path) ← ❌ 이중 경로

수정 후:
write_session() → rel_path(상대) → DB 저장 ← ✅ 상대경로
get --full → config.vault.path.join(rel_path) ← ✅ 정상 합성

[Lint L002 흐름 — Task 03]
vault file: "claude-code_seCall_a1b2c3d4.md"
file_stem → "claude-code_seCall_a1b2c3d4" → db.session_exists() ← ❌ UUID 아님

수정 후:
vault file → frontmatter 파싱 → session_id 추출 → db.session_exists() ← ✅ 정확
```

## Subtasks

1. **BM25 turn_index 수정** — FTS INSERT/SELECT에서 rowid 대신 turn_index 사용
   - parallel_group: A
   - depends_on: —

2. **vault_path 상대경로 전환** — write_session()이 상대경로 반환, DB 마이그레이션
   - parallel_group: A
   - depends_on: —

3. **Lint L002 session_id 추출 수정** — frontmatter 기반 session_id 조회로 전환
   - parallel_group: A
   - depends_on: —

## Dependency Graph

```
Task 01 (BM25 turn_index)
Task 02 (vault_path)       ← 모두 독립, 병렬 실행 가능
Task 03 (Lint L002)
```

## Constraints

- 기존 FTS5 인덱스 데이터는 `secall reindex`로 재생성 가능 (자동 마이그레이션 불필요)
- vault 디렉토리 구조는 변경하지 않음
- DB 스키마(테이블 구조)는 변경하지 않음 — FTS5 REBUILD + 데이터 마이그레이션만

## Non-goals

- FTS5 스키마 자체 재설계 (content-sync 등)
- vault 파일명 규칙 변경
- 다른 lint 룰(L001, L003~L010) 수정
- 벡터 인덱스 관련 수정

## Risks

- **FTS 재생성 필요**: Task 01 적용 후 기존 FTS 데이터에 rowid가 저장되어 있으므로 `secall reindex` 실행 필요. 사용자 안내 문구 추가.
- **기존 DB 절대경로**: Task 02 적용 후 기존 DB에 절대경로가 남아있으므로 마이그레이션 함수 필요.
