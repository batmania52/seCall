---
type: task
status: draft
plan: secall-phase-4
task_number: 2
title: "OpenAI 임베딩 API embedder"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 02: OpenAI 임베딩 API embedder

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/search/embedding.rs` | 수정 | `OpenAIEmbedder` 추가 |
| `crates/secall-core/src/vault/config.rs` | 수정 | `EmbeddingConfig`에 openai 필드 추가 |
| `crates/secall-core/src/search/vector.rs:177-202` | 수정 | `create_vector_indexer()`에 openai backend 분기 추가 |

## Change description

### 1. OpenAIEmbedder 구현 (embedding.rs)

```rust
pub struct OpenAIEmbedder {
    client: Client,
    api_key: String,
    model: String,
    dim: usize,
}

impl OpenAIEmbedder {
    pub fn new(api_key: &str, model: Option<&str>) -> Self {
        let model = model.unwrap_or("text-embedding-3-large").to_string();
        let dim = match model.as_str() {
            "text-embedding-3-large" => 3072,
            "text-embedding-3-small" => 1536,
            _ => 3072,
        };
        OpenAIEmbedder {
            client: Client::new(),
            api_key: api_key.to_string(),
            model,
            dim,
        }
    }
}

#[derive(Serialize)]
struct OpenAIEmbedRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct OpenAIEmbedResponse {
    data: Vec<OpenAIEmbedData>,
}

#[derive(Deserialize)]
struct OpenAIEmbedData {
    embedding: Vec<f32>,
}
```

### 2. Embedder trait 구현

```rust
#[async_trait]
impl Embedder for OpenAIEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let mut batch = self.embed_batch(&[text]).await?;
        batch.pop().ok_or_else(|| anyhow!("empty response"))
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let req = OpenAIEmbedRequest {
            model: self.model.clone(),
            input: texts.iter().map(|s| s.to_string()).collect(),
        };

        let resp = self.client
            .post("https://api.openai.com/v1/embeddings")
            .bearer_auth(&self.api_key)
            .json(&req)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("OpenAI embed failed ({status}): {body}"));
        }

        let embed_resp: OpenAIEmbedResponse = resp.json().await?;
        Ok(embed_resp.data.into_iter().map(|d| d.embedding).collect())
    }

    async fn is_available(&self) -> bool {
        // API 키가 있으면 available로 간주
        // 실제 호출은 embed 시 검증
        !self.api_key.is_empty()
    }

    fn dimensions(&self) -> usize { self.dim }
    fn model_name(&self) -> &str { &self.model }
}
```

### 3. Config 확장 (config.rs)

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct EmbeddingConfig {
    /// "ollama" | "ort" | "openai"
    pub backend: String,
    pub ollama_url: Option<String>,
    pub ollama_model: Option<String>,
    pub model_path: Option<PathBuf>,
    /// OpenAI model name (openai backend)
    pub openai_model: Option<String>,
}
```

API 키는 config에 넣지 않음 — 환경변수 `OPENAI_API_KEY`에서만 읽음 (보안).

config.toml 예시:
```toml
[embedding]
backend = "openai"
openai_model = "text-embedding-3-large"  # or "text-embedding-3-small"
```

### 4. create_vector_indexer() 확장 (vector.rs)

```rust
"openai" => {
    let api_key = std::env::var("OPENAI_API_KEY").ok();
    match api_key {
        Some(key) if !key.is_empty() => {
            let model = config.embedding.openai_model.as_deref();
            let embedder = OpenAIEmbedder::new(&key, model);
            eprintln!("✓ OpenAI embedder ready ({}).", embedder.model_name());
            Some(VectorIndexer::new(Box::new(embedder)))
        }
        _ => {
            eprintln!("⚠ OPENAI_API_KEY not set. Trying Ollama fallback...");
            try_ollama_fallback(config).await
        }
    }
}
```

## Dependencies

- 없음 (다른 task와 독립)
- Phase 2의 `Embedder` trait 존재 전제 (`embedding.rs:11-19`)

## Verification

```bash
# 타입 체크
cargo check -p secall-core

# Embedder trait 컴파일 검증
cargo test -p secall-core embedding

# Config 역직렬화 테스트
cargo test -p secall-core config

# 전체 테스트 회귀
cargo test

# 실제 API 호출 (수동, API 키 필요)
# Manual: OPENAI_API_KEY=sk-... cargo run -p secall -- recall "test query" --vec
```

테스트 작성 요구사항:
- `test_embedder_trait_openai`: `OpenAIEmbedder`가 `Embedder` trait 구현 확인 (컴파일 타임)
- `test_openai_model_names`: `text-embedding-3-large` → dim 3072, `text-embedding-3-small` → dim 1536
- `test_openai_missing_key_not_available`: API 키 빈 문자열 → `is_available() == false`
- `#[ignore] test_openai_embed_real`: 실제 API 호출 (API 키 필요)

## Risks

- **API 비용**: text-embedding-3-large $0.13/1M tokens. 대량 ingest 시 비용 발생. batch_size 조절로 완화
- **Rate limit**: OpenAI API rate limit 초과 시 재시도 로직 필요. 초기 구현은 단순 에러 반환, 향후 exponential backoff 추가 가능
- **API 키 노출**: config.toml에 키를 넣지 않도록 환경변수만 지원. 실수 방지
- **벡터 차원 불일치**: OllamaEmbedder(1024) → OpenAIEmbedder(3072)로 전환 시 기존 벡터와 차원 불일치. `secall embed --all`로 재임베딩 필요. 혼합 검색 불가 주의

## Scope Boundary

수정 금지 파일:
- `search/tokenizer.rs` — 토크나이저 변경 금지
- `ingest/*` — 파서 변경 금지
- `mcp/*` — MCP 서버 변경 금지
