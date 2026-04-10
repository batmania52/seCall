---
type: task
status: draft
task_number: 3
plan: secall-p17-cli-git-branch
title: 대화형 온보딩 (secall init 개선)
depends_on: [1, 2]
parallel_group: C
updated_at: 2026-04-10
---

# Task 03 — 대화형 온보딩 (`secall init` 개선)

## Changed files

1. `crates/secall/Cargo.toml:9` — `dialoguer = "0.11"` 의존성 추가
2. `crates/secall/src/commands/init.rs` — 전면 재작성. 대화형 위저드 + non-interactive 분기
3. `crates/secall/src/main.rs:23-30` — `Commands::Init`에 `--non-interactive` 플래그 추가 (기존 --vault, --git 유지)

## Change description

### Step 1: dialoguer 의존성 추가

`crates/secall/Cargo.toml`에:

```toml
dialoguer = "0.11"
```

### Step 2: main.rs Init variant 확장

```rust
Init {
    /// Vault path (non-interactive mode)
    #[arg(short, long)]
    vault: Option<PathBuf>,
    /// Git remote URL (non-interactive mode)
    #[arg(long)]
    git: Option<String>,
},
```

기존 인자 그대로 유지. `vault` 또는 `git` 인자가 전달되면 non-interactive 모드로 동작 (기존 로직).
인자 없이 `secall init`만 실행하면 대화형 모드 진입.

### Step 3: init.rs 대화형 위저드 구현

```rust
pub fn run(vault: Option<PathBuf>, git: Option<String>) -> Result<()> {
    if vault.is_some() || git.is_some() {
        return run_non_interactive(vault, git);  // 기존 로직
    }
    run_interactive()
}
```

**대화형 흐름**:

```
$ secall init

  seCall — Agent Session Search Engine
  =====================================

  Step 1/6: Vault 경로
  Obsidian vault 경로를 입력하세요
  > [~/obsidian-vault/seCall]  ← Input with default

  Step 2/6: Git 동기화 (선택)
  멀티 기기 동기화를 위한 Git remote URL
  없으면 Enter를 누르세요
  > []  ← Input, empty = skip

  Step 3/6: Git 브랜치
  (git remote 설정한 경우만 표시)
  > [main]  ← Input with default

  Step 4/6: 토크나이저
  ❯ lindera — 한국어+일본어 형태소 분석 (범용)
    kiwi — 한국어 전용, 더 정확 (macOS/Linux만 지원)
  ← Select (Windows에서는 kiwi 비표시)

  Step 5/6: 임베딩 백엔드
  ❯ ollama — 로컬 임베딩 (bge-m3, 무료)
    none — 벡터 검색 비활성화 (BM25만 사용)
  ← Select

  Step 6/6: Ollama 설정
  (ollama 선택 시만 표시)
  ⟳ Ollama 설치 확인 중...
  ✓ Ollama가 설치되어 있습니다.
  ⟳ bge-m3 모델 확인 중...
  ⟳ ollama pull bge-m3 실행 중... (최초 ~1.5GB 다운로드)
  ✓ bge-m3 모델 준비 완료.

  --- 또는 미설치 시 ---

  ✗ Ollama가 설치되어 있지 않습니다.
    설치 방법:
    macOS:   brew install ollama
    Linux:   curl -fsSL https://ollama.com/install.sh | sh
    Windows: https://ollama.com/download

    설치 후 다음 명령을 실행하세요:
      ollama serve          # 서버 시작
      ollama pull bge-m3    # 임베딩 모델 다운로드

    Ollama 없이도 BM25 검색은 사용 가능합니다.
    나중에 `secall config set embedding.backend ollama`로 변경할 수 있습니다.
```

### Step 4: Ollama 확인 + 모델 pull 로직

```rust
fn check_and_setup_ollama() -> Result<()> {
    // 1. ollama 명령어 존재 확인 (command_exists 공통 함수 사용)
    if !command_exists("ollama") {
        // 설치 안내 출력
        return Ok(());
    }
    println!("  ✓ Ollama가 설치되어 있습니다.");

    // 2. ollama 서버 동작 확인
    // ollama list 실행 — 서버 미실행이면 안내
    let output = Command::new("ollama").args(["list"]).output();
    if output.is_err() || !output.unwrap().status.success() {
        println!("  ⚠ Ollama 서버가 실행 중이 아닙니다.");
        println!("    `ollama serve`로 서버를 시작한 후 다시 시도하세요.");
        return Ok(());
    }

    // 3. bge-m3 모델 존재 확인
    let list_output = Command::new("ollama").args(["list"]).output()?;
    let models = String::from_utf8_lossy(&list_output.stdout);
    if models.contains("bge-m3") {
        println!("  ✓ bge-m3 모델이 이미 설치되어 있습니다.");
        return Ok(());
    }

    // 4. 모델 pull
    println!("  ⟳ ollama pull bge-m3 실행 중... (최초 ~1.5GB 다운로드)");
    let status = Command::new("ollama")
        .args(["pull", "bge-m3"])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if status.success() {
        println!("  ✓ bge-m3 모델 준비 완료.");
    } else {
        println!("  ⚠ 모델 다운로드 실패. 나중에 `ollama pull bge-m3`로 재시도하세요.");
    }
    Ok(())
}
```

### Step 5: command_exists 공통 함수

`crates/secall-core/src/vault/mod.rs` 또는 적절한 위치에 이미 `command_exists` 함수가 있는지 확인.
기존 `sync.rs`에서 사용 중인 함수를 재사용하거나, 없으면 init.rs 로컬에 정의.

### Step 6: 설정 저장 + 초기화

위저드 완료 후:
1. 선택 값으로 Config 구성
2. `config.save()` — config.toml 생성
3. `Vault::init()` — vault 디렉토리 구조 생성
4. `Database::open()` — DB 초기화
5. git remote 설정했으면 `VaultGit::init()` 실행
6. 완료 메시지 + 다음 단계 안내 (기존 init.rs의 hooks/MCP 설정 안내 유지)

## Dependencies

- Task 01 (VaultConfig에 branch 필드 필요)
- Task 02 (config 서브커맨드 존재해야 안내 메시지에서 참조 가능)
- crate: `dialoguer = "0.11"` 추가 필요

## Verification

```bash
# 1. 타입 체크
cargo check 2>&1 | tail -3

# 2. non-interactive 모드 (기존 동작 유지)
cargo run -p secall -- init --vault /tmp/test-secall-init 2>&1
# 출력에 "Initializing seCall..." 포함

# 3. 대화형 모드는 CI에서 테스트 불가 — Manual 확인
# Manual: `cargo run -p secall -- init` 실행 → 6단계 프롬프트 확인
# Manual: 각 단계에서 기본값 Enter → 정상 완료 확인
# Manual: vault.path에 ~/ 입력 → 경로 확장 확인

# 4. Ollama 확인 로직 (ollama 설치된 환경)
# Manual: init 중 Step 6에서 Ollama 감지 → bge-m3 pull 실행 확인

# 5. Windows에서 kiwi 비표시 확인
# Manual: Windows 빌드에서 토크나이저 선택지에 kiwi 없음 확인

# 6. 전체 테스트
cargo test 2>&1 | tail -10

# 7. init 완료 후 config show로 저장 확인
cargo run -p secall -- config show 2>&1
```

## Risks

- **dialoguer stdin 문제**: CI 환경에서 stdin 없으면 panic 가능. non-interactive 분기로 보호됨
- **Ollama pull 시간**: bge-m3는 ~1.5GB, 네트워크 느리면 오래 걸림. stdout inherit로 진행률 표시
- **shellexpand**: vault 경로에 `~/` 입력 시 확장 필요. `secall` crate에 이미 `shellexpand = "3"` 있음
- **기존 config.toml 덮어쓰기**: 이미 config가 있으면 `load_or_default()` + 사용자 선택으로 merge. 기존 값을 기본값으로 제시

## Scope boundary — 수정 금지 파일

- `crates/secall-core/src/vault/config.rs` — Task 01 영역 (Config 구조체 수정 금지)
- `crates/secall-core/src/vault/git.rs` — Task 01 영역
- `crates/secall/src/commands/config.rs` — Task 02 영역
- `crates/secall-core/src/store/**` — DB 영역
- `crates/secall-core/src/search/**` — 검색 엔진
