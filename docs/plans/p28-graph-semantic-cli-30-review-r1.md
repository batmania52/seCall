# Review Report: P28 — Graph Semantic CLI 플래그 확장 (#30) — Round 1

> Verdict: fail
> Reviewer: 
> Date: 2026-04-16 12:49
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall-core/src/vault/config.rs:409 — `test_graph_env_override_backend`가 process-global 환경변수 `SECALL_GRAPH_BACKEND`를 직접 변경하지만 직렬화 장치가 없습니다. 같은 파일의 다른 env 테스트와 병렬 실행되면 값이 섞여 간헐적으로 실패할 수 있습니다.
2. crates/secall-core/src/vault/config.rs:417 — `test_graph_env_override_api_url`가 process-global 환경변수를 직접 변경하며 동기화가 없습니다. `cargo test` 기본 병렬 실행에서 다른 env 테스트와 충돌할 수 있습니다.
3. crates/secall-core/src/vault/config.rs:425 — `test_graph_env_override_model_gemini`가 `SECALL_GRAPH_BACKEND`와 `SECALL_GRAPH_MODEL`을 동시에 변경하지만 보호 장치가 없습니다. 병렬 실행 시 다른 테스트와 상호 간섭해 잘못된 backend/model 조합을 읽을 수 있습니다.
4. crates/secall-core/src/vault/config.rs:435 — `test_graph_env_override_api_key`도 동일하게 process-global 환경변수를 직접 변경하며 직렬화가 없습니다. 테스트 안정성이 보장되지 않습니다.

## Recommendations

1. `config.rs`의 env 기반 테스트들을 전역 mutex로 감싸거나, 직렬 실행 매크로/헬퍼를 도입해 테스트 간 환경변수 접근을 serialize 하세요.
2. 가능하면 환경변수 직접 접근 로직을 별도 helper로 분리해 입력 주입 방식으로 테스트하면 병렬성 문제를 피할 수 있습니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | CLI 플래그 추가 (`main.rs`) | ✅ done |
| 2 | GraphConfig 오버라이드 로직 (`main.rs` + `commands/graph.rs`) | ✅ done |
| 3 | 환경변수 fallback (`config.rs`) | ✅ done |
| 4 | 테스트 및 문서 | ✅ done |

