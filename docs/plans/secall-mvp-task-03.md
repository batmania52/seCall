---
type: task
plan: secall-mvp
task_number: 3
title: Claude Code JSONL 파서
status: draft
parallel_group: 1
depends_on: [1]
updated_at: 2026-04-05
---

# Task 03: Claude Code JSONL 파서

## Changed Files

- `crates/secall-core/src/lib.rs` — `pub mod ingest;` 추가
- `crates/secall-core/src/ingest/mod.rs` — **신규**. Ingest 모듈, `SessionParser` trait 정의
- `crates/secall-core/src/ingest/types.rs` — **신규**. `Session`, `Turn`, `Action`, `AgentKind` 등 공통 타입
- `crates/secall-core/src/ingest/claude.rs` — **신규**. `ClaudeCodeParser` 구현
- `crates/secall-core/src/ingest/detect.rs` — **신규**. 자동 파서 감지

## Change Description

### 1. 공통 타입 (ingest/types.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentKind {
    ClaudeCode,
    Codex,
    GeminiCli,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub agent: AgentKind,
    pub model: Option<String>,
    pub project: Option<String>,
    pub cwd: Option<PathBuf>,
    pub git_branch: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub turns: Vec<Turn>,
    pub total_tokens: TokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub index: u32,
    pub role: Role,
    pub timestamp: Option<DateTime<Utc>>,
    pub content: String,
    pub actions: Vec<Action>,
    pub tokens: Option<TokenUsage>,
    pub thinking: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Role { User, Assistant, System }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    ToolUse {
        name: String,
        input_summary: String,   // 전체 input이 아닌 요약
        output_summary: String,
    },
    FileEdit {
        path: String,
    },
    Command {
        cmd: String,
        exit_code: Option<i32>,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub cached: u64,
}
```

### 2. SessionParser trait (ingest/mod.rs)

```rust
pub trait SessionParser: Send + Sync {
    /// 이 파서가 주어진 경로를 처리할 수 있는지 확인
    fn can_parse(&self, path: &Path) -> bool;

    /// 세션 파일을 파싱하여 Session 구조체 반환
    fn parse(&self, path: &Path) -> Result<Session>;

    /// 파서가 지원하는 에이전트 종류
    fn agent_kind(&self) -> AgentKind;
}
```

### 3. Claude Code 파서 (ingest/claude.rs)

**입력**: `~/.claude/projects/<encoded-cwd>/<session-uuid>.jsonl`

**JSONL 라인 구조** (이 프로젝트의 실제 세션 파일에서 검증):

라인 타입 분포 (실측): `queue-operation: 12, user: 55, assistant: 76, attachment: 2, last-prompt: 5`

```json
// type: "user" — 사용자 메시지
// 공통 필드: parentUuid, isSidechain, uuid, timestamp, sessionId, cwd, version, gitBranch, userType, entrypoint
// content가 문자열일 수 있음 (배열이 아닌 경우 주의!)
{"type":"user","message":{"role":"user","content":"텍스트..."},"timestamp":"ISO8601","sessionId":"...","cwd":"...","version":"...","gitBranch":"main","permissionMode":"...","promptId":"..."}

// type: "assistant" — 어시스턴트 응답
// content는 항상 배열: [{type: "thinking"}, {type: "text"}, {type: "tool_use"}]
{"type":"assistant","message":{"role":"assistant","id":"msg_...","model":"claude-opus-4-6","content":[{"type":"thinking","thinking":"..."},{"type":"text","text":"..."},{"type":"tool_use","id":"toolu_...","name":"Bash","input":{"command":"ls","description":"..."}}],"usage":{"input_tokens":3,"cache_creation_input_tokens":10388,"cache_read_input_tokens":11108,"output_tokens":28,"service_tier":"standard"}},"timestamp":"..."}

// type: "user" (tool_result) — 도구 실행 결과
// content는 배열: [{type: "tool_result", tool_use_id, content, is_error}]
{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_...","type":"tool_result","content":"실행결과...","is_error":false}]},"timestamp":"..."}

// type: "queue-operation" — 큐 관리 (무시 가능)
{"type":"queue-operation","operation":"enqueue","timestamp":"...","sessionId":"...","content":"..."}

// type: "attachment" — 시스템 첨부 (deferred_tools_delta, mcp_instructions_delta 등)
{"type":"attachment","attachment":{"type":"deferred_tools_delta","delta":[...]},...}

// type: "last-prompt" — 마지막 프롬프트 (무시 가능)
{"type":"last-prompt","promptId":"...","timestamp":"..."}
```

**파싱 전략**:

1. **스트리밍 읽기**: `BufReader::lines()` + 라인별 `serde_json::from_str::<Value>`
2. **타입별 분기**: `value["type"].as_str()` → 5가지 분기
   - `"user"` + content에 text 있음 → 새 User 턴 시작
   - `"user"` + content에 `tool_result` 있음 → 이전 Assistant 턴에 Action 결과 부속
   - `"assistant"` → Assistant 턴 (content 배열 순회하여 text/tool_use/thinking 분리)
   - `"queue-operation"`, `"attachment"`, `"last-prompt"` → **skip** (검색 가치 없음)
3. **content 타입 분기**: `value["message"]["content"]`
   - **문자열인 경우** (`is_string()`) → 텍스트 그대로 (user 메시지에서 발생)
   - **배열인 경우** (`is_array()`) → 각 요소의 `type` 필드로 분기:
     - `"text"` → `text` 필드 추출
     - `"tool_use"` → `Action::ToolUse { name, input_summary }` (input은 JSON → 요약)
     - `"thinking"` → `Turn.thinking` 에 append
     - `"tool_result"` → 이전 `tool_use` Action에 output 매칭 (by `tool_use_id`)
4. **usage 집계**: `message.usage.input_tokens` + `output_tokens`. `cache_read_input_tokens`는 cached에 합산.
5. **메타데이터**: 첫 `user` 라인에서 `sessionId`, `cwd`, `version`, `gitBranch` 추출. 첫 `assistant`에서 `model` 추출.

**에러 처리**:
- JSON 파싱 실패 라인 → `eprintln!` warn + skip (전체 파싱은 계속)
- 빈 파일 → `Err("empty session file")`
- content 없는 라인 → skip
- `isSidechain: true` → MVP에서는 포함하되 Turn에 `is_sidechain` 플래그 추가

### 4. 자동 파서 감지 (ingest/detect.rs)

```rust
pub fn detect_parser(path: &Path) -> Result<Box<dyn SessionParser>> {
    // 1. 경로 패턴: .claude/ → ClaudeCode
    // 2. 경로 패턴: .codex/ → Codex (Task 12)
    // 3. 경로 패턴: .gemini/ → GeminiCli (Task 13)
    // 4. 첫 줄 JSON 스니핑: "sessionId" + "type":"user" → ClaudeCode
    // 5. 실패 → Err("unknown format")
}
```

### 5. 세션 경로 탐색 유틸

```rust
/// ~/.claude/projects/ 하위의 모든 세션 파일 경로 반환
pub fn find_claude_sessions(base: Option<&Path>) -> Result<Vec<PathBuf>> {
    let base = base.unwrap_or(&dirs::home_dir()?.join(".claude/projects"));
    // glob: base/**/*.jsonl
}

/// 특정 cwd에 대한 세션 파일 경로 반환
pub fn find_sessions_for_cwd(cwd: &Path) -> Result<Vec<PathBuf>> {
    let encoded = encode_cwd(cwd);  // /Users/d9ng/foo → -Users-d9ng-foo
    // ~/.claude/projects/{encoded}/*.jsonl
}
```

## Dependencies

- Task 01 (workspace)
- `serde`, `serde_json`, `chrono` (이미 workspace deps)
- `dirs` crate (홈 디렉토리 탐색)
- `glob` 또는 `walkdir` crate (세션 파일 탐색) — workspace dependency 추가

## Verification

```bash
# 빌드 성공
cd /Users/d9ng/privateProject/seCall && cargo build 2>&1 | tail -3

# 유닛 테스트: 파서
cargo test -p secall-core -- ingest::claude::tests --nocapture

# 테스트 항목:
# 1. 실제 Claude Code JSONL 샘플 파싱 (테스트 fixture)
# 2. user/assistant 턴 그룹핑 정확성
# 3. tool_use 액션 추출
# 4. thinking block 추출
# 5. 토큰 집계
# 6. 잘못된 라인 skip (에러 아닌 warn)
# 7. 빈 파일 → Err

# 실제 세션 파싱 테스트 (로컬 환경에 Claude Code 세션이 있다면)
cargo run -- ingest ~/.claude/projects/-Users-d9ng-privateProject-seCall/ 2>&1 | head -20
```

## Risks

- **Claude Code JSONL 포맷 비공식**: Anthropic이 공식 스키마를 제공하지 않음. 버전 업데이트 시 필드가 변경될 수 있음. `version` 필드 체크 + 알 수 없는 필드 무시(`#[serde(deny_unknown_fields)]` 사용하지 않음)로 대응.
- **대용량 세션 파일**: 긴 세션은 수천 줄, 수 MB. `BufReader` 스트리밍으로 메모리 효율적 처리. 전체 파일을 메모리에 올리지 않음.
- **tool_result가 매우 클 수 있음**: 파일 전체 내용이 tool_result에 들어가는 경우. `output_summary`로 잘라서 저장 (기본 1000자 제한, 설정 가능).
- **sidechain 메시지**: Claude Code의 `isSidechain: true` 메시지는 분기된 대화. MVP에서는 sidechain도 포함하되 별도 표시.

## Scope Boundary

- Codex 파서, Gemini 파서는 이 태스크에서 구현하지 않음 (Task 12, 13)
- Session을 DB에 저장하는 로직은 이 태스크에서 구현하지 않음 (Task 06, 07에서 인덱싱과 함께)
- Markdown 렌더링은 이 태스크에서 구현하지 않음 (Task 04)
