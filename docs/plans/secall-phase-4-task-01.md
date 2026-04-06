---
type: task
status: draft
plan: secall-phase-4
task_number: 1
title: "ort 모델 자동 다운로드"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 01: ort 모델 자동 다운로드

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/search/model_manager.rs` | **신규 생성** | 모델 다운로드 + 버전 관리 |
| `crates/secall-core/src/search/mod.rs` | 수정 | `pub mod model_manager;` 추가 |
| `crates/secall-core/src/search/vector.rs:177-202` | 수정 | 모델 미존재 시 자동 다운로드 시도 |
| `crates/secall/src/commands/model.rs` | **신규 생성** | CLI 커맨드 핸들러 |
| `crates/secall/src/commands/mod.rs` | 수정 | `pub mod model;` 추가 |
| `crates/secall/src/main.rs` | 수정 | `Model` 서브커맨드 추가 |
| `Cargo.toml` | 수정 | `sha2` 의존성 추가 |

## Change description

### 1. 모델 파일 구조

`~/.cache/secall/models/bge-m3-onnx/`:
```
bge-m3-onnx/
├── model.onnx          ← ONNX 모델 (~1.2GB)
├── tokenizer.json      ← HuggingFace tokenizer
└── version.json        ← seCall 관리 메타데이터
```

`version.json`:
```json
{
  "model": "BAAI/bge-m3",
  "downloaded_at": "2026-04-06T12:00:00Z",
  "sha256_model": "abc123...",
  "sha256_tokenizer": "def456...",
  "source_revision": "main"
}
```

### 2. ModelManager 구현

```rust
pub struct ModelManager {
    model_dir: PathBuf,
    client: reqwest::Client,
}

impl ModelManager {
    pub fn new(model_dir: PathBuf) -> Self { ... }

    pub fn is_downloaded(&self) -> bool {
        self.model_dir.join("model.onnx").exists()
            && self.model_dir.join("tokenizer.json").exists()
    }

    /// HuggingFace에서 모델 다운로드 (스트리밍 + 진행률)
    pub async fn download(&self, force: bool) -> Result<()> {
        if self.is_downloaded() && !force {
            eprintln!("✓ Model already exists. Use --force to re-download.");
            return Ok(());
        }
        // 1. 임시 파일에 다운로드 (model.onnx.tmp)
        // 2. SHA256 체크섬 계산
        // 3. rename → model.onnx (원자적 교체)
        // 4. tokenizer.json 다운로드
        // 5. version.json 저장
    }

    /// 업데이트 확인
    pub async fn check_update(&self) -> Result<UpdateStatus> {
        // GET https://huggingface.co/api/models/BAAI/bge-m3
        // → lastModified 비교
    }

    pub fn remove(&self) -> Result<()> { ... }
    pub fn info(&self) -> Result<ModelInfo> { ... }
}

pub enum UpdateStatus {
    UpToDate,
    NeedsUpdate { remote_modified: String },
    NotInstalled,
    CheckFailed(String),
}
```

### 3. 다운로드 URL

```
https://huggingface.co/BAAI/bge-m3/resolve/main/onnx/model.onnx
https://huggingface.co/BAAI/bge-m3/resolve/main/tokenizer.json
```

업데이트 체크 API:
```
GET https://huggingface.co/api/models/BAAI/bge-m3
→ { "lastModified": "2024-06-01T...", "sha": "abc..." }
```

### 4. 진행률 표시

`reqwest::Response::bytes_stream()`으로 청크 단위 다운로드:
```
⬇ Downloading model.onnx... 45% (540MB / 1.2GB)
⬇ Downloading tokenizer.json... done
✓ Model downloaded to ~/.cache/secall/models/bge-m3-onnx/
```

### 5. CLI 서브커맨드

```rust
/// Manage ONNX embedding models
Model {
    #[command(subcommand)]
    action: ModelAction,
},

#[derive(Subcommand)]
enum ModelAction {
    /// Download bge-m3 ONNX model from HuggingFace
    Download {
        #[arg(long)]
        force: bool,
    },
    /// Check for model updates
    Check,
    /// Remove downloaded model
    Remove,
    /// Show model info (path, size, version)
    Info,
}
```

### 6. create_vector_indexer() 연동

`vector.rs:177-202`의 `create_vector_indexer()` 수정:
```rust
"ort" => {
    let model_dir = config.embedding.model_path
        .clone()
        .unwrap_or_else(default_model_path);

    // 모델이 없으면 자동 다운로드 시도
    if !model_dir.join("model.onnx").exists() {
        eprintln!("⚠ ONNX model not found. Downloading...");
        let mgr = ModelManager::new(model_dir.clone());
        if let Err(e) = mgr.download(false).await {
            eprintln!("⚠ Download failed: {e}. Trying Ollama fallback...");
            return try_ollama_fallback(config).await;
        }
    }
    // 기존 OrtEmbedder::new() 로직
}
```

### 7. Cargo.toml 의존성

```toml
[workspace.dependencies]
sha2 = "0.10"
```

## Dependencies

- 없음 (다른 task와 독립)
- Phase 2의 `OrtEmbedder` + `create_vector_indexer()` 존재 전제

## Verification

```bash
# 타입 체크
cargo check

# ModelManager 단위 테스트
cargo test -p secall-core model_manager

# CLI 서브커맨드 등록 확인
cargo run -p secall -- model --help
cargo run -p secall -- model info

# 전체 테스트 회귀
cargo test

# 실제 다운로드 (수동, 네트워크 필요)
# Manual: cargo run -p secall -- model download
# Manual: ls -la ~/.cache/secall/models/bge-m3-onnx/
# Manual: cargo run -p secall -- model check
```

테스트 작성 요구사항:
- `test_model_manager_not_downloaded`: 빈 디렉토리 → `is_downloaded() == false`
- `test_version_json_serde`: version.json 직렬화/역직렬화
- `test_default_model_path`: 경로 생성 확인
- `#[ignore] test_download_real`: 실제 다운로드 (네트워크)

## Risks

- **HuggingFace URL 변경**: `resolve/main/` 경로는 안정적이나, 리포 구조 변경 가능
- **다운로드 크기**: ~1.2GB. 디스크 공간 부족 시 명확한 에러 메시지 필요
- **네트워크 타임아웃**: reqwest에 timeout 설정 (connect: 30s, total: 600s)
- **부분 다운로드**: 임시 파일(.tmp)에 다운로드 후 rename으로 원자적 교체

## Scope Boundary

수정 금지 파일:
- `search/embedding.rs` — OrtEmbedder 자체는 변경하지 않음
- `ingest/*` — 파서 변경 금지
- `mcp/*` — MCP 서버 변경 금지
