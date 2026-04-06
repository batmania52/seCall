---
type: task
plan: secall-mvp
task_number: 11
title: Ingest 완료 이벤트 + hook
status: draft
parallel_group: 3
depends_on: [9]
updated_at: 2026-04-05
---

# Task 11: Ingest 완료 이벤트 + hook

## Changed Files

- `crates/secall-core/src/lib.rs` — `pub mod hooks;` 추가
- `crates/secall-core/src/hooks/mod.rs` — **신규**. Hook 시스템
- `crates/secall-core/src/vault/config.rs` — `post_ingest_hook` 설정 필드 추가
- `crates/secall/src/commands/ingest.rs` — ingest 완료 후 이벤트 출력 + hook 실행

## Change Description

### 1. Ingest 완료 이벤트 (stdout JSON)

ingest 완료 시 stdout에 구조화된 이벤트 출력:

```json
{
  "event": "ingest_complete",
  "session_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "agent": "claude-code",
  "project": "seCall",
  "date": "2026-04-05",
  "vault_path": "raw/sessions/2026-04-05/claude-code_seCall_a1b2c3d4.md",
  "turns": 23,
  "tokens": { "input": 45000, "output": 12000 },
  "index": {
    "bm25_indexed": true,
    "vector_indexed": true,
    "chunks_embedded": 45
  },
  "timestamp": "2026-04-05T15:50:00+09:00"
}
```

`--format json` 모드에서만 이벤트 출력. `--format text`에서는 사람이 읽을 수 있는 요약.

### 2. Post-ingest Hook

config.toml 설정:
```toml
[hooks]
post_ingest = "~/.config/secall/hooks/post-ingest.sh"
```

Hook 스크립트에 환경변수로 정보 전달:
```bash
#!/bin/bash
# ~/.config/secall/hooks/post-ingest.sh
# 환경변수:
#   SECALL_SESSION_ID
#   SECALL_AGENT
#   SECALL_PROJECT
#   SECALL_VAULT_PATH (절대 경로)
#   SECALL_TURNS
#   SECALL_DATE

echo "Session ingested: $SECALL_SESSION_ID ($SECALL_PROJECT)"
# 여기서 에이전트 호출하여 wiki 업데이트 트리거 가능
```

```rust
pub fn run_post_ingest_hook(config: &Config, session: &Session, vault_path: &Path) -> Result<()> {
    let hook_path = match &config.hooks.post_ingest {
        Some(p) => expand_tilde(p),
        None => return Ok(()),  // hook 미설정 시 skip
    };

    if !hook_path.exists() {
        eprintln!("⚠ post_ingest hook not found: {}", hook_path.display());
        return Ok(());
    }

    let status = std::process::Command::new(&hook_path)
        .env("SECALL_SESSION_ID", &session.id)
        .env("SECALL_AGENT", session.agent.as_str())
        .env("SECALL_PROJECT", session.project.as_deref().unwrap_or(""))
        .env("SECALL_VAULT_PATH", vault_path)
        .env("SECALL_TURNS", session.turns.len().to_string())
        .env("SECALL_DATE", session.start_time.format("%Y-%m-%d").to_string())
        .status()?;

    if !status.success() {
        eprintln!("⚠ post_ingest hook exited with: {}", status);
    }
    Ok(())
}
```

### 3. 세션 종료 자동 인덱싱 가이드

seCall 코드에 포함하지 않지만, 문서로 제공:

**Claude Code hook** (`~/.claude/settings.json`):
```json
{
  "hooks": {
    "PostToolUse": [{
      "matcher": "Exit",
      "hooks": [{"type": "command", "command": "secall ingest --auto --cwd $PWD"}]
    }]
  }
}
```

**Zsh precmd hook** (`~/.zshrc`):
```bash
# 세션 종료 후 자동 ingest (마지막 명령이 claude/codex 였으면)
secall_auto_ingest() {
  local last_cmd=$(fc -ln -1)
  if [[ "$last_cmd" == claude* ]] || [[ "$last_cmd" == codex* ]]; then
    secall ingest --auto --cwd "$PWD" 2>/dev/null &
  fi
}
precmd_functions+=(secall_auto_ingest)
```

이 가이드를 `secall init` 완료 시 출력.

## Dependencies

- Task 09 (CLI ingest 커맨드)
- 추가 crate 없음

## Verification

```bash
# 빌드 성공
cd /Users/d9ng/privateProject/seCall && cargo build 2>&1 | tail -3

# JSON 이벤트 출력 확인
cargo run -- ingest --auto --format json 2>/dev/null | python3 -c "
import sys, json
for line in sys.stdin:
    event = json.loads(line)
    assert event['event'] == 'ingest_complete'
    assert 'session_id' in event
    print('✅ Event format valid')
    break
"

# Hook 실행 확인
mkdir -p /tmp/secall-hook-test
cat > /tmp/secall-hook-test/hook.sh << 'HOOK'
#!/bin/bash
echo "HOOK: $SECALL_SESSION_ID $SECALL_PROJECT" > /tmp/secall-hook-test/result.txt
HOOK
chmod +x /tmp/secall-hook-test/hook.sh

# config.toml에 hook 경로 설정 후 ingest 실행
# → /tmp/secall-hook-test/result.txt에 세션 정보 기록 확인
cat /tmp/secall-hook-test/result.txt

# Hook 미설정 시 에러 없이 통과
cargo test -p secall-core -- hooks::tests --nocapture
```

## Risks

- **Hook 스크립트 보안**: 사용자가 설정한 스크립트를 실행하므로 보안 문제는 사용자 책임. seCall은 스크립트 내용을 검사하지 않음. 권한 체크(실행 가능)만 수행.
- **Hook 타임아웃**: 장시간 실행되는 hook이 ingest를 블록. 기본 30초 타임아웃 설정 (`config.toml`의 `hook_timeout_secs`).
- **`--auto` 모드에서 hook이 세션마다 실행**: 10개 세션 한꺼번에 ingest하면 hook도 10번 실행. 배치 모드에서는 마지막에 한 번만 실행하는 옵션 제공 (`--batch-hook`).

## Scope Boundary

- Hook 스크립트 자체 작성은 사용자 책임
- Wiki 업데이트 로직은 seCall에 포함하지 않음
- Claude Code의 PostToolUse hook 자동 설정은 하지 않음 (가이드만 제공)
