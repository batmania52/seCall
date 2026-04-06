# Review Report: seCall Phase 4 — 검색 고도화 + 인프라 완성 — Round 1

> Verdict: fail
> Reviewer: 
> Date: 2026-04-06 09:26
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall-core/src/search/vector.rs:188 — Task 01의 자동 다운로드 분기가 `model.onnx` 존재만 확인하고 `tokenizer.json` 누락은 검사하지 않습니다. 부분 다운로드나 손상된 캐시로 `model.onnx`만 남은 경우 자동 복구를 건너뛰고 `OrtEmbedder::new()`로 진행한 뒤 실패 시 Ollama fallback으로 빠져, task 문서의 "모델 미존재 시 자동 다운로드" 보장을 깨뜨립니다.
2. crates/secall-core/src/mcp/server.rs:266 — Task 03은 localhost 전용 HTTP transport가 계약인데, 현재 구현은 사용자가 입력한 임의의 bind 주소를 그대로 허용합니다. `0.0.0.0:8080` 같은 주소로도 바인드 가능해 인증 없는 MCP 서버가 외부에 노출될 수 있으므로, task의 보안 제약을 위반하는 구체적인 보안 결함입니다.

## Recommendations

1. Task 01은 `create_vector_indexer()`에서 직접 `model.onnx`만 보지 말고 `ModelManager::is_downloaded()`를 사용해 두 파일을 함께 검증하는 편이 맞습니다.
2. Task 03은 `127.0.0.1`, `::1`, `localhost`만 허용하도록 bind 주소를 검증하거나, 비-loopback 주소는 명시적으로 거부하세요.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | ort 모델 자동 다운로드 | ✅ done |
| 2 | OpenAI 임베딩 API embedder | ✅ done |
| 3 | MCP HTTP transport | ✅ done |
| 4 | LLM 쿼리 확장 | ✅ done |

