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

## 3. Calibration Commit 패널 오류 수정 (Write-back vs Calibration 혼동)

### 발견된 문제

MU 상세 페이지의 "Write & Commit Substation Config" 패널에 **SDD와 불일치하는 텍스트 5건**이 존재했다.

### 오류 분석

| 기존 UI 텍스트 | SDD 근거 | 판정 |
|---|---|---|
| "Write Configuration to **Device**" | SDD §2.2: "The MU hardware and its analog front-end calibration" → **Out of Scope** | ❌ SVDC는 물리 MU에 쓰지 않음 |
| "lock discipline recalculation" | SDD/IP 전체 검색 결과 해당 용어 **미존재** | ❌ 조작된 용어 |
| "audited under QSE write-back controls" | QSE write-back(M8, FR-6)은 **손상된 샘플을 CB에서 추정값으로 대체**하는 기능으로 캘리브레이션(M4)과 **완전히 별개** | ❌ 두 개념 혼합 |
| "Uploading config via PRP" | PRP는 Ethernet 프레임 이중화 프로토콜 | ❌ 설정 전송 경로 아님 |
| Toast: "saved to MU-XX" | 물리 MU에 저장하는 것이 아님 | ❌ 부정확 |

### SDD 기반 정확한 동작 설명

**캘리브레이션 업데이트** (SDD §6 M4 + §8.4):

> "Applies the per-channel calibration triple `(scale, offset, φ)` derived from the procedures of [1, §10].
> Calibration factors are updated atomically through the management interface;
> the application path reads them via a versioned snapshot."

- API: `POST /calibration/{channel_id}` (SDD §8.4)
- 메커니즘: Copy-on-write — 새 포인터를 release ordering으로 발행, 리더는 old 또는 new triple만 보고 torn read 없음 (IP WBS-4.4)
- 대상: **SVDC 내부 캘리브레이션 테이블** (물리 MU 하드웨어가 아님)

**QSE Write-back** (SDD §6 M8, FR-6) — 별개 기능:

> "Accepts overwrite requests of the form `(channel_id, tick_id, estimated_value, qse_quality)`.
> The target record's original value is copied to the audit log,
> the slot is updated in place in both CB-A and CB-B,
> and the record flag is set to `QSE_CORRECTED`."

- API: `svdc_writeback(const QseCorrection* batch, size_t n)` (C ABI)
- QSE 알고리즘 자체는 SVDC 범위 밖 (SDD §2.2: "The QSE algorithm itself")
- Audit log: QSE write-back에만 존재, 캘리브레이션 변경에는 없음

### 조치 사항 (완료 ✅)

UI 텍스트를 SDD에 맞게 수정:
- **제목**: "Write & Commit Substation Config" → "Apply Calibration to SVDC Engine"
- **설명**: per-channel calibration triple의 atomic copy-on-write 업데이트임을 명시
- **API 참조**: `POST /calibration/{channel_id}` 명시
- **범위 명시**: "This operation targets the SVDC internal calibration table, not the physical Merging Unit hardware."
- **버튼**: "Write Configuration to Device" → "Apply Calibration to Engine"
- **Progress**: "Uploading config via PRP..." → "Committing calibration triple (copy-on-write)..."
- **Toast**: calibration triple이 channel pipeline에 적용되었음을 표시

---

## 4. 전체 Southbound Gap 요약

| 항목 | 현재 상태 | 결정 |
|---|---|---|
| DC Offset (Configuration 전역) | ✅ 구현됨 | 현재 위치 유지 (올바름) |
| DC Offset (MU 개별) | ✅ 추가 완료 | MU 캘리브레이션 3-tuple |
| CT/PT Ratio | ✅ 추가 완료 | Section 3 Instrument Transformer |
| Polarity | ✅ 추가 완료 | Normal(+1) / Inverted(-1) |
| Mag + Angle 보정 | ✅ 구현됨 | 유지 |
| IEC 61850 파라미터 | ✅ 구현됨 | 유지 |
| Ethernet/VLAN/PRP | ✅ 구현됨 | 유지 |
| Waveform 6채널 표시 | ✅ 추가 완료 | V+I 각 3상, 체크박스 토글 |
| Calibration Commit 텍스트 | ✅ 수정 완료 | SDD §6 M4 + §8.4 기준으로 교정 |
