---
type: task
status: in_progress
updated_at: 2026-04-15
plan: p26-gemini-api-log
task_number: 02
title: 시맨틱 그래프 Gemini 백엔드
parallel_group: B
depends_on: [01]
---

# Task 02 — 시맨틱 그래프 Gemini 백엔드 (`graph/semantic.rs`)

## Changed files

- `crates/secall-core/src/graph/semantic.rs` (수정)
  - 상수/응답 구조체: 파일 상단 (~L1-90)
  - `extract_with_gemini()` 신규 함수 추가
  - `extract_with_llm()` 디스패치 분기 확장: L238-261

## Change description

### 현재 디스패치 구조 (`extract_with_llm`, L238-261)

```rust
match cfg.semantic_backend.as_str() {
    "ollama"     => extract_with_ollama(body, cfg).await,
    "anthropic"  => extract_with_anthropic(body, cfg).await,
    _            => Err(anyhow!("unknown backend: {}", cfg.semantic_backend)),
}
```

### 추가할 내용

**1. 응답 구조체 추가** (파일 상단, 기존 `AnthropicResponse` 이후)

```rust
// Gemini API 응답 구조
#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
}
#[derive(Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
}
#[derive(Deserialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}
#[derive(Deserialize)]
struct GeminiPart {
    text: String,
}
```

**2. `extract_with_gemini()` 함수 추가**

```rust
async fn extract_with_gemini(body: &str, cfg: &GraphConfig) -> Result<Vec<SemanticEdge>> {
    // API 키 우선순위: config > 환경변수
    let api_key = cfg.gemini_api_key
        .clone()
        .or_else(|| std::env::var("SECALL_GEMINI_API_KEY").ok())
        .ok_or_else(|| anyhow!("gemini api key not set (config.graph.gemini_api_key or SECALL_GEMINI_API_KEY)"))?;

    let model = cfg.gemini_model.as_deref().unwrap_or("gemini-2.5-flash");
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, api_key
    );

    // 기존 Anthropic 프롬프트와 동일한 system + user 메시지 구성
    let payload = serde_json::json!({
        "contents": [{
            "role": "user",
            "parts": [{"text": format!("{}\n\n{}", SYSTEM_PROMPT, body)}]
        }],
        "generationConfig": {
            "temperature": 0.1,
            "maxOutputTokens": 512
        }
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("gemini api error {}: {}", status, text));
    }

    let data: GeminiResponse = resp.json().await?;
    let text = data.candidates
        .into_iter()
        .next()
        .and_then(|c| c.content.parts.into_iter().next())
        .map(|p| p.text)
        .unwrap_or_default();

    parse_llm_edges(&text)  // 기존 파싱 함수 재사용
}
```

**3. `extract_with_llm()` 디스패치 확장**

```rust
match cfg.semantic_backend.as_str() {
    "ollama"     => extract_with_ollama(body, cfg).await,
    "anthropic"  => extract_with_anthropic(body, cfg).await,
    "gemini"     => extract_with_gemini(body, cfg).await,   // 추가
    _            => Err(anyhow!("unknown backend: {}", cfg.semantic_backend)),
}
```

### 주의사항

- Gemini는 `system` role을 별도로 지원하지 않는 경우가 있으므로, system 프롬프트를 user 메시지 앞에 붙여서 단일 contents로 전송한다.
- `SYSTEM_PROMPT` 상수가 현재 파일에 정의되어 있는지 확인 후 재사용. 없으면 Anthropic 쪽 system 파라미터와 동일 텍스트를 상수로 추출한다.
- `parse_llm_edges()` 함수가 이미 존재한다면 재사용. 없으면 Ollama/Anthropic 응답 파싱 로직을 공통 함수로 추출한다.

## Dependencies

- Task 01 완료 후 진행 (GraphConfig에 `gemini_api_key`, `gemini_model` 필드 존재해야 함)

## Verification

```bash
cd /Users/d9ng/privateProject/seCall
cargo check -p secall-core 2>&1 | tail -10
```

컴파일 에러 없으면 통과.

실제 API 호출 테스트 (선택적, API 키 있을 때):
```bash
# Manual: SECALL_GEMINI_API_KEY=<key> secall ingest --backend gemini <session_file>
# graph/semantic 추출 로그에서 "gemini" 백엔드 확인
```

## Risks

- Gemini `contents` 구조는 `system_instruction` 별도 지원 (v1beta). 필요 시 `systemInstruction` 필드 사용 가능하나 단순화를 위해 user message에 합치는 방식 우선 사용.
- API 응답에 `candidates`가 비어있을 경우 빈 edges 반환 — 기존 Ollama와 동일한 fallback 동작이므로 허용.
- 429(rate limit) 에러 처리: 현재 Ollama/Anthropic도 재시도 없음. 동일하게 에러 전파로 처리.

## Scope boundary

수정 금지 파일:
- `vault/config.rs` (Task 01 영역)
- `commands/log.rs` (Task 03 영역)
- `graph/extract.rs` (규칙 기반 추출 — 변경 없음)
- `graph/build.rs` (빌드 파이프라인 — 변경 없음)
