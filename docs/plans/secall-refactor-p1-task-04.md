---
type: task
status: draft
plan: secall-refactor-p1
task_number: 4
title: "Codex/Gemini 타임스탬프 복원"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 04: Codex/Gemini 타임스탬프 복원

## 문제

Codex와 Gemini 파서가 `start_time: Utc::now()`를 사용하여 파싱 시점의 시간을 저장함.
실제 세션이 발생한 시간이 아니므로 날짜 필터(`--since`, `--until`) 검색 결과가 부정확.

| 파서 | 현재 | 가용 데이터 | 대안 |
|------|------|------------|------|
| Claude | `first_timestamp` from JSONL ✅ | JSONL 각 행에 `timestamp` 필드 | — |
| Gemini | `Utc::now()` ❌ | `gs.create_time: Option<String>` (RFC3339) | `create_time` 파싱 |
| Codex | `Utc::now()` ❌ | JSON에 timestamp 없음 | 파일 mtime fallback |

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/ingest/gemini.rs:194-205` | 수정 | `start_time` 파싱, `end_time` 설정 |
| `crates/secall-core/src/ingest/codex.rs:152-163` | 수정 | 파일 mtime → `start_time` |
| `crates/secall-core/src/ingest/codex.rs:55` | 수정 | `parse_codex_jsonl(path)` 시그니처에 mtime 전달 방식 |

## Change description

### Part A: Gemini 타임스탬프 복원

#### Step 1: create_time/update_time 파싱

```rust
// gemini.rs:194-205 — 변경 전
Ok(Session {
    id: gs.id,
    agent: AgentKind::GeminiCli,
    model: None,
    project,
    cwd: None,
    git_branch: None,
    start_time: Utc::now(),    // ← 현재 시간
    end_time: None,
    turns,
    total_tokens: Default::default(),
})

// 변경 후
use chrono::DateTime;

let start_time = gs.create_time
    .as_deref()
    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
    .map(|dt| dt.with_timezone(&Utc))
    .unwrap_or_else(Utc::now);

let end_time = gs.update_time
    .as_deref()
    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
    .map(|dt| dt.with_timezone(&Utc));

Ok(Session {
    id: gs.id,
    agent: AgentKind::GeminiCli,
    model: None,
    project,
    cwd: None,
    git_branch: None,
    start_time,
    end_time,
    turns,
    total_tokens: Default::default(),
})
```

> `create_time`이 None이거나 파싱 실패 시 `Utc::now()` fallback (기존 동작 유지).
> `update_time`은 `end_time`에 매핑 (세션 종료 시간).

#### Step 2: Gemini 테스트 수정

기존 `BASIC_SESSION` 테스트 데이터에 이미 `createTime`/`updateTime`이 포함됨:

```rust
// gemini.rs tests — 추가
#[test]
fn test_gemini_timestamps_parsed() {
    let f = make_gemini_file(BASIC_SESSION);
    let session = parse_gemini_json(f.path()).unwrap();
    // create_time "2026-04-05T10:00:00Z" → start_time
    assert_eq!(session.start_time.date_naive().to_string(), "2026-04-05");
    // update_time "2026-04-05T10:30:00Z" → end_time
    assert!(session.end_time.is_some());
    assert_eq!(session.end_time.unwrap().date_naive().to_string(), "2026-04-05");
}

#[test]
fn test_gemini_missing_timestamps_fallback() {
    let json = r#"{"id": "s-no-time", "messages": [
        {"role":"user","parts":[{"text":"hello"}]},
        {"role":"model","parts":[{"text":"hi"}]}
    ]}"#;
    let f = make_gemini_file(json);
    let session = parse_gemini_json(f.path()).unwrap();
    // create_time 없으면 Utc::now() 근처 시간이어야 함
    let diff = (Utc::now() - session.start_time).num_seconds().abs();
    assert!(diff < 5, "fallback start_time should be near Utc::now()");
    assert!(session.end_time.is_none());
}
```

### Part B: Codex 타임스탬프 복원

#### Step 3: 파일 mtime 추출

Codex JSONL에는 타임스탬프 필드가 없으므로 파일 시스템 메타데이터를 사용.

```rust
// codex.rs:55 — parse_codex_jsonl 함수 내, file open 직후
let file = std::fs::File::open(path)?;
let file_mtime = file.metadata()
    .and_then(|m| m.modified())
    .ok()
    .and_then(|st| {
        let duration = st.duration_since(std::time::UNIX_EPOCH).ok()?;
        DateTime::from_timestamp(duration.as_secs() as i64, duration.subsec_nanos())
    });
```

#### Step 4: start_time에 mtime 적용

```rust
// codex.rs:152-163 — 변경 전
Ok(Session {
    ...
    start_time: Utc::now(),
    end_time: None,
    ...
})

// 변경 후
Ok(Session {
    ...
    start_time: file_mtime.unwrap_or_else(Utc::now),
    end_time: None,
    ...
})
```

> mtime은 파일이 마지막으로 수정된 시간. Codex 세션 파일은 세션 중에 라인이 추가되므로 mtime ≈ 세션 종료 시간. 정확한 start_time은 아니지만 `Utc::now()`보다 훨씬 정확.

#### Step 5: Codex 테스트 수정

```rust
// codex.rs tests — 추가
#[test]
fn test_codex_timestamp_from_file_mtime() {
    let f = make_codex_file(&[
        r#"{"type":"user","message":{"role":"user","content":"hello"}}"#,
        r#"{"type":"assistant","message":{"role":"assistant","content":"hi"}}"#,
    ]);
    let session = parse_codex_jsonl(f.path()).unwrap();
    // mtime은 방금 생성한 파일이므로 현재 시간 근처
    let diff = (Utc::now() - session.start_time).num_seconds().abs();
    assert!(diff < 10, "start_time from mtime should be recent");
}
```

## Dependencies

- 없음 (독립 실행 가능)
- Task 01, 02와 병렬 실행 가능 (parallel_group: A)

## Verification

```bash
# 1. 컴파일 확인
cargo check -p secall-core

# 2. Gemini 파서 테스트
cargo test -p secall-core gemini

# 3. Codex 파서 테스트
cargo test -p secall-core codex

# 4. 전체 테스트 회귀 없음
cargo test
```

> **[Developer 필수]** subtask-done 시그널 전에 위 명령의 실행 결과를 result 문서에 기록하세요. 형식: `✅ 명령 — exit 0` 또는 `❌ 명령 — 에러 내용 (사유)`. 검증 증빙 미제출 시 리뷰에서 conditional 처리됩니다.

## Risks

- **Codex mtime 부정확**: 파일 복사, 압축 해제, git clone 등으로 mtime이 변경될 수 있음. 그래도 `Utc::now()`(파싱 시점)보다는 원본에 가까움.
- **Gemini create_time 형식**: RFC3339 외 형식이 올 가능성. `parse_from_rfc3339` 실패 시 fallback으로 처리됨.
- **기존 DB 데이터**: 이미 `Utc::now()`로 저장된 세션은 업데이트되지 않음. 재인덱싱(`secall ingest --force`) 시에만 새 타임스탬프 적용.
- **SystemTime 크로스 플랫폼**: `UNIX_EPOCH` 기반 변환은 Linux/macOS에서 안전. Windows에서도 동작하나 1970년 이전 파일은 에러 → fallback 처리됨.

## Scope boundary

다음 파일은 이 task에서 수정하지 않음:
- `crates/secall-core/src/ingest/claude.rs` — Claude 파서는 이미 정확한 타임스탬프 추출 구현됨 (참조 구현)
- `crates/secall-core/src/ingest/types.rs` — `Session` 구조체의 `start_time: DateTime<Utc>` 타입 변경 없음
- `crates/secall-core/src/store/schema.rs` — `sessions.start_time TEXT` 스키마 변경 없음
