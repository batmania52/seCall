---
type: task
status: draft
plan: secall-extensions-nlp
task_number: 4
title: "kiwi-rs 토크나이저"
parallel_group: B
depends_on: []
updated_at: 2026-04-06
---

# Task 04: kiwi-rs 토크나이저

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/search/tokenizer.rs` | 수정 | `KiwiTokenizer` 추가, config 기반 선택 로직 |
| `crates/secall-core/src/search/bm25.rs` | 수정 | `Bm25Indexer::new()` 시그니처 유지 확인 |
| `crates/secall-core/src/vault/config.rs` | 수정 | `SearchConfig`에 `tokenizer` 필드 추가 |
| `Cargo.toml` | 수정 | `kiwi-rs` 의존성 추가 |
| `crates/secall-core/Cargo.toml` | 수정 | `kiwi-rs` 의존성 추가 |

## Change description

### 1. 현재 Tokenizer 구조 (tokenizer.rs)

기존 trait은 이미 추출되어 있음 (`tokenizer.rs:12-18`):
```rust
pub trait Tokenizer: Send + Sync {
    fn tokenize(&self, text: &str) -> Vec<String>;
    fn tokenize_for_fts(&self, text: &str) -> String {
        self.tokenize(text).join(" ")
    }
}
```

구현체:
- `LinderaKoTokenizer` (tokenizer.rs:20-66) — lindera 2.3.4 + KoDic
- `SimpleTokenizer` (tokenizer.rs:69-75) — whitespace fallback

`Bm25Indexer`는 이미 `Box<dyn Tokenizer>`를 받음 (bm25.rs 확인 필요).

### 2. KiwiTokenizer 구현

```rust
use kiwi_rs::Kiwi;

pub struct KiwiTokenizer {
    kiwi: Kiwi,
}

impl KiwiTokenizer {
    pub fn new() -> Result<Self> {
        // Kiwi::init() → 모델 자동 다운로드 (~/.cache/kiwi/)
        // 첫 호출 시 네트워크 필요, 이후 캐시 사용
        let kiwi = Kiwi::init()?;
        Ok(Self { kiwi })
    }
}

impl Tokenizer for KiwiTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        match self.kiwi.tokenize(text) {
            Ok(tokens) => {
                tokens.into_iter()
                    .filter(|t| {
                        // POS 필터: NNG, NNP, NNB, VV, VA, SL (lindera와 동일 기준)
                        matches!(t.tag.as_str(),
                            "NNG" | "NNP" | "NNB" | "VV" | "VA" | "SL"
                        )
                    })
                    .map(|t| t.form.to_lowercase())
                    .filter(|s| s.chars().count() > 1)
                    .collect()
            }
            Err(_) => tokenize_fallback(text),
        }
    }
}
```

kiwi-rs 0.1.4 API:
- `Kiwi::init()` → 모델 자동 다운로드 + 로드
- `kiwi.tokenize(text)` → `Vec<Token>` where `Token { form: String, tag: String, start: usize, len: usize }`
- POS tag set: 세종 태그셋 (NNG, NNP, VV, VA, SL 등)

### 3. Config 확장 (config.rs)

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SearchConfig {
    pub default_limit: usize,
    /// "lindera" | "kiwi"
    pub tokenizer: String,
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            default_limit: 10,
            tokenizer: "lindera".to_string(),  // 기존 동작 유지
        }
    }
}
```

config.toml 예시:
```toml
[search]
tokenizer = "kiwi"
default_limit = 10
```

### 4. 토크나이저 팩토리 함수 (tokenizer.rs)

```rust
pub fn create_tokenizer(backend: &str) -> Result<Box<dyn Tokenizer>> {
    match backend {
        "kiwi" => {
            match KiwiTokenizer::new() {
                Ok(t) => {
                    eprintln!("✓ kiwi-rs tokenizer loaded.");
                    Ok(Box::new(t))
                }
                Err(e) => {
                    eprintln!("⚠ kiwi-rs failed: {e}. Falling back to lindera.");
                    Ok(Box::new(LinderaKoTokenizer::new()?))
                }
            }
        }
        "lindera" | _ => Ok(Box::new(LinderaKoTokenizer::new()?)),
    }
}
```

### 5. BM25 FTS5 연동

`Bm25Indexer`는 이미 `Box<dyn Tokenizer>`를 생성자에서 받으므로 변경 불필요.
팩토리 함수를 호출하는 곳 (CLI 초기화, MCP 서버 초기화)에서 config 기반으로 `create_tokenizer()` 호출.

확인 필요: FTS5 custom tokenizer 등록 방식. 현재 FTS5 인덱싱 시 Rust 코드에서 먼저 토크나이즈 후 토큰 연결 문자열을 FTS5에 저장하는 방식이라면, `Tokenizer` trait만 교체하면 됨. FTS5 자체의 tokenizer 플러그인을 사용하는 경우 추가 작업 필요.

### 6. Cargo.toml 의존성

```toml
[workspace.dependencies]
# 추가
kiwi-rs = "0.1.4"
```

## Dependencies

- 없음 (Task 03과 병렬 실행 가능)
- MVP Task 06 (BM25 인덱서) 코드가 존재해야 함

## Verification

```bash
# 타입 체크
cargo check -p secall-core

# KiwiTokenizer 단위 테스트
cargo test -p secall-core kiwi

# 기존 lindera 토크나이저 테스트 회귀 없음
cargo test -p secall-core tokenizer

# BM25 테스트 회귀 없음
cargo test -p secall-core bm25

# 전체 테스트
cargo test -p secall-core
```

테스트 작성 요구사항:
- `test_kiwi_korean_tokenization`: "아키텍처를 설계한다" → NNG/VV 토큰 추출
- `test_kiwi_english_tokenization`: "Rust workspace" → SL 토큰
- `test_kiwi_mixed_tokenization`: "seCall의 BM25 검색" → 혼합 토큰
- `test_kiwi_empty`: 빈 문자열 → 빈 벡터
- `test_create_tokenizer_lindera`: "lindera" → LinderaKoTokenizer
- `test_create_tokenizer_kiwi`: "kiwi" → KiwiTokenizer
- `test_create_tokenizer_fallback`: 알 수 없는 값 → lindera fallback

## Risks

- **kiwi-rs 모델 다운로드**: `Kiwi::init()` 첫 호출 시 네트워크 필요. CI/CD 환경에서 캐시 설정 필요
- **kiwi-rs 모델 크기**: ~50MB. 디스크 공간 확인
- **POS 태그 차이**: kiwi의 태그셋이 lindera(mecab-ko-dic 기반)과 미세하게 다를 수 있음. 동일 기준(NNG, NNP, NNB, VV, VA, SL)으로 필터하되 결과 품질 비교 필요
- **FTS5 재인덱싱**: 토크나이저 변경 시 기존 FTS5 인덱스와 불일치. `secall embed --all` 유사하게 `secall reindex` 필요할 수 있으나 이 태스크 범위 밖
- **Send + Sync**: `Kiwi` 타입이 `Send + Sync`인지 확인. 아닐 경우 `Arc<Mutex<Kiwi>>` 래핑 필요

## Scope Boundary

수정 금지 파일:
- `search/embedding.rs` — Task 03 영역
- `search/vector.rs` — Task 03 영역
- `ingest/*` — 파서는 이 태스크에서 변경하지 않음
- `mcp/*` — MCP 서버는 이 태스크에서 변경하지 않음
