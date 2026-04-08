---
type: task
plan: secall-p12-timezone
task_number: 4
title: 테스트 + 문서
depends_on: [2, 3]
parallel_group: C
status: draft
updated_at: 2026-04-08
---

# Task 04: 테스트 + 문서

## Changed files

| 파일 | 변경 유형 | 설명 |
|------|----------|------|
| `crates/secall-core/src/ingest/markdown.rs` (tests 섹션) | 수정 | 타임존 변환 단위 테스트 추가 |
| `crates/secall-core/src/vault/config.rs` (tests 섹션) | 수정 | timezone 파싱 테스트 추가 |
| `CHANGELOG.md` | 수정 | v0.2.2 섹션 추가 |
| `README.md` | 수정 | 한국어/영어 양쪽에 타임존 설정 안내 추가 |
| `Cargo.toml` (workspace root):12 | 수정 | version `0.2.1` → `0.2.2` |

## Change description

### 1단계: config.rs 타임존 파싱 테스트

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timezone_default_is_utc() {
        let config = Config::default();
        assert_eq!(config.output.timezone, "UTC");
        assert_eq!(config.timezone(), chrono_tz::Tz::UTC);
    }

    #[test]
    fn test_timezone_valid_iana() {
        let mut config = Config::default();
        config.output.timezone = "Asia/Seoul".to_string();
        assert_eq!(config.timezone(), chrono_tz::Tz::Asia__Seoul);
    }

    #[test]
    fn test_timezone_invalid_falls_back_to_utc() {
        let mut config = Config::default();
        config.output.timezone = "INVALID/TZ".to_string();
        assert_eq!(config.timezone(), chrono_tz::Tz::UTC);
    }

    #[test]
    fn test_config_without_output_section() {
        // [output] 섹션 없는 TOML에서 기본값 적용 확인
        let toml_str = r#"
[vault]
path = "/tmp/test-vault"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.output.timezone, "UTC");
    }
}
```

### 2단계: markdown.rs 타임존 렌더링 테스트

```rust
#[test]
fn test_render_session_with_kst_timezone() {
    let session = make_session(vec![]);
    let tz: chrono_tz::Tz = "Asia/Seoul".parse().unwrap();
    let md = render_session(&session, tz);

    // UTC 05:30 → KST 14:30
    assert!(md.contains("date: 2026-04-05"));
    assert!(md.contains("+09:00"));
    assert!(md.contains("14:30"));
}

#[test]
fn test_render_session_utc_default() {
    let session = make_session(vec![]);
    let md = render_session(&session, chrono_tz::Tz::UTC);

    // 기존 동작과 동일
    assert!(md.contains("date: 2026-04-05"));
    assert!(md.contains("+00:00"));
    assert!(md.contains("05:30"));
}

#[test]
fn test_vault_path_uses_timezone_date() {
    let session = make_session(vec![]);
    // UTC 2026-04-05T05:30 → 날짜 변경 없음
    let path_utc = session_vault_path(&session, chrono_tz::Tz::UTC);
    assert!(path_utc.to_string_lossy().contains("2026-04-05"));

    // UTC 23:00 → KST 다음날 08:00 되는 케이스는 별도 세션으로 테스트
}

#[test]
fn test_vault_path_date_crosses_midnight() {
    // UTC 2026-04-05T15:30 → KST 2026-04-06T00:30 (날짜 변경!)
    let mut session = make_session(vec![]);
    session.start_time = chrono::Utc.with_ymd_and_hms(2026, 4, 5, 15, 30, 0).unwrap();
    let tz: chrono_tz::Tz = "Asia/Seoul".parse().unwrap();
    let path = session_vault_path(&session, tz);
    assert!(path.to_string_lossy().contains("2026-04-06")); // KST 기준 다음날
}
```

### 3단계: CHANGELOG 업데이트

```markdown
## v0.2.2 (2026-04-XX)

### Added
- `config.toml`에 `[output] timezone` 설정 추가 — IANA 타임존(예: `Asia/Seoul`)으로 vault 마크다운 타임스탬프 렌더링. 기본값 UTC.

### Changed
- vault 디렉토리 경로(`raw/sessions/YYYY-MM-DD/`)가 설정된 타임존 기준 날짜로 생성
- frontmatter `start_time`/`end_time`에 동적 UTC 오프셋 적용 (예: `+09:00`)
```

### 4단계: README 업데이트

한국어/영어 양쪽 섹션에 Config 예시 추가:

```toml
# ~/.config/secall/config.toml
[output]
timezone = "Asia/Seoul"    # 기본값: UTC
```

Quick Start 또는 Configuration 섹션에 짧게 추가. Updates 히스토리 테이블에 행 추가.

### 5단계: 버전 업데이트

`Cargo.toml` workspace version: `0.2.1` → `0.2.2`

## Dependencies

- **Task 02**: `render_session(session, tz)` 시그니처 확정 필요
- **Task 03**: 보조 렌더링 위치 적용 완료 필요

## Verification

```bash
# 1. config 타임존 파싱 테스트
cargo test -p secall-core -- vault::config

# 2. markdown 렌더링 타임존 테스트
cargo test -p secall-core -- ingest::markdown

# 3. 전체 테스트 통과
cargo test --all

# 4. 버전 확인
cargo metadata --format-version 1 | grep -o '"version":"0\.2\.2"' | head -1

# 5. clippy 경고 없음
cargo clippy --all-targets -- -D warnings
```

## Risks

| 리스크 | 영향 | 대응 |
|--------|------|------|
| 기존 테스트 UTC 하드코딩 | `+00:00` 가정 assert가 실패할 수 있음 | `render_session(session, chrono_tz::Tz::UTC)` 호출로 기존 테스트 수정 |
| README 양쪽 섹션 동기화 | 한쪽만 업데이트 위험 | 체크리스트로 양쪽 확인 |

## Scope boundary

수정 금지 파일:
- `crates/secall-core/src/vault/index.rs` — Task 03 완료 후에만 테스트
- `crates/secall-core/src/vault/log.rs` — Task 03 완료 후에만 테스트
- `crates/secall-core/src/hooks/mod.rs` — Task 03 완료 후에만 테스트

수정 허용 (테스트 추가만):
- `crates/secall-core/src/ingest/markdown.rs` — 테스트 섹션만 수정
- `crates/secall-core/src/vault/config.rs` — 테스트 섹션만 수정
