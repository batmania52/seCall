---
type: task
status: draft
plan: secall-p6
task_number: 2
title: "Sync pull 안전성"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 02: Sync pull 안전성

## 문제

`secall sync`의 Phase 1 (git pull)이 이전 sync에서 push되지 않은 파일이 남아있으면 실패한다.

```
error: 리베이스로 풀하기 할 수 없습니다: 스테이징하지 않은 변경 사항이 있습니다.
```

### 재현 시나리오

1. `secall sync` 실행 → Phase 3 (ingest)에서 새 MD 파일 생성
2. Phase 4 (push) 전에 네트워크 오류 or 프로세스 중단
3. 다음 `secall sync` 실행 → Phase 1 (pull) 실패 (unstaged changes)
4. 이후 모든 sync가 pull 단계에서 차단

### 현재 코드

```rust
// sync.rs:22-40 — Phase 1: Pull
if !local_only && vault_git.is_git_repo() {
    // 바로 pull 시도 → unstaged 변경이 있으면 실패
    match vault_git.pull() { ... }
}
```

`git.rs:47-79` — `pull()` 메서드에도 사전 정리 로직 없음.

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/vault/git.rs:82-103` | 수정 | `auto_commit()` 메서드 추가, `push()` 앞에 stage 로직 분리 |
| `crates/secall-core/src/vault/git.rs:47-79` | 수정 | `pull()` 내부에 auto_commit 호출 추가 |
| `crates/secall/src/commands/sync.rs:21-41` | 수정 | Phase 1 전에 auto_commit 호출 |

## Change description

### Step 1: git.rs에 auto_commit() 메서드 추가

`crates/secall-core/src/vault/git.rs` — push() (lines 82-103) 옆에 추가:

```rust
/// unstaged 변경이 있으면 자동 커밋. pull 전에 호출하여 rebase 충돌 방지.
pub fn auto_commit(&self) -> crate::error::Result<bool> {
    if !self.is_git_repo() {
        return Ok(false);
    }

    let status = self.run_git(&["status", "--porcelain"])?;
    let changes = String::from_utf8_lossy(&status.stdout);
    if changes.trim().is_empty() {
        return Ok(false);
    }

    let change_count = changes.lines().count();
    tracing::info!(changes = change_count, "auto-committing unstaged vault changes before pull");

    // vault 관련 파일만 stage (raw/, wiki/, index.md, log.md, .gitignore)
    self.run_git(&["add", "raw/", "wiki/", "index.md", "log.md", ".gitignore"])?;
    self.run_git(&["commit", "-m", "auto: uncommitted vault changes"])?;

    Ok(true)
}
```

> **핵심 결정**: stash 대신 commit. 이유:
> - vault 파일은 모두 유효한 세션 데이터이므로 stash→pop보다 commit이 안전
> - stash pop 시 충돌 가능성 (같은 index.md 수정), commit은 rebase로 자동 해결
> - push phase에서 어차피 commit+push 하므로, 선 commit이 자연스러움

### Step 2: pull() 전에 auto_commit 호출

방법 A — sync.rs에서 호출 (권장):

`crates/secall/src/commands/sync.rs` — Phase 1 (lines 21-41) 수정:

```rust
// === Phase 0: 이전 sync에서 push되지 않은 변경 자동 커밋 ===
if !local_only && vault_git.is_git_repo() {
    match vault_git.auto_commit() {
        Ok(true) => eprintln!("Auto-committed pending vault changes."),
        Ok(false) => {} // no changes
        Err(e) => {
            tracing::warn!(error = %e, "auto-commit failed");
            eprintln!("  ⚠ Auto-commit failed: {e}");
        }
    }
}

// === Phase 1: Pull (다른 기기 세션 수신) ===
if !local_only && vault_git.is_git_repo() {
    // ... 기존 pull 로직 ...
}
```

방법 B — git.rs pull() 내부에서 호출:
> 이 방법은 pull()의 책임이 과대해지므로 비권장. sync.rs에서 명시적 호출이 더 명확.

### Step 3: push()에서 auto_commit 재사용 (리팩토링)

현재 push() (lines 82-103)의 stage+commit 로직이 auto_commit()과 유사. push()를 간소화:

```rust
pub fn push(&self, message: &str) -> crate::error::Result<PushResult> {
    if !self.is_git_repo() {
        return Ok(PushResult { committed: 0 });
    }

    let status = self.run_git(&["status", "--porcelain"])?;
    let changes = String::from_utf8_lossy(&status.stdout);
    if changes.trim().is_empty() {
        return Ok(PushResult { committed: 0 });
    }

    let committed = changes.lines().count();

    self.run_git(&["add", "raw/", "wiki/", "index.md", "log.md"])?;
    self.run_git(&["commit", "-m", message])?;
    self.run_git(&["push", "origin", "main"])?;

    tracing::info!(committed, "vault changes pushed");
    Ok(PushResult { committed })
}
```

> push()는 기존 구조 유지. auto_commit()은 별도 메서드로 존재하여 pull 전용으로 사용.

## Dependencies

- 없음
- Task 01, 03과 독립적으로 구현 가능

## Verification

```bash
# 1. 컴파일 확인
cargo check --all

# 2. 전체 테스트 통과
cargo test --all

# 3. 시뮬레이션: unstaged 파일이 있는 상태에서 sync 성공 확인
# (vault에 임시 파일 생성 → sync 실행 → pull 성공)
touch "/Users/d9ng/Documents/Obsidian Vault/seCall/raw/sessions/test_dummy.md" && \
  cargo run -p secall -- sync --local-only 2>&1 && \
  rm -f "/Users/d9ng/Documents/Obsidian Vault/seCall/raw/sessions/test_dummy.md" && \
  echo "OK: sync succeeded with unstaged files"

# 4. clippy 통과
cargo clippy --all-targets -- -D warnings

# 5. auto_commit 로그 확인 (vault에 변경이 있을 때)
# Manual: vault에 파일 추가 후 `secall sync` 실행 → "Auto-committed" 메시지 확인
```

## Risks

- **auto_commit 대상 범위**: `raw/`, `wiki/`, `index.md`, `log.md`만 stage. 사용자가 vault에 비관련 파일을 넣으면 제외됨. 의도적 — vault 외 파일은 사용자가 직접 관리.
- **commit 메시지 품질**: "auto: uncommitted vault changes"는 git log에서 식별 가능하나 세부 내용 없음. 충분함 — push phase에서 의미 있는 메시지로 다시 commit.
- **rebase 충돌**: auto_commit 후 pull --rebase에서 여전히 충돌 가능 (같은 index.md 수정 시). 하지만 index.md는 append-only이므로 실질적 충돌 확률 매우 낮음.

## Scope boundary

수정 금지 파일:
- `crates/secall-core/src/search/ann.rs` — Task 01 영역
- `crates/secall-core/src/search/vector.rs` — Task 03 영역
- `crates/secall/src/commands/ingest.rs` — Task 03 영역
- `crates/secall/src/commands/reindex.rs` — reindex 로직 변경 없음
- `crates/secall-core/src/vault/config.rs` — 설정 변경 없음
