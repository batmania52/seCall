---
type: task
status: draft
plan: secall-p6
task_number: 1
title: "ANN reserve 사전 할당"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 01: ANN reserve 사전 할당

## 문제

`secall sync` 또는 `secall ingest --auto` 실행 시 ANN 인덱스에 벡터를 추가할 때마다 usearch가 "Reserve capacity ahead of insertions!" 경고를 출력한다.
현재 세션 1개당 수십~수백 청크의 벡터가 추가되므로, 경고가 수백 줄 발생하여 터미널 출력을 오염시킨다.

### 근본 원인

`ann.rs:13-45` `open_or_create()`에서:
- **신규 생성 시** (line 36): `index.reserve(10_000)` 호출 → 정상
- **기존 로드 시** (lines 25-34): reserve 호출 없음 → 즉시 insert하면 capacity 부족 경고

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/search/ann.rs:25-34` | 수정 | loaded index에 reserve 호출 추가 |
| `crates/secall-core/src/search/ann.rs:48-52` | 수정 | `add()` 전 capacity 체크 로직 추가 |

## Change description

### Step 1: loaded index에 reserve 추가

`crates/secall-core/src/search/ann.rs` — `open_or_create()` (lines 25-34):

```rust
if path.exists() {
    let path_str = path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("non-UTF-8 ANN index path: {:?}", path))?;
    index.load(path_str).map_err(|e| anyhow::anyhow!("{e}"))?;

    // 로드 후 추가 삽입을 위한 여유 공간 사전 할당.
    // usearch는 capacity 없이 add()하면 경고를 출력한다.
    let current = index.size();
    let reserve_target = current + 10_000;
    index.reserve(reserve_target).map_err(|e| anyhow::anyhow!("{e}"))?;

    tracing::info!(
        path = %path.display(),
        vectors = current,
        capacity = reserve_target,
        "ANN index loaded"
    );
} else {
    // (기존 코드 유지)
    index.reserve(10_000).map_err(|e| anyhow::anyhow!("{e}"))?;
    tracing::info!(path = %path.display(), "ANN index created (empty)");
}
```

핵심: `index.size() + 10_000`으로 reserve. 기존 벡터 수 + 여유분.

### Step 2: add() 시 capacity 체크 (선택적 방어)

`crates/secall-core/src/search/ann.rs` — `add()` (lines 48-52):

```rust
pub fn add(&self, key: u64, vector: &[f32]) -> Result<()> {
    // capacity 부족 시 자동 reserve (방어적)
    if self.index.size() >= self.index.capacity() {
        let new_cap = self.index.capacity() + 10_000;
        self.index.reserve(new_cap).map_err(|e| anyhow::anyhow!("{e}"))?;
        tracing::debug!(new_capacity = new_cap, "ANN index auto-reserved");
    }
    self.index
        .add(key, vector)
        .map_err(|e| anyhow::anyhow!("{e}"))
}
```

> usearch의 `capacity()` API가 있는지 확인 필요. 없으면 `AnnIndex` struct에 `capacity: usize` 필드를 수동 관리.

### Step 3: usearch capacity API 확인

`usearch::Index`에 `capacity()` 메서드가 없으면 대안:

```rust
pub struct AnnIndex {
    index: Index,
    path: PathBuf,
    dimensions: usize,
    capacity: usize,  // 수동 추적
}
```

`reserve()` 호출 시 `self.capacity = reserve_target` 업데이트, `add()` 시 `self.index.size() >= self.capacity` 체크.

## Dependencies

- 없음 (기존 usearch crate 활용)
- Task 02, 03과 독립적으로 구현 가능

## Verification

```bash
# 1. 컴파일 확인
cargo check --all

# 2. ANN 테스트 통과
cargo test -p secall-core ann

# 3. 전체 테스트 통과
cargo test --all

# 4. sync 실행 시 "Reserve capacity" 경고 없음 확인
cargo run -p secall -- sync --local-only 2>&1 | grep -c "Reserve capacity" | xargs -I{} test {} -eq 0 && echo "OK: 0 warnings"

# 5. clippy 통과
cargo clippy --all-targets -- -D warnings
```

## Risks

- **usearch capacity() API 미존재**: usearch crate에 `capacity()` 메서드가 없을 수 있음. 이 경우 struct 필드로 수동 관리 (Step 3 대안).
- **reserve 크기 선택**: 10,000이 부족할 수 있음 (대규모 ingest 시). 하지만 Step 2의 auto-reserve가 방어.
- **메모리 사용량**: reserve가 메모리를 선점. 10,000 벡터 × 384차원 × 4바이트 ≈ 15MB. 무시 가능.

## Scope boundary

수정 금지 파일:
- `crates/secall-core/src/search/vector.rs` — ANN 호출부는 변경 없음, ann.rs 내부만 수정
- `crates/secall/src/commands/sync.rs` — Task 02 영역
- `crates/secall/src/commands/ingest.rs` — Task 03 영역
- `crates/secall-core/src/search/embedding.rs` — 임베딩 로직 변경 없음
