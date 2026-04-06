# Review Report: 페르소나 고도화 + PersonaEditor 연결 — Round 1

> Verdict: fail
> Reviewer: 
> Date: 2026-04-05 17:45
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. pages/project/[id].vue:61 — `body.personas`를 `editingPreset.value`가 true일 때만 포함해서, 사용자가 PersonaEditor에서 값을 수정한 뒤 `접기`를 누르고 분석을 시작하면 수정된 personas가 서버로 전송되지 않습니다. Task 02의 요구사항인 "분석 요청에 수정된 personas 포함"을 만족하지 못합니다.

## Recommendations

1. components/analysis/PresetSelector.vue:43 — 편집 후 접은 상태에서는 미리보기가 여전히 `selectedPreset.personas`를 보여줘 실제 전송 대상과 달라질 수 있습니다. `customPersonas` 기반 미리보기 또는 "수정됨" 표시를 추가하면 혼동을 줄일 수 있습니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | 페르소나 프롬프트 강화 | ✅ done |
| 2 | PersonaEditor 연결 | ✅ done |
| 3 | 디버그 코드 제거 | ✅ done |

