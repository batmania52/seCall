---
type: reference
status: draft
canonical: false
updated_at: 2026-04-16
---

# Idea: 2계층 LLM 파이프라인 (Wiki 생성 고도화)

## 배경

damoang.net 커뮤니티에서 seCall을 기반으로 자체 LLM Wiki 시스템을 구축한 사례가 공유됨.
핵심 아이디어: **저비용 LLM으로 대량 초안 생성 → 고품질 LLM으로 검수/병합**하는 2계층 파이프라인.

현재 seCall은 단일 LLM(claude -p 또는 Gemini Flash)으로 위키 생성+검수를 동시에 수행함.

## 아이디어

### Tier 1: 초안 생성 (Draft)

- 역할: 세션 본문 → 위키 초안 대량 생산
- 후보 모델: Gemini Flash, Gemma4 (로컬), Cerebras 호스팅 OSS (Llama 120B 등)
- 요구사항: 빠르고 저렴, 구조화된 마크다운 출력, 한국어 지원
- 현재 코드에서 `run_generate()` 단계에 해당

### Tier 2: 검수/병합 (Review & Merge)

- 역할: 초안 품질 검증, 중복 병합, 팩트 체크, 최종 위키 확정
- 후보 모델: Claude Sonnet/Opus, Gemini Pro 2.5
- 요구사항: 높은 추론 능력, 기존 위키와의 일관성 판단
- 현재 코드에서 `run_review()` 단계에 해당

### 파이프라인 흐름

```
세션 본문 → [Tier 1: Flash/Gemma4] → 초안 .md
                                         ↓
              기존 위키 목록 + 초안 → [Tier 2: Sonnet/Pro] → 최종 위키 .md
```

## 기대 효과

| 항목 | 현재 (단일 LLM) | 2계층 파이프라인 |
|------|------------------|------------------|
| 비용 | 전 과정 고비용 모델 | Tier 1은 무료/저비용 |
| 속도 | 세션당 10~30초 | Tier 1 대량 병렬 → Tier 2는 검수만 |
| 품질 | 모델 능력에 전적 의존 | Tier 1 실패해도 Tier 2에서 보정 |
| 확장성 | 백엔드 1개 고정 | Tier별 독립 교체 가능 |

## 구현 시 고려사항

1. **백엔드 추상화**: 현재 `WikiBackend` enum에 tier 개념 추가 또는 `--draft-backend` / `--review-backend` 분리
2. **중간 산출물 저장**: Tier 1 초안을 임시 저장할지, 메모리에서 바로 Tier 2로 넘길지
3. **실패 처리**: Tier 1 실패 시 Tier 2로 직접 생성 fallback vs. skip
4. **비용 모니터링**: Tier 2 호출 횟수 추적 (Tier 1이 좋으면 Tier 2 skip 가능)

## 레퍼런스

- damoang.net/ai/2363 — Gemma4 + Claude 2계층 wiki 구축기 (seCall 기반)
- 현재 코드: `crates/secall-core/src/wiki/pipeline.rs` (generate → lint → review 3단계)

## 우선순위

- **지금은 아이디어 단계** — tunaFlow 베타 오픈 후 seCall 업데이트 시 검토
- Gemini Pro 2.5 wiki 백엔드 완성이 선행 과제
