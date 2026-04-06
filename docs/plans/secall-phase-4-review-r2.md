# Review Report: seCall Phase 4 — 검색 고도화 + 인프라 완성 — Round 2

> Verdict: fail
> Reviewer: 
> Date: 2026-04-06 09:32
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall-core/src/search/model_manager.rs:112 — `download_file()`가 `resp.bytes().await`로 `model.onnx` 전체 응답을 메모리에 한 번에 적재한 뒤 파일로 쓰고 있습니다. Task 01 계약은 대용량 모델(~1.2GB) 다운로드를 스트리밍으로 처리해 부분 다운로드/자원 사용 리스크를 낮추는 것이었는데, 현재 구현은 다운로드 시 메모리 급증 또는 OOM을 유발할 수 있는 구체적인 런타임 결함입니다.

## Recommendations

1. Task 03 구현은 localhost 강제라는 핵심 보안 요구는 충족합니다. 다만 현재 endpoint가 `/mcp`이고 Cargo feature도 `transport-streamable-http-server`라서, task 문서의 `/sse` 예시 및 feature 설명과 어긋납니다. 구현이 맞다면 task/result 문서를 현재 동작에 맞게 정리하는 편이 좋습니다.
2. 결과 문서에는 task별 Verification 명령 전체가 보이지 않습니다. 다음 re-review에서는 각 task의 계약 명령과 대응되는 결과를 함께 남기면 검토가 더 명확해집니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | ort 모델 자동 다운로드 | ✅ done |
| 2 | OpenAI 임베딩 API embedder | ✅ done |
| 3 | MCP HTTP transport | ✅ done |
| 4 | LLM 쿼리 확장 | ✅ done |

