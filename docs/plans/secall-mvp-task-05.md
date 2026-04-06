---
type: task
plan: secall-mvp
task_number: 5
title: Vault 구조 초기화 + index/log 관리
status: draft
parallel_group: 1
depends_on: [4]
updated_at: 2026-04-05
---

# Task 05: Vault 구조 초기화 + index/log 관리

## Changed Files

- `crates/secall-core/src/lib.rs` — `pub mod vault;` 추가
- `crates/secall-core/src/vault/mod.rs` — **신규**. Vault 관리 모듈
- `crates/secall-core/src/vault/init.rs` — **신규**. Vault 디렉토리 초기화
- `crates/secall-core/src/vault/index.rs` — **신규**. index.md 관리
- `crates/secall-core/src/vault/log.rs` — **신규**. log.md append
- `crates/secall-core/src/vault/config.rs` — **신규**. config.toml 읽기/쓰기

## Change Description

### 1. 설정 파일 (~/.config/secall/config.toml)

```toml
[vault]
path = "/Users/d9ng/obsidian-vault/seCall"  # Obsidian vault 경로

[ingest]
tool_output_max_chars = 500
thinking_included = true

[search]
default_limit = 10
```

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub vault: VaultConfig,
    pub ingest: IngestConfig,
    pub search: SearchConfig,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct VaultConfig {
    pub path: PathBuf,
}

impl Config {
    pub fn load() -> Result<Self>;              // ~/.config/secall/config.toml
    pub fn load_or_default() -> Self;           // 없으면 기본값
    pub fn config_path() -> PathBuf;            // 설정 파일 경로
}
```

설정 파일 위치: `~/.config/secall/config.toml`
환경변수 오버라이드: `SECALL_VAULT_PATH`

### 2. Vault 초기화 (vault/init.rs)

```rust
pub fn init_vault(vault_path: &Path) -> Result<()> {
    // 디렉토리 생성
    // vault_path/raw/sessions/
    // vault_path/wiki/projects/
    // vault_path/wiki/topics/
    // vault_path/wiki/decisions/

    // SCHEMA.md 생성 (없을 때만)
    // index.md 생성 (없을 때만)
    // log.md 생성 (없을 때만)
}
```

**SCHEMA.md 초기 내용**:
```markdown
---
type: schema
updated_at: 2026-04-05
---

# seCall Wiki Schema

## Directory Structure

- `raw/sessions/` — seCall이 생성한 세션 마크다운 (수정 금지)
- `wiki/` — 에이전트가 유지보수하는 위키 페이지

## Session Frontmatter Fields

| Field | Type | Description |
|---|---|---|
| type | string | 항상 "session" |
| agent | string | claude-code, codex, gemini-cli |
| session_id | string | UUID |
| date | string | YYYY-MM-DD |
| ... | ... | ... |

## Wiki Page Conventions

- 위키 페이지는 에이전트가 생성/수정
- 세션 참조: `[[raw/sessions/YYYY-MM-DD/filename]]`
- 태그: frontmatter의 tags 필드 사용
```

### 3. 세션 마크다운 저장 (vault/mod.rs)

```rust
pub struct Vault {
    path: PathBuf,
}

impl Vault {
    pub fn new(path: PathBuf) -> Self;
    pub fn init(&self) -> Result<()>;                          // init_vault 호출
    pub fn write_session(&self, session: &Session) -> Result<PathBuf>;
    // 1. render_session(session) 호출
    // 2. raw/sessions/YYYY-MM-DD/ 디렉토리 생성
    // 3. 마크다운 파일 쓰기
    // 4. update_index(session) 호출
    // 5. append_log(session) 호출
    // 6. 생성된 파일 경로 반환

    pub fn session_exists(&self, session_id: &str) -> bool;    // 중복 ingest 방지
}
```

### 4. index.md 관리 (vault/index.rs)

```rust
pub fn update_index(vault_path: &Path, session: &Session, md_path: &Path) -> Result<()> {
    // index.md 읽기
    // "## Sessions" 섹션 찾기 (없으면 생성)
    // 새 항목 추가 (최신 순, 상단에):
    //   - [[raw/sessions/2026-04-05/claude-code_seCall_a1b2c3d4|seCall 세션]] — 23턴, claude-code, 14:30
    // 파일 쓰기
}
```

항목 형식:
```markdown
## Sessions

- [[raw/sessions/2026-04-05/claude-code_seCall_a1b2c3d4|seCall 아키텍처 설계]] — 23턴, claude-code, 14:30–15:45
- [[raw/sessions/2026-04-04/claude-code_myapp_b2c3d4e5|myapp 버그 수정]] — 12턴, claude-code, 10:00–10:30
```

세션 제목: 첫 번째 User 턴의 첫 50자 (없으면 "Untitled Session")

### 5. log.md append (vault/log.rs)

```rust
pub fn append_log(vault_path: &Path, session: &Session, md_path: &Path) -> Result<()> {
    // log.md 열기 (append 모드)
    // 엔트리 추가:
    // ## [YYYY-MM-DD] ingest | {agent} {project} 세션
    // - session: {id_prefix}
    // - turns: {N}, tokens: {total}k
    // - file: {relative_path}
}
```

## Dependencies

- Task 04 (`render_session`, `session_vault_path`)
- Task 03 (`Session` 타입)
- `toml` crate — workspace dependency 추가
- `dirs` crate

## Verification

```bash
# 빌드 성공
cd /Users/d9ng/privateProject/seCall && cargo build 2>&1 | tail -3

# 유닛 테스트
cargo test -p secall-core -- vault::tests --nocapture

# 테스트 항목:
# 1. init_vault이 디렉토리 구조 생성
# 2. init_vault이 SCHEMA.md, index.md, log.md 생성
# 3. init_vault 재호출 시 기존 파일 덮어쓰지 않음
# 4. write_session이 올바른 경로에 MD 파일 생성
# 5. write_session이 index.md에 항목 추가
# 6. write_session이 log.md에 엔트리 append
# 7. session_exists가 중복 감지
# 8. Config::load_or_default가 기본값 반환

# 통합 테스트 (임시 디렉토리 사용)
cargo test -p secall-core -- vault::integration --nocapture
```

## Risks

- **index.md 동시 쓰기**: 여러 ingest가 동시 실행되면 index.md 손상 가능. 파일 잠금(`fs2` crate) 또는 단일 프로세스 보장 필요. MVP에서는 단일 프로세스 가정.
- **vault 경로 미설정**: `config.toml`이 없고 `SECALL_VAULT_PATH`도 없으면 에러. 첫 실행 시 `secall init` 또는 대화형 프롬프트로 vault 경로 설정 안내.
- **Obsidian 동시 접근**: seCall이 파일 쓰는 동안 Obsidian이 같은 파일 읽으면 부분 내용이 보일 수 있음. 임시 파일에 쓰고 rename하는 방식으로 원자적 쓰기.

## Scope Boundary

- wiki/ 디렉토리 내부 파일 생성은 하지 않음 (에이전트의 일)
- SCHEMA.md는 초기 템플릿만 생성. 에이전트가 이후 수정 가능
