# DC Offset Calibration 위치 검토 및 Southbound Gap 분석

> **Review Date**: 2026-05-22
> **Basis**: CIGRE 2024 Paper (ID-10427), SDD v0.1, IP v0.2
> **Status**: Decision Record

---

## 1. DC Offset Calibration 위치 검토

### 질문

DC Offset 보정 파라미터가 현재 **Configuration** 메뉴(노드 전역)에 배치되어 있다.
이 위치가 올바른가, 아니면 MU 상세 페이지(per-MU/per-channel)로 이동해야 하는가?

### SDD 근거

**SDD §7.2 Channel Registry** — 캘리브레이션은 **채널별(per-channel)** 속성:

```
calib: {scale, offset, φ_ns}   // Per-channel calibration triple
```

**SDD §6 M4 Calibration** — 채널별 적용:

> "Per-channel calibration triple: (scale, offset, φ).
>  Scale: multiplicative (magnitude correction).
>  Offset: additive (DC offset correction).
>  φ: timing correction via fractional-sample re-interpolation."

### CIGRE Paper 근거

**Paper Section 5 & 10** — Calibration은 MU별로 수행됨:

- 각 MU의 계기용 변압기(CT/PT)는 고유한 오차 특성을 가진다
- 소프트웨어 기반 캘리브레이션은 **각 MU의 측정 오차를 개별적으로 보정**하는 것이 목적
- Figure 10-11: MU별 캘리브레이션 인터페이스 도시

### 결론

> [!IMPORTANT]
> **Configuration 메뉴의 DC Offset은 올바른 위치이다.**
>
> 현재 Configuration 페이지의 "Active Calibration Triple Parameters"는 **노드 레벨 전역 보정**
> (전체 SVDC 정렬 파이프라인에 적용되는 기본값)으로서 유효하다.
>
> 다만 SDD §7.2에 따르면 캘리브레이션은 **per-channel** 속성이므로,
> **MU 상세 페이지에도 DC Offset이 있어야 한다.**
>
> 즉, **두 곳 모두 존재하는 것이 맞다:**
> - **Configuration (전역)**: 노드 레벨 기본 보정값 (현재 구현됨 ✅)
> - **MU Detail (개별)**: MU/채널별 미세 보정값 (현재 MU 상세에는 Mag+Angle만 있고 DC Offset 누락 ⚠️)

### 조치 사항

MU 상세 페이지의 캘리브레이션 섹션에 DC Offset 입력 필드를 추가한다.
현재 `(Magnitude, Angle)` 2-tuple → `(DC Offset, Magnitude, Angle)` 3-tuple로 확장.

---

## 2. CT/PT Ratio 및 Polarity 추가

### SDD 근거

**SDD §7.2 Channel Registry**:

| Field | Type | Description |
|-------|------|-------------|
| `ct_pt_ratio` | f64 | Instrument transformer ratio |
| `polarity` | ±1 | Polarity convention |

이 두 필드는 채널 레지스트리의 필수 속성이지만 현재 MU 상세 페이지에 입력란이 없다.

### 조치 사항

MU 상세 페이지의 IEC 61850 또는 캘리브레이션 섹션에 다음을 추가한다:
- **CT Ratio** (전류 변성기 비율): 전류 채널용
- **PT Ratio** (전압 변성기 비율): 전압 채널용
- **Polarity** (극성): Normal (+1) / Inverted (-1) 토글

---

## 3. 전체 Southbound Gap 요약

| 항목 | 현재 상태 | 결정 |
|---|---|---|
| DC Offset (Configuration 전역) | ✅ 구현됨 | 현재 위치 유지 (올바름) |
| DC Offset (MU 개별) | ❌ 누락 | MU 캘리브레이션에 추가 |
| CT/PT Ratio | ❌ 누락 | MU 상세에 추가 |
| Polarity | ❌ 누락 | MU 상세에 추가 |
| Mag + Angle 보정 | ✅ 구현됨 | 유지 |
| IEC 61850 파라미터 | ✅ 구현됨 | 유지 |
| Ethernet/VLAN/PRP | ✅ 구현됨 | 유지 |
