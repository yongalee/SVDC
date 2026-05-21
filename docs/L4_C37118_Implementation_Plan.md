# L4 IEEE C37.118 Synchrophasor Adapter 구현 계획

## 배경

CIGRE 2024 논문 Section 8에서는 Local Node가 **IEEE C37.118/TCP-IP를 통해 phasor 데이터와 QSE 결과를 Master Node로 전송**해야 한다고 명시한다. 현재 SVDC Console의 Northbound 어댑터는 L0(SHM), L1(OPC UA), L2(MQTT), L3(TimescaleDB) 4개 계층만 구현되어 있으며, **논문에서 직접 명시한 C37.118 프로토콜은 전혀 구현되어 있지 않다.**

IEEE C37.118-2 (Synchrophasor Data Transfer)는 변전소 보호·제어 분야의 **사실상 표준(de facto standard)** 프로토콜로, Master Node(PDC)와의 통신에 필수적이다.

## 구현 범위

기존 L0~L3 어댑터와 **동일한 아키텍처 패턴**으로 **L4 C37.118 Synchrophasor** 어댑터를 추가한다.

---

## Proposed Changes

### Northbound Router

#### [MODIFY] [northbound.rs](file:///c:/Users/yonga/TestWork/SVDC/crates/svdc-console/src/routes/northbound.rs)

**1. 백엔드 상태 추가 (L4 Static Variables)**

기존 L3 상태 변수 블록 아래에 L4 C37.118 전용 상태 변수를 추가한다:

```rust
// L4 Settings — IEEE C37.118-2 Synchrophasor
static L4_ENABLED: AtomicBool = AtomicBool::new(false);
static L4_PDC_ADDRESS: OnceLock<Mutex<String>> = OnceLock::new();  // PDC 수신 주소
static L4_PMU_ID: Mutex<u32> = Mutex::new(1);                      // PMU Station ID (IDCODE)
static L4_DATA_RATE: Mutex<u32> = Mutex::new(60);                   // Reporting Rate (fps)
static L4_PHASOR_FORMAT: OnceLock<Mutex<String>> = OnceLock::new(); // Float/Integer
static L4_FREQ_FORMAT: OnceLock<Mutex<String>> = OnceLock::new();   // Float/Integer
static L4_NOMINAL_FREQ: Mutex<u32> = Mutex::new(60);               // 50Hz / 60Hz
static L4_NUM_PHASORS: Mutex<u32> = Mutex::new(6);                 // Va,Vb,Vc,Ia,Ib,Ic
```

초기화 함수:
```rust
fn get_l4_pdc_address() -> &'static Mutex<String> {
    L4_PDC_ADDRESS.get_or_init(|| Mutex::new("tcp://192.168.1.100:4712".to_string()))
}
fn get_l4_phasor_format() -> &'static Mutex<String> {
    L4_PHASOR_FORMAT.get_or_init(|| Mutex::new("Float".to_string()))
}
fn get_l4_freq_format() -> &'static Mutex<String> {
    L4_FREQ_FORMAT.get_or_init(|| Mutex::new("Float".to_string()))
}
```

**2. Form Deserializer 추가**

```rust
#[derive(Deserialize, Debug)]
pub struct L4Form {
    pdc_address: String,
    pmu_id: u32,
    data_rate: u32,
    phasor_format: String,
    freq_format: String,
    nominal_freq: u32,
    num_phasors: u32,
}
```

**3. Route 등록 추가**

```rust
pub fn register(router: Router) -> Router {
    router
        // ... 기존 L0~L3 routes ...
        .route("/api/v1/northbound/l4/save", post(save_l4_settings))
}
```

**4. Grid 목록에 L4 행 추가**

`northbound_page()` 함수의 `x_data_str` adapters 배열에 추가:

```javascript
{ id: 'L4', name: 'C37.118 Synchrophasor PMU', endpoint: '{l4_pdc}', consumers: 1, throughput: 60, active: {l4_active} }
```

**5. Detail Page 추가 (`adapter_detail_page` match 분기)**

`match layer_lower.as_str()` 에 `"l4"` 분기를 추가하고, 기존 어댑터와 동일한 패턴으로 구성:

- **Header Card**: "L4 IEEE C37.118-2 Synchrophasor PMU Stream"
- **Configuration Parameters Card**:

| 파라미터 | 입력 타입 | 설명 |
|---|---|---|
| PDC Destination Address | text | Master Node PDC의 TCP 수신 주소 (tcp://host:port) |
| PMU Station ID (IDCODE) | number | C37.118 프레임의 PMU 식별자 (1-65534) |
| Reporting Rate (fps) | select | 10 / 30 / 60 / 120 fps |
| Phasor Data Format | select | Float (IEEE 754) / Integer (16-bit scaled) |
| Frequency Data Format | select | Float / Integer |
| Nominal System Frequency | select | 50 Hz / 60 Hz |
| Number of Phasors | select | 6 (3V+3I) / 8 (3V+3I+V0+I0) / 12 (Full) |

- **Monitoring Card**: "C37.118 Data Frame Transmission Log"
  - 터미널 로그 형태로 CONFIG-2 프레임, DATA 프레임 전송 이벤트 표시

```
[2026-05-22 04:30:00] CONFIG-2 frame sent to PDC (IDCODE: 1, 6 phasors, 60 fps)
[2026-05-22 04:30:01] DATA frame #3600 transmitted (latency: 0.8ms, STAT: 0x0000)
[2026-05-22 04:30:02] DATA frame #3660 transmitted (latency: 0.7ms, STAT: 0x0000)
[2026-05-22 04:30:05] PDC keep-alive CMD received (response: HDR frame)
```

- **Write & Commit Card**: 동일 패턴 (프로그레스 바 + Toast)

**6. Save Endpoint 추가**

```rust
async fn save_l4_settings(Form(payload): Form<L4Form>) -> Html<String> {
    *get_l4_pdc_address().lock().unwrap() = payload.pdc_address;
    *L4_PMU_ID.lock().unwrap() = payload.pmu_id;
    *L4_DATA_RATE.lock().unwrap() = payload.data_rate;
    *get_l4_phasor_format().lock().unwrap() = payload.phasor_format;
    *get_l4_freq_format().lock().unwrap() = payload.freq_format;
    *L4_NOMINAL_FREQ.lock().unwrap() = payload.nominal_freq;
    *L4_NUM_PHASORS.lock().unwrap() = payload.num_phasors;
    Html("OK".to_string())
}
```

**7. Toggle Endpoint 확장**

`toggle_adapter()` 함수의 match에 `"l4"` 분기 추가.

---

## IEEE C37.118-2 프로토콜 참고 사항

> [!NOTE]
> C37.118-2 프레임 구조 참고 (UI 설명문에 활용):
> - **CONFIG-2 Frame**: PMU 설정 정보 (phasor 이름, 채널 수, 데이터 형식) 전송
> - **DATA Frame**: 실시간 phasor 측정값 + 주파수 + ROCOF + 디지털 상태
> - **HEADER Frame**: PMU 식별 텍스트 정보
> - **CMD Frame**: PDC→PMU 명령 (Start/Stop/CONFIG 요청)
>
> 주요 필드:
> - `IDCODE`: PMU 고유 식별자
> - `SOC` + `FRACSEC`: 초 단위 + 소수점 이하 타임스탬프 (PTP 동기)
> - `STAT`: 데이터 품질 플래그 (Bit 0-15)
> - `PHASORS[]`: 전압/전류 페이저 배열 (Magnitude + Angle)
> - `FREQ` / `DFREQ`: 주파수 편차 / ROCOF

---

## Verification Plan

### Build & UI 검증
1. `cargo build --workspace` 빌드 성공 확인
2. 서버 시작 후 `/north` 목록에서 L4 행이 표시되는지 확인
3. `/north/l4` 상세 페이지에서 설정 입력 필드, 모니터링 로그, 저장 버튼이 정상 동작하는지 확인
4. L4 Enable/Disable 토글 동작 확인
5. Write Configuration 버튼 클릭 시 파라미터가 정상 저장되는지 확인

### Commit
- `debug/` 폴더에 빌드 후 커밋
