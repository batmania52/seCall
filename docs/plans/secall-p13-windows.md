---
type: plan
status: draft
updated_at: 2026-04-09
plan: secall-p13-windows
version: 1
---

# seCall P13 — Windows 빌드 지원

## Description

`x86_64-pc-windows-msvc` 타겟을 CI/CD에 추가하여 Windows에서 seCall을 빌드·배포한다. 네이티브 C/C++ 의존성(ort, tokenizers+onig, kiwi-rs, usearch, lindera)의 MSVC 컴파일 이슈를 해결하고, ORT DLL을 번들링하여 Windows 바이너리를 GitHub Release에 포함한다.

## Background

- 순수 Rust 코드(파싱, DB, vault, 검색 로직)는 이미 Windows 호환 — 경로 처리에 `\\` 분기문 존재
- 문제는 네이티브 의존성의 MSVC 빌드 + ORT 런타임 DLL
- P8에서 Windows를 명시적으로 non-goal로 제외했으나, 사용자 요청으로 지원 추가

## Expected Outcome

- `cargo build --release --target x86_64-pc-windows-msvc` CI에서 성공
- `cargo test` Windows에서 전체 통과
- GitHub Release에 `secall-x86_64-pc-windows-msvc.zip` (secall.exe + onnxruntime.dll) 포함
- 기존 macOS 빌드·테스트에 regression 없음

## Subtasks

| # | 제목 | depends_on | parallel_group |
|---|------|------------|----------------|
| 1 | CI에 Windows 빌드 추가 + 깨지는 것 확인 | — | — |
| 2 | 네이티브 의존성 컴파일 이슈 수정 | 1 | — |
| 3 | Release 워크플로우에 Windows 바이너리 추가 | 2 | — |

## Constraints

- Task 2는 Task 1의 CI 결과에 의존 — 실제로 깨지는 것만 수정
- 기존 macOS/Linux 빌드에 영향 없어야 함
- `#[cfg(target_os)]` 분기는 최소화 — 가능하면 크로스플랫폼으로 통일

## Non-goals

- ARM Windows (aarch64-pc-windows-msvc) — x86_64만 우선
- Windows installer (MSI/NSIS) — ZIP 배포
- Windows-specific UX (PowerShell 자동완성 등)
- Linux 빌드 추가 (별도 플랜)

## Known Limitations (Windows)

### usearch — HNSW ANN 인덱스 비활성

- **원인**: `usearch` v2.24.0 Rust crate의 `index_plugins.hpp:962`에서 POSIX 전용 `MAP_FAILED` 상수 사용. MSVC에는 `mmap`이 없어 컴파일 실패.
- **upstream 상태**: C++ 본체(`page_allocator_t`)에는 `VirtualAlloc`/`VirtualFree` Windows 분기가 있으나, 문제의 `memory_mapping_allocator_gt` 클래스에서 `MAP_FAILED`를 직접 참조하여 불완전.
- **현재 대응**: `cfg(not(target_os = "windows"))`로 조건부 제외. Windows에서는 BLOB 코사인 스캔 fallback (10만 청크 이하 체감 차이 없음).
- **해결 방안**:
  1. usearch crate 포크 → `MAP_FAILED` 부분 `#ifdef` 패치 → `Cargo.toml` git 의존성으로 교체
  2. upstream(unum-cloud/usearch)에 PR 제출 → 다음 릴리스 반영 대기
  3. 순수 Rust ANN 라이브러리(`hnsw_rs`, `instant-distance`)로 교체

### kiwi-rs — 한국어 형태소 분석기 비활성

- **원인**: C++ 래핑 crate. MSVC 빌드 미확인 — 실제로 깨지는지 테스트하지 않고 선제적으로 제외한 상태.
- **현재 대응**: `cfg(not(target_os = "windows"))`로 조건부 제외. Windows에서는 Lindera ko-dic fallback.
- **해결 방안**: CI에서 `cfg` 분기를 제거하고 빌드해보면 MSVC에서 동작할 수도 있음. 실패 시 포크+패치.
