---
type: plan
status: draft
version: "1.0"
updated_at: 2026-04-06
slug: secall-refactor-p2
title: "seCall Refactor P2 — 인프라 + 성능"
---

# seCall Refactor P2 — 인프라 + 성능

## Description

운영 안정성과 성능을 개선하는 인프라 작업:
1. 구조적 로깅 (tracing 크레이트) — 12개 파일, 32개 `eprintln!` 교체
2. 벡터 검색 메모리 최적화 — 전체 테이블 로드 제거
3. 디렉토리 ingest 멀티에이전트 지원 — Claude 전용 → 3개 에이전트
4. BLOB 차원 검증 + CLI/MCP 테스트 추가

## Expected Outcome

- `RUST_LOG=debug secall recall "query"` → stderr에 구조적 로그 출력 (MCP stdout 무오염)
- 100k 벡터에서도 OOM 없이 검색 가능 (현재: 전체 로드 ~400MB → 최적화 후: 세션 필터링)
- `secall ingest /path/` → Claude/Codex/Gemini 세션 모두 탐색
- CLI 커맨드와 MCP 서버의 기본 테스트 존재

## Architecture

```
[로깅 흐름 — Task 01]
현재: eprintln!("warn: ...") → stderr (비구조적, 레벨 없음)
수정: tracing::warn!("...") → tracing-subscriber → stderr (구조적, 필터 가능)
MCP: subscriber.with(fmt::layer().with_writer(std::io::stderr))

[벡터 검색 흐름 — Task 02]
현재: SELECT * FROM turn_vectors → 전체 로드 → in-memory cosine → truncate
수정: BM25 결과의 session_id → SELECT WHERE session_id IN (...) → 후보만 로드

[디렉토리 ingest — Task 03]
현재: ingest /path → find_claude_sessions() 만 호출
수정: ingest /path → find_claude + find_codex + find_gemini 호출

[테스트 — Task 04]
현재: crates/secall/ → 0 tests, mcp/ → 0 tests
수정: CLI smoke test + MCP tool 단위 테스트 + BLOB 차원 검증
```

## Subtasks

1. **tracing 도입** — `eprintln!` → 구조적 로깅 전환
   - parallel_group: A
   - depends_on: —

2. **벡터 검색 메모리 최적화** — session_id 필터 WHERE절 추가
   - parallel_group: A
   - depends_on: —

3. **디렉토리 ingest 멀티에이전트** — 3줄 변경
   - parallel_group: A
   - depends_on: —

4. **BLOB 검증 + 테스트** — 차원 체크 + CLI/MCP 테스트
   - parallel_group: A
   - depends_on: —

## Dependency Graph

```
Task 01 (tracing)
Task 02 (vector memory)     ← 모두 독립, 병렬 실행 가능
Task 03 (directory ingest)
Task 04 (BLOB + tests)
```

## Constraints

- tracing은 stderr 전용 (stdout은 MCP 프로토콜 전용)
- sqlite-vec는 이 플랜에서 도입하지 않음 (WHERE 필터 최적화만)
- CI 파이프라인 구축은 별도 작업 (이 플랜은 테스트 코드만 추가)

## Non-goals

- HNSW/sqlite-vec 인덱스 전환
- GitHub Actions CI 구축
- 성능 벤치마크 스위트
- 전체 테스트 커버리지 목표 설정
- log4rs, slog 등 다른 로깅 프레임워크 검토

## Risks

- **tracing + MCP stdout 충돌**: tracing의 기본 출력은 stdout. subscriber를 stderr-only로 설정해야 MCP 프로토콜 무오염.
- **벡터 WHERE 필터와 검색 품질**: session_id 필터 적용 시 cross-session 벡터 검색 품질이 하락할 수 있음. BM25 결과 기반 후보 세션 추출 전략 필요.
- **CLI 통합 테스트 환경**: 실제 DB가 필요. tempdir + in-memory DB 활용.
