---
type: task
plan: secall-mvp
task_number: 4
title: Markdown 렌더러
status: draft
parallel_group: 1
depends_on: [3]
updated_at: 2026-04-05
---

# Task 04: Markdown 렌더러

## Changed Files

- `crates/secall-core/src/ingest/mod.rs` — `pub mod markdown;` 추가
- `crates/secall-core/src/ingest/markdown.rs` — **신규**. Session → Obsidian MD 렌더링

## Change Description

### 1. 렌더링 함수

```rust
/// Session 구조체를 Obsidian-compatible 마크다운 문자열로 변환
pub fn render_session(session: &Session) -> String;

/// 마크다운 파일의 vault 내 상대 경로 생성
pub fn session_vault_path(session: &Session) -> PathBuf;
// → "raw/sessions/2026-04-05/claude-code_seCall_a1b2c3d4.md"
```

### 2. 출력 포맷

```markdown
---
type: session
agent: claude-code
model: claude-opus-4-6
project: seCall
cwd: /Users/d9ng/privateProject/seCall
session_id: a1b2c3d4-e5f6-7890-abcd-ef1234567890
date: 2026-04-05
start_time: "2026-04-05T14:30:00+09:00"
end_time: "2026-04-05T15:45:00+09:00"
turns: 23
tokens_in: 45000
tokens_out: 12000
tools_used: [Bash, Read, Edit, Grep]
status: raw
---

# claude-code 세션: seCall

> **프로젝트**: seCall | **브랜치**: main | **시간**: 14:30–15:45 (1h 15m)

## Turn 1 — User (14:30)

seCall의 컴포넌트 아키텍처를 어떻게 잡을 것인가?

## Turn 2 — Assistant (14:31)

권장 구조: 단일 Rust 바이너리, 내부 모듈 분리...

> [!tool]- Bash
> ```
> ls -la /Users/d9ng/privateProject/seCall/
> ```
> **Output:**
> ```
> drwxr-xr-x@ - d9ng ...
> ```

> [!tool]- Read `/Users/d9ng/privateProject/seCall/CLAUDE.md`
> *(1.2k chars)*

## Turn 3 — User (14:35)

qmd 코드베이스 직접 읽고 참고해줘.

## Turn 4 — Assistant (14:36)

> [!thinking]- Thinking
> Let me explore the qmd repository...

qmd 코드베이스를 분석하겠습니다...
```

### 3. 렌더링 규칙

**Frontmatter**:
- YAML 형식, `---` 구분
- `tools_used`: 세션 전체에서 사용된 도구 이름 중복 제거 목록
- `tags`: MVP에서는 빈 배열 (향후 자동 태깅)
- `status`: 항상 `"raw"` (에이전트가 위키에 통합하면 변경)

**턴 헤딩**:
- `## Turn {N} — {Role} ({HH:MM})` 형식
- Role: User, Assistant, System
- 타임스탬프가 없으면 시간 생략

**도구 호출 (Obsidian callout)**:
- `> [!tool]- {ToolName}` — 접힌 상태 (Obsidian에서 클릭하여 펼침)
- 도구 입력 요약 (명령어, 파일 경로 등)
- 도구 출력 요약 (기본 500자 제한, `...` truncation)
- `Read`, `Glob`, `Grep` 등 조회 도구는 한 줄 요약
- `Bash`, `Edit`, `Write` 등 변경 도구는 입출력 모두 표시

**Thinking block**:
- `> [!thinking]- Thinking` — 접힌 상태
- 내용 전체 포함 (thinking은 에이전트 추론 과정이므로 검색 가치 있음)

**Content 정리**:
- assistant 텍스트에서 tool_use JSON 블록 제거 (이미 callout으로 분리)
- 연속 빈 줄 → 단일 빈 줄
- 코드 블록 내부는 그대로 유지

### 4. 파일명 생성 규칙

```rust
fn session_filename(session: &Session) -> String {
    let agent = match session.agent {
        AgentKind::ClaudeCode => "claude-code",
        AgentKind::Codex => "codex",
        AgentKind::GeminiCli => "gemini-cli",
    };
    let project = session.project.as_deref().unwrap_or("unknown");
    let id_prefix = &session.id[..8]; // UUID 앞 8자
    format!("{agent}_{project}_{id_prefix}.md")
}
// → "claude-code_seCall_a1b2c3d4.md"
```

## Dependencies

- Task 03 (Session, Turn, Action 타입)
- `chrono` (시간 포맷팅)

## Verification

```bash
# 빌드 성공
cd /Users/d9ng/privateProject/seCall && cargo build 2>&1 | tail -3

# 유닛 테스트
cargo test -p secall-core -- ingest::markdown::tests --nocapture

# 테스트 항목:
# 1. 기본 세션 렌더링 (frontmatter + 턴 구조)
# 2. 도구 호출이 callout으로 렌더링
# 3. thinking block이 접힌 callout으로 렌더링
# 4. 빈 세션 (턴 0개) 처리
# 5. session_vault_path 경로 생성 정확성
# 6. 긴 tool output이 500자로 truncation
# 7. frontmatter YAML이 유효한지 확인
```

## Risks

- **Obsidian callout 구문 호환성**: `> [!type]- title` 형식은 Obsidian 1.1+ 필요. 구 버전 사용자는 raw 텍스트로 보임. Obsidian 최소 버전을 문서에 명시.
- **대규모 세션의 마크다운 크기**: 100턴 세션 → 수백 KB 마크다운. Obsidian에서 열기 느릴 수 있으나 기능적 문제는 없음.
- **tool output 내 마크다운 구문 충돌**: tool output에 `---`나 ` ``` `가 포함되면 마크다운 구조 깨짐. 코드 블록 내부에 넣어서 방지. 중첩 코드 펜스 시 `````(4+ backtick)` 사용.

## Scope Boundary

- vault에 실제 파일 쓰기는 Task 05
- 이 태스크는 `String` 반환까지만. 파일 I/O 없음
