---
type: task
status: draft
plan: secall-wiki-claude-code
task_number: 1
title: "Wiki Vault 구조 초기화"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 01: Wiki Vault 구조 초기화

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/vault/init.rs` | 수정 | wiki/ 디렉토리 + SCHEMA.md + overview.md 생성 추가 |
| `crates/secall-core/src/vault/mod.rs` | 수정 (필요 시) | wiki 관련 헬퍼 함수 |

## Change description

### 1. secall init 확장

현재 `vault/init.rs`의 `init_vault()` 함수가 `raw/sessions/`, `index.md`, `log.md`를 생성.
여기에 wiki/ 디렉토리 구조를 추가:

```
vault_path/
├── raw/sessions/      (기존)
├── index.md           (기존)
├── log.md             (기존)
├── SCHEMA.md          (신규)
└── wiki/              (신규)
    ├── projects/
    ├── topics/
    ├── decisions/
    └── overview.md
```

### 2. SCHEMA.md 내용

메타에이전트(Claude Code)가 위키 페이지를 생성할 때 참고할 컨벤션 문서:

```markdown
# seCall Wiki Schema

## 페이지 구조

모든 wiki 페이지는 YAML frontmatter를 포함해야 합니다:

---
title: "페이지 제목"
type: project | topic | decision
created: YYYY-MM-DD
updated: YYYY-MM-DD
sources: ["session-id-1", "session-id-2"]
tags: ["tag1", "tag2"]
---

## 디렉토리 규칙

- `wiki/projects/` — 프로젝트별 페이지 (예: secall.md, tunaflow.md)
- `wiki/topics/` — 주제별 페이지 (예: rust-unsafe-patterns.md, korean-nlp.md)
- `wiki/decisions/` — 의사결정 기록 (예: 2026-04-05-embedder-trait.md)
- `wiki/overview.md` — 전체 위키 요약 + 페이지 목록

## 링크 규칙

- 세션 참조: `[[raw/sessions/YYYY-MM-DD_session-id]]`
- 위키 내부 링크: `[[wiki/topics/topic-name]]`
- sources 배열에 참조한 세션 ID를 반드시 포함

## 파일명 규칙

- kebab-case (예: rust-unsafe-patterns.md)
- decision은 날짜 prefix (예: 2026-04-05-embedder-trait.md)
```

### 3. overview.md 초기 내용

```markdown
---
title: "Wiki Overview"
type: overview
created: {today}
updated: {today}
---

# seCall Wiki

에이전트 세션에서 추출된 지식 위키입니다.

## 프로젝트
<!-- 메타에이전트가 자동 갱신 -->

## 주제
<!-- 메타에이전트가 자동 갱신 -->

## 최근 결정
<!-- 메타에이전트가 자동 갱신 -->
```

### 4. 멱등성

이미 wiki/ 디렉토리가 존재하면 건너뜀. 기존 파일을 덮어쓰지 않음.
`SCHEMA.md`도 이미 존재하면 건너뜀.

## Dependencies

- 없음 (Task 02와 병렬 실행 가능)

## Verification

```bash
# 타입 체크
cargo check -p secall-core

# vault 테스트
cargo test -p secall-core vault

# 전체 테스트 회귀
cargo test -p secall-core
```

테스트 작성 요구사항:
- `test_init_vault_creates_wiki_dirs`: wiki/, wiki/projects/, wiki/topics/, wiki/decisions/ 생성 확인
- `test_init_vault_creates_schema`: SCHEMA.md 존재 + 내용 검증
- `test_init_vault_creates_overview`: wiki/overview.md 존재
- `test_init_vault_idempotent_wiki`: 두 번 호출해도 기존 파일 보존

## Risks

- **기존 vault에 wiki/ 추가**: 이미 secall init을 실행한 사용자의 vault에 wiki/가 없음. 다음 init 시 추가되지만, init 없이 `secall wiki update`를 실행하면 디렉토리 미존재 오류 가능 → wiki CLI에서 자동 init 또는 에러 메시지

## Scope Boundary

수정 금지 파일:
- `vault/config.rs` — config 구조 변경 없음
- `ingest/*` — 파서 코드 변경 금지
- `search/*` — 검색 모듈 변경 금지
