---
type: task
status: draft
plan: secall-refactor-p2
task_number: 3
title: "디렉토리 ingest 멀티에이전트"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 03: 디렉토리 ingest 멀티에이전트

## 문제

`ingest.rs:128`에서 사용자가 디렉토리를 직접 지정할 때 Claude 세션만 탐색:

```rust
} else if pb.is_dir() {
    find_claude_sessions(Some(&pb))  // ← Claude만
}
```

반면 `--auto` 모드(line 118-120)는 Claude/Codex/Gemini 모두 탐색.

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall/src/commands/ingest.rs:128` | 수정 | 1줄 → 5줄 변경 |

## Change description

### 변경 내용 (3줄 추가)

```rust
// ingest.rs:127-129 — 변경 전
} else if pb.is_dir() {
    find_claude_sessions(Some(&pb))
}

// 변경 후
} else if pb.is_dir() {
    let mut paths = find_claude_sessions(Some(&pb))?;
    paths.extend(find_codex_sessions(Some(&pb))?);
    paths.extend(find_gemini_sessions(Some(&pb))?);
    Ok(paths)
}
```

> `--auto` 모드(line 118-120)와 동일한 패턴. `Some(&pb)`를 전달하여 해당 디렉토리 하위만 탐색.

### 확인 사항

`find_codex_sessions(Some(&pb))`와 `find_gemini_sessions(Some(&pb))`가 지정 디렉토리 기준으로 올바르게 탐색하는지 `detect.rs`에서 확인:

- `find_codex_sessions(Some(dir))`: `dir` 하위의 `.codex/sessions/**/*.jsonl` 탐색
- `find_gemini_sessions(Some(dir))`: `dir` 하위의 `.gemini/**/*.json` 탐색

이 함수들이 `None`일 때는 `~/.codex/`, `~/.gemini/` 기본 경로를 사용하고, `Some(dir)`일 때는 `dir` 하위를 탐색해야 함. detect.rs 구현을 확인하여 `Some(dir)` 동작이 올바른지 검증.

## Dependencies

- 없음 (독립 실행 가능)
- `find_codex_sessions`, `find_gemini_sessions`는 이미 `crates/secall-core/src/ingest/detect.rs`에 구현됨

## Verification

```bash
# 1. 컴파일 확인
cargo check -p secall

# 2. 전체 테스트 회귀 없음
cargo test
```

> **[Developer 필수]** subtask-done 시그널 전에 위 명령의 실행 결과를 result 문서에 기록하세요. 형식: `✅ 명령 — exit 0` 또는 `❌ 명령 — 에러 내용 (사유)`. 검증 증빙 미제출 시 리뷰에서 conditional 처리됩니다.

## Risks

- **낮음**: 3줄 추가. `--auto` 모드와 동일한 패턴이므로 버그 가능성 극히 낮음.
- **find_codex/gemini_sessions의 Some(dir) 동작**: 함수가 `Some(dir)`를 root 디렉토리로 사용하여 하위 탐색하는지 확인 필요. 만약 `None` 전용으로 구현되어 있으면 `Some(dir)` 전달 시 빈 결과 반환 가능. detect.rs 읽기로 사전 확인.

## Scope boundary

다음 파일은 이 task에서 수정하지 않음:
- `crates/secall-core/src/ingest/detect.rs` — 탐색 함수 내부 로직 변경 없음
- `crates/secall/src/commands/ingest.rs:108-122` — auto 모드 및 collect_paths 시그니처 변경 없음
