---
type: task
status: draft
plan: secall-extensions-nlp
task_number: 3
title: "ort ONNX 로컬 임베딩"
parallel_group: B
depends_on: []
updated_at: 2026-04-06
---

# Task 03: ort ONNX 로컬 임베딩

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/search/embedding.rs` | 수정 | `Embedder` trait 추출 + `OrtEmbedder` 추가 |
| `crates/secall-core/src/search/vector.rs:12,25-31` | 수정 | `OllamaEmbedder` → `Box<dyn Embedder>` |
| `crates/secall-core/src/search/hybrid.rs:66-74` | 수정 | `SearchEngine` 생성 시 embedder trait 사용 |
| `crates/secall-core/src/search/vector.rs:174-183` | 수정 | `create_vector_indexer()` → config 기반 backend 선택 |
| `crates/secall-core/src/vault/config.rs:6-13` | 수정 | `EmbeddingConfig` 추가 |
| `Cargo.toml` | 수정 | `ort`, `tokenizers` 의존성 추가 |
| `crates/secall-core/Cargo.toml` | 수정 | `ort`, `tokenizers` 의존성 추가 |

## Change description

### 1. Embedder trait 추출 (embedding.rs)

기존 `OllamaEmbedder`의 인터페이스를 trait으로 추출:

```rust
#[async_trait::async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
    async fn is_available(&self) -> bool;
    fn dimensions(&self) -> usize;
    fn model_name(&self) -> &str;
}
```

`OllamaEmbedder`에 `impl Embedder` 적용. 기존 API 유지.

### 2. OrtEmbedder 구현 (embedding.rs)

```rust
pub struct OrtEmbedder {
    session: ort::Session,
    tokenizer: tokenizers::Tokenizer,
    dim: usize,
}

impl OrtEmbedder {
    pub fn new(model_dir: &Path) -> Result<Self> {
        // 1. ort::Session::builder()
        //      .with_optimization_level(GraphOptimizationLevel::Level3)?
        //      .commit_from_file(model_dir.join("model.onnx"))?
        // 2. tokenizers::Tokenizer::from_file(model_dir.join("tokenizer.json"))?
        // 3. dim = 시험 embed로 차원 획득 (or 하드코딩 1024 for bge-m3)
    }

    /// 모델 파일 자동 다운로드 (없을 때)
    pub fn ensure_model(model_dir: &Path) -> Result<PathBuf> {
        // ~/.cache/secall/models/bge-m3-onnx/ 에 model.onnx + tokenizer.json 존재 확인
        // 없으면 HuggingFace에서 다운로드 (reqwest)
        // 또는 에러 + 수동 다운로드 안내 메시지
    }
}
```

ort 2.0.0-rc.12 API:
```rust
use ort::{Session, GraphOptimizationLevel};

let session = Session::builder()?
    .with_optimization_level(GraphOptimizationLevel::Level3)?
    .commit_from_file("model.onnx")?;
```

임베딩 추론 파이프라인:
1. `tokenizer.encode(text)` → `input_ids`, `attention_mask`, `token_type_ids`
2. `session.run(inputs)` → last_hidden_state `[1, seq_len, dim]`
3. Mean pooling: attention_mask 가중 평균 → `[dim]`
4. L2 정규화: `vec / ||vec||`

### 3. VectorIndexer 수정 (vector.rs)

```rust
// 기존
pub struct VectorIndexer {
    embedder: OllamaEmbedder,  // 구체 타입
}

// 변경
pub struct VectorIndexer {
    embedder: Box<dyn Embedder>,  // trait object
}

impl VectorIndexer {
    pub fn new(embedder: Box<dyn Embedder>) -> Self { ... }
}
```

`create_vector_indexer()` 함수를 config 기반으로 변경:
```rust
pub async fn create_vector_indexer(config: &Config) -> Option<VectorIndexer> {
    match config.embedding.backend.as_str() {
        "ort" => {
            let model_dir = config.embedding.model_path
                .clone()
                .unwrap_or_else(default_model_path);
            match OrtEmbedder::new(&model_dir) {
                Ok(e) => {
                    eprintln!("✓ ort ONNX loaded. Local vector search enabled.");
                    Some(VectorIndexer::new(Box::new(e)))
                }
                Err(e) => {
                    eprintln!("⚠ ort load failed: {e}. Trying Ollama fallback...");
                    try_ollama_fallback().await
                }
            }
        }
        "ollama" | _ => {
            // 기존 Ollama 로직
            let embedder = OllamaEmbedder::new(None, None);
            if embedder.is_available().await {
                eprintln!("✓ Ollama available. Vector search enabled.");
                Some(VectorIndexer::new(Box::new(embedder)))
            } else {
                eprintln!("⚠ Ollama not available. BM25-only mode.");
                None
            }
        }
    }
}
```

### 4. Config 확장 (config.rs)

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub vault: VaultConfig,
    pub ingest: IngestConfig,
    pub search: SearchConfig,
    pub hooks: HooksConfig,
    pub embedding: EmbeddingConfig,  // 신규
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct EmbeddingConfig {
    /// "ollama" | "ort"
    pub backend: String,
    /// Ollama base URL (ollama backend)
    pub ollama_url: Option<String>,
    /// Ollama model name (ollama backend)
    pub ollama_model: Option<String>,
    /// ONNX model directory (ort backend)
    pub model_path: Option<PathBuf>,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        EmbeddingConfig {
            backend: "ollama".to_string(),  // 기존 동작 유지
            ollama_url: None,
            ollama_model: None,
            model_path: None,
        }
    }
}
```

config.toml 예시:
```toml
[embedding]
backend = "ort"
model_path = "~/.cache/secall/models/bge-m3-onnx"
```

### 5. Cargo.toml 의존성

```toml
[workspace.dependencies]
# 추가
ort = "2.0.0-rc.12"
tokenizers = "0.21"
async-trait = "0.1"
```

`async-trait`은 `Embedder` trait의 async 메서드를 위해 필요.

## Dependencies

- 없음 (Task 04와 병렬 실행 가능)
- MVP Task 07 (벡터 인덱서) 코드가 존재해야 함

## Verification

```bash
# 타입 체크
cargo check -p secall-core

# Embedder trait + OrtEmbedder 단위 테스트
cargo test -p secall-core embedding

# VectorIndexer가 trait object로 동작하는지 확인
cargo test -p secall-core vector

# 기존 테스트 회귀 없음
cargo test -p secall-core

# ort 모델 로드 통합 테스트 (모델 파일 필요)
# Manual: ~/.cache/secall/models/bge-m3-onnx/에 model.onnx + tokenizer.json 배치 후
# cargo test -p secall-core ort_embed -- --ignored
```

테스트 작성 요구사항:
- `test_embedder_trait_ollama`: OllamaEmbedder가 Embedder trait 구현 확인 (컴파일 타임)
- `test_embedder_trait_ort`: OrtEmbedder가 Embedder trait 구현 확인 (컴파일 타임)
- `test_vector_indexer_with_trait_object`: Box<dyn Embedder>로 VectorIndexer 생성
- `#[ignore] test_ort_embed_basic`: 실제 ONNX 모델로 임베딩 + L2 norm 검증

## Risks

- **ort 2.0.0-rc.12 RC 상태**: API 변경 가능. `ort` 릴리스 노트 모니터링 필요
- **macOS arm64 ONNX Runtime**: `ort`의 기본 빌드가 arm64를 지원하는지 확인. `ort` features에 `coreml` 추가 시 Apple Silicon 가속 가능하나 RC에서 안정성 미확인
- **모델 파일 크기**: bge-m3 ONNX ≈ 1.2GB. 자동 다운로드 시 네트워크 + 디스크 고려
- **async_trait 오버헤드**: vtable dispatch + heap allocation. 임베딩 호출 빈도 대비 무시할 수준이지만, 배치 처리 시 batch 단위 호출로 최소화
- **기존 Ollama 사용자 영향**: default backend = "ollama" 유지하므로 breaking change 없음

## Scope Boundary

수정 금지 파일:
- `search/bm25.rs` — BM25 인덱서는 이 태스크에서 변경하지 않음
- `search/tokenizer.rs` — Task 04 영역
- `ingest/*` — 파서는 이 태스크에서 변경하지 않음
- `mcp/*` — MCP 서버는 이 태스크에서 변경하지 않음
