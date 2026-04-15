---
type: task
status: in_progress
updated_at: 2026-04-15
plan: p26-gemini-api-log
task_number: 04
title: 통합 테스트 및 검증
parallel_group: C
depends_on: [01, 02, 03]
---

# Task 04 — 통합 테스트 및 검증

## Changed files

없음 (신규 파일 생성 없음, 기존 테스트 실행만)

## Change description

Task 01-03 구현 완료 후 전체 빌드 및 기존 테스트 통과를 확인한다.

추가로 `.env` 파일에 이미 `GEMINI_API_KEY` 또는 `SECALL_GEMINI_API_KEY`가 있으면 수동 smoke test를 진행한다.

## Dependencies

- Task 01, 02, 03 모두 완료 후 진행

## Verification

### 1. 전체 빌드 확인

```bash
cd /Users/d9ng/privateProject/seCall
cargo build 2>&1 | tail -10
```

`Finished` 출력 확인.

### 2. 기존 단위 테스트 통과

```bash
cd /Users/d9ng/privateProject/seCall
cargo test -p secall-core 2>&1 | tail -20
```

기존 테스트 모두 통과 (실패 0).

### 3. Config 직렬화/역직렬화 확인

```bash
cd /Users/d9ng/privateProject/seCall
cargo test -p secall-core config 2>&1
```

config 관련 테스트가 있으면 통과 확인. 없으면 skip.

### 4. (선택) Gemini API smoke test

API 키가 있을 때만 실행:

```bash
# Manual: config.toml에 아래 설정 추가 후
# [graph]
# semantic_backend = "gemini"
# gemini_model = "gemini-2.5-flash"
#
# SECALL_GEMINI_API_KEY=<key> cargo run --bin secall -- log 2026-04-15
# 일기 출력 확인
```

## Risks

- 기존 테스트가 GraphConfig 기본값을 직접 비교하는 경우 필드 추가로 실패 가능 → 테스트 코드 확인 필요
- API 키 미설정 상태에서 `gemini` 백엔드 선택 시 명확한 에러 메시지 출력 여부 확인

## Scope boundary

수정 금지 파일: 없음 (테스트 실행 전용 task)
