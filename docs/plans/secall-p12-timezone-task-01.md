---
type: task
plan: secall-p12-timezone
task_number: 1
title: 의존성 + Config 구조체
depends_on: []
parallel_group: A
status: draft
updated_at: 2026-04-08
---

# Task 01: 의존성 + Config 구조체

## Changed files

| 파일 | 변경 유형 | 설명 |
|------|----------|------|
| `Cargo.toml` (workspace root):38 | 수정 | `[workspace.dependencies]`에 `chrono-tz` 추가 |
| `crates/secall-core/Cargo.toml` | 수정 | `chrono-tz.workspace = true` 추가 |
| `crates/secall-core/src/vault/config.rs`:8-14 | 수정 | `Config`에 `output: OutputConfig` 필드 추가 |
| `crates/secall-core/src/vault/config.rs` (신규 struct) | 수정 | `OutputConfig` 구조체 + `Default` impl 추가 |
| `crates/secall-core/src/vault/config.rs`:60-75 | 수정 | `Config::default()`에 `output` 필드 추가 |

## Change description

### 1단계: chrono-tz 의존성 추가

```toml
# Cargo.toml (workspace root) — [workspace.dependencies] 섹션
chrono-tz = "0.10"
```

```toml
# crates/secall-core/Cargo.toml — [dependencies] 섹션
chrono-tz.workspace = true
```

### 2단계: OutputConfig 구조체 추가

`config.rs`에 새 구조체 추가:

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct OutputConfig {
    /// IANA timezone name (e.g. "Asia/Seoul", "America/New_York")
    /// Default: "UTC"
    pub timezone: String,
}

impl Default for OutputConfig {
    fn default() -> Self {
        OutputConfig {
            timezone: "UTC".to_string(),
        }
    }
}
```

### 3단계: Config에 output 필드 추가

`Config` 구조체(line 8)에 `pub output: OutputConfig` 추가.
`Config::default()`(line 60)에 `output: OutputConfig::default()` 추가.

### 4단계: 타임존 파싱 헬퍼

`Config`에 파싱된 타임존을 반환하는 메서드 추가:

```rust
impl Config {
    /// 설정된 타임존을 chrono_tz::Tz로 파싱.
    /// 잘못된 값이면 UTC로 fallback + 경고 로그.
    pub fn timezone(&self) -> chrono_tz::Tz {
        self.output.timezone.parse::<chrono_tz::Tz>().unwrap_or_else(|_| {
            tracing::warn!(tz = &self.output.timezone, "invalid timezone, falling back to UTC");
            chrono_tz::Tz::UTC
        })
    }
}
```

## Dependencies

- 없음 (첫 번째 태스크)

## Verification

```bash
# 1. chrono-tz 의존성 해소 + 컴파일
cargo check -p secall-core

# 2. 기존 테스트 통과 (리그레션 없음)
cargo test -p secall-core

# 3. timezone 파싱 기능 확인 (Task 04에서 정식 테스트 추가)
cargo check --all
```

## Risks

| 리스크 | 영향 | 대응 |
|--------|------|------|
| `chrono-tz` 컴파일 시간 증가 | TZ 데이터베이스 임베딩으로 ~10-15초 추가 | 허용 범위. release 빌드에서만 영향 |
| serde 호환성 | `OutputConfig`의 `timezone` 필드가 TOML 역직렬화 실패 시 | `#[serde(default)]`로 기본값 UTC fallback |
| 기존 config.toml에 `[output]` 없음 | 기존 사용자 config 파싱 실패 가능 | `#[serde(default)]`로 해결 — 없으면 기본값 |

## Scope boundary

수정 금지 파일:
- `crates/secall-core/src/ingest/markdown.rs` — Task 02 영역
- `crates/secall-core/src/vault/index.rs` — Task 03 영역
- `crates/secall-core/src/vault/log.rs` — Task 03 영역
- `crates/secall-core/src/search/chunker.rs` — Task 03 영역
- `crates/secall-core/src/hooks/mod.rs` — Task 03 영역
- `crates/secall-core/src/vault/init.rs` — Task 03 영역
