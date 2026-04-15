---
type: task
status: in_progress
updated_at: 2026-04-15
plan: p26-gemini-api-log
task_number: 03
title: Log 일기 Gemini 백엔드
parallel_group: B
depends_on: [01]
---

# Task 03 — Log 일기 Gemini 백엔드 (`commands/log.rs`)

## Changed files

- `crates/secall/src/commands/log.rs` (수정)
  - 백엔드 분기: L123-140
  - `call_gemini()` 신규 함수 추가 (기존 `call_ollama()` L155-187 참고)

## Change description

### 현재 상태 (`log.rs`, L123-140)

```rust
if config.graph.semantic_backend == "ollama" {
    let base_url = config.graph.ollama_url
        .clone()
        .unwrap_or_else(|| "http://localhost:11434".to_string());
    let model = config.graph.ollama_model
        .clone()
        .unwrap_or_else(|| "gemma4:e4b".to_string());
    call_ollama(&prompt, &base_url, &model).await?
} else {
    // anthropic 또는 기타 — 현재 비어있거나 fallback
    prompt.clone()
}
```

> 현재 `anthropic` 분기가 존재하는지 확인 후 구현. 없으면 `ollama` / `gemini` / else(raw) 3-way 분기로 구성.

### 추가할 내용

**1. `call_gemini()` 함수 추가** (`call_ollama()` 이후 위치)

```rust
async fn call_gemini(prompt: &str, api_key: &str, model: &str) -> Result<String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, api_key
    );

    let payload = serde_json::json!({
        "contents": [{
            "role": "user",
            "parts": [{"text": prompt}]
        }],
        "generationConfig": {
            "temperature": 0.3,
            "maxOutputTokens": 1024
        }
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let resp = client
        .post(&url)
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("gemini api error {}: {}", status, text));
    }

    // 응답 구조: candidates[0].content.parts[0].text
    let data: serde_json::Value = resp.json().await?;
    let text = data["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(text)
}
```

> 응답 구조체를 별도 정의하지 않고 `serde_json::Value`로 처리 (log.rs는 단일 텍스트 응답이므로 단순화).

**2. 백엔드 분기 확장** (L123-140 대체)

```rust
let diary = if config.graph.semantic_backend == "ollama" {
    let base_url = config.graph.ollama_url
        .clone()
        .unwrap_or_else(|| "http://localhost:11434".to_string());
    let model = config.graph.ollama_model
        .clone()
        .unwrap_or_else(|| "gemma4:e4b".to_string());
    call_ollama(&prompt, &base_url, &model).await?
} else if config.graph.semantic_backend == "gemini" {
    let api_key = config.graph.gemini_api_key
        .clone()
        .or_else(|| std::env::var("SECALL_GEMINI_API_KEY").ok())
        .ok_or_else(|| anyhow::anyhow!("gemini api key not set"))?;
    let model = config.graph.gemini_model
        .as_deref()
        .unwrap_or("gemini-2.5-flash")
        .to_string();
    call_gemini(&prompt, &api_key, &model).await?
} else {
    // anthropic 또는 disabled — 현재와 동일한 fallback
    prompt.clone()
};
```

### 주의사항

- `temperature: 0.3` — 현재 Ollama와 동일한 값 사용
- `maxOutputTokens: 1024` — 일기 생성은 semantic 추출보다 출력이 길 수 있으므로 512→1024로 설정
- 실제 코드에서 `call_ollama()` 반환값이 변수에 바인딩되는 방식 확인 후 일치시킬 것

## Dependencies

- Task 01 완료 후 진행 (GraphConfig에 `gemini_api_key`, `gemini_model` 필드 존재해야 함)
- Task 02와 병렬 실행 가능 (같은 parallel_group B)

## Verification

```bash
cd /Users/d9ng/privateProject/seCall
cargo check -p secall 2>&1 | tail -10
```

컴파일 에러 없으면 통과.

## Risks

- `log.rs`에서 현재 Ollama가 아닌 경우의 fallback 동작을 정확히 파악한 후 분기 구성 필요. 현재 상태에 따라 else 분기 처리가 달라질 수 있음.
- `anyhow::anyhow!` import: log.rs에서 이미 `anyhow` 사용 중인지 확인. 사용 중이면 추가 import 불필요.

## Scope boundary

수정 금지 파일:
- `vault/config.rs` (Task 01 영역)
- `graph/semantic.rs` (Task 02 영역)
- `commands/` 내 다른 파일들 (`ingest.rs`, `serve.rs` 등)
