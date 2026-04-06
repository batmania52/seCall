---
type: task
status: draft
plan: secall-phase-4
task_number: 4
title: "LLM 쿼리 확장"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 04: LLM 쿼리 확장

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/search/query_expand.rs` | **신규 생성** | 쿼리 확장 엔진 |
| `crates/secall-core/src/search/mod.rs` | 수정 | `pub mod query_expand;` 추가 |
| `crates/secall/src/commands/recall.rs` | 수정 | `--expand` 플래그 추가 + 확장 로직 연동 |
| `crates/secall/src/main.rs` | 수정 | Recall에 `expand` 필드 추가 |

## Change description

### 1. 쿼리 확장 개념

사용자 쿼리 → Claude Code가 키워드 + 동의어 + 관련 용어로 확장 → 확장된 쿼리로 검색

예시:
```
원본: "벡터 검색"
확장: "벡터 검색 embedding cosine similarity 유사도 vector search 임베딩"
```

BM25 검색은 정확한 키워드 매칭에 의존하므로, 쿼리 확장으로 recall(재현율)이 크게 향상됨.

### 2. query_expand.rs 구현

```rust
use anyhow::Result;

/// Claude Code를 이용한 쿼리 확장
pub fn expand_query(query: &str) -> Result<String> {
    if !command_exists("claude") {
        // Claude Code 없으면 원본 쿼리 반환
        eprintln!("⚠ claude not found, using original query.");
        return Ok(query.to_string());
    }

    let prompt = format!(
        "다음 검색 쿼리를 확장해주세요. \
         원본 쿼리의 키워드, 동의어, 관련 기술 용어, 영어/한국어 변환을 포함하세요. \
         결과는 공백으로 구분된 키워드만 출력하세요. 설명 없이 키워드만.\n\n\
         쿼리: {query}"
    );

    let output = std::process::Command::new("claude")
        .args(["-p", &prompt, "--model", "claude-haiku-4-5-20251001"])
        .output()?;

    if output.status.success() {
        let expanded = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !expanded.is_empty() {
            eprintln!("✓ Query expanded: {query} → {expanded}");
            // 원본 + 확장 결합
            Ok(format!("{query} {expanded}"))
        } else {
            Ok(query.to_string())
        }
    } else {
        eprintln!("⚠ Query expansion failed, using original query.");
        Ok(query.to_string())
    }
}

fn command_exists(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
```

**모델 선택**: 쿼리 확장은 단순 작업이므로 **Haiku** 사용 (빠르고 저렴). Opus/Sonnet은 과잉.

### 3. CLI 수정

`main.rs` Recall 서브커맨드에 `--expand` 추가:

```rust
Recall {
    query: Vec<String>,
    #[arg(long)]
    since: Option<String>,
    #[arg(long, short)]
    project: Option<String>,
    #[arg(long, short)]
    agent: Option<String>,
    #[arg(long, short = 'n', default_value = "10")]
    limit: usize,
    #[arg(long)]
    lex: bool,
    #[arg(long)]
    vec: bool,
    /// Expand query using Claude Code (requires claude CLI)
    #[arg(long)]
    expand: bool,
},
```

### 4. recall.rs 수정

```rust
pub async fn run(
    query: Vec<String>,
    since: Option<String>,
    project: Option<String>,
    agent: Option<String>,
    limit: usize,
    lex_only: bool,
    vec_only: bool,
    expand: bool,       // 신규
    format: &OutputFormat,
) -> Result<()> {
    let query_str = query.join(" ");

    // 쿼리 확장
    let final_query = if expand {
        secall_core::search::query_expand::expand_query(&query_str)?
    } else {
        query_str.clone()
    };

    // ... 이하 기존 로직에서 query_str → final_query 사용 ...
}
```

### 5. 성능 고려

- Claude Code subprocess 호출은 ~1-3초 소요 (Haiku 기준)
- 대화형 검색에서는 허용 가능한 지연
- `--expand` 없이는 기존 동작 유지 (0 지연)

## Dependencies

- 없음 (다른 task와 독립)
- Claude Code CLI(`claude`)가 PATH에 있어야 함 (없으면 graceful fallback)

## Verification

```bash
# 타입 체크
cargo check

# CLI 옵션 등록 확인
cargo run -p secall -- recall --help

# expand 없이 기존 동작 확인
cargo run -p secall -- recall "테스트 쿼리"

# dry-run: expand가 Claude Code를 호출하는지 확인
# Manual: cargo run -p secall -- recall "벡터 검색" --expand
# (Claude Code가 설치되어 있으면 확장된 쿼리 출력)

# Claude Code 없는 환경에서 fallback 확인
# Manual: PATH에서 claude 제거 후 --expand → 원본 쿼리 사용

# 전체 테스트 회귀
cargo test
```

테스트 작성 요구사항:
- `test_expand_query_no_claude`: `claude` CLI 없을 때 원본 쿼리 반환
- `test_command_exists_false`: 존재하지 않는 명령어 → false
- `#[ignore] test_expand_query_real`: 실제 Claude Code 호출 (수동)

## Risks

- **Claude Code 호출 비용**: Haiku는 저렴하지만 호출당 ~$0.001. 빈번한 검색 시 비용 누적 가능
- **Claude Code 비설치 환경**: CI/CD 등에서 `claude` 없음 → graceful fallback으로 대응
- **확장 품질**: Haiku가 도메인 특화 용어를 잘못 확장할 수 있음. 원본 쿼리를 항상 포함하여 최소 보장
- **subprocess 보안**: 사용자 입력(query)이 `-p` 인자로 전달됨. shell injection은 `Command`가 방지하지만, 프롬프트 injection은 가능. 검색 쿼리에 한정하므로 위험도 낮음

## Scope Boundary

수정 금지 파일:
- `search/hybrid.rs` — 검색 엔진 내부 로직 변경 금지
- `search/bm25.rs` — BM25 인덱서 변경 금지
- `search/embedding.rs` — 임베딩 변경 금지
- `mcp/*` — MCP 서버 변경 금지 (MCP의 recall은 이 task 범위 밖)
- `ingest/*` — 파서 변경 금지
