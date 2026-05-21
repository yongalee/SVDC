# Northbound "Write & Commit Configuration" — Review Analysis

> **Review Date**: 2026-05-22  
> **Scope**: All four Northbound adapter detail pages (L0, L1, L2, L3)  
> **Reference Documents**: SDD v0.1 §5, §6, §8, CIGRE 2024 Paper §8

---

## 1. SDD가 정의한 Northbound 아키텍처

SDD §5.1과 §8.2에서 정의하는 "Northbound"는 다음을 의미한다:

| SDD Section | 정의 | 설명 |
|---|---|---|
| §5.1 Context | `north: pull API + write-back` | EBP Relays, Phasor Computation, Transient Recorder, Fault Locator, QSE가 소비 |
| §8.2 Subscriber API | **In-process (C ABI)** + **Out-of-process (UNIX domain socket)** | SVDC 내부 모듈 간 데이터 전달 |
| §8.3 Write-back | `svdc_writeback()` | QSE가 compromised SV를 교체 |
| §8.4 Management | `GET /health`, `GET /channels`, `GET /metrics`, `POST /calibration/{channel_id}` | HTTP/JSON telemetry |

> [!IMPORTANT]
> SDD는 OPC UA, MQTT, TimescaleDB를 **명시적으로 언급하지 않는다**. 현재 Northbound Controls의 L1~L3 어댑터는 **SDD를 초과하는 확장 구현**이다.

---

## 2. 현재 "Write & Commit" 구현 상태 분석

### 2.1 공통 패턴 (4개 어댑터 모두 동일)

각 어댑터의 `writeConfiguration()` Alpine.js 함수가 동일한 패턴을 따른다:

```
1. saving = true, progress = 0
2. setInterval로 progress 10%씩 증가 (60ms 간격 → 총 600ms)
3. progress >= 100 도달 시 fetch POST 호출
4. 응답 후 toast 표시, 4초 후 사라짐
```

### 2.2 어댑터별 세부 구현

| 항목 | L0 SHM | L1 OPC UA | L2 MQTT | L3 TimescaleDB |
|---|---|---|---|---|
| **API Endpoint** | `/api/v1/northbound/l0/save` | `/api/v1/northbound/l1/save` | `/api/v1/northbound/l2/save` | `/api/v1/northbound/l3/save` |
| **설정 필드** | path, buffer_size, lock_mode, sync_mode | address, namespace, security, max_sessions, pub_interval | broker, topic, qos, keep_alive, clean_session, pub_rate | conn_string, target_table, batch_size, delay_limit, retention_days, pool_size |
| **Warning 메시지** | "...atomic IPC ringbuffer re-alignment. Local application queues will momentarily buffer..." | "Applying new endpoints will cause active SCADA sessions to restart. Connection failovers will trigger automatically." | "...reconnects the MQTT publisher stack. Topic structures will refresh..." | "...restarts database connection pools. Tables are auto-migrated if table schemas don't match." |
| **Progress 텍스트** | "Re-mapping shared memory segment..." | "Restarting OPC UA Server stack..." | "Re-establishing broker subscription..." | "Flushing database pools..." |
| **Toast 메시지** | "L0 ... saved successfully." | "L1 ... saved successfully." | "L2 ... saved successfully." | "L3 ... saved successfully." |
| **Backend 동작** | Mutex lock → 값 교체 | Mutex lock → 값 교체 | Mutex lock → 값 교체 | Mutex lock → 값 교체 |

---

## 3. 확인된 문제점 및 개선 사항

### ✅ 잘 구현된 부분

1. **프로토콜별 적절한 설정 필드**: 각 어댑터의 프로토콜 특성에 맞는 configuration 파라미터 제공
2. **Warning 메시지 분화**: 어댑터별 서로 다른 side-effect 경고 (세션 재시작, 브로커 재연결 등)
3. **Progress 애니메이션**: 사용자 피드백을 위한 시각적 프로그레스 바
4. **Toast 알림**: 저장 완료 알림
5. **Guard 패턴**: `if (this.saving) return;` 중복 저장 방지
6. **Disabled 상태 버튼**: 저장 중 버튼 비활성화

### ⚠️ 개선이 필요한 부분

| # | 문제 | 심각도 | 설명 |
|---|---|---|---|
| 1 | **Fake progress bar** | 낮음 | Progress가 실제 서버 작업 진행률이 아닌, 단순 60ms × 10 = 600ms 타이머임. 서버 save는 거의 즉시 완료(Mutex lock/unlock)되므로 progress bar가 오해를 줄 수 있음. 다만 UX 관점에서 사용자에게 "작업 중" 인상을 주기 위한 의도적 선택으로 볼 수 있음 |
| 2 | **Error handling 부재** | 중간 | `fetch().then()` 체인에 `.catch()` 없음. 네트워크 실패 또는 서버 에러 시 사용자에게 피드백 없음 |
| 3 | **Confirmation dialog 없음** | 낮음 | L1 OPC UA는 "active SCADA sessions restart" 경고를 표시하지만, 실제 confirm dialog가 없어 실수로 클릭 시 즉시 실행됨 |
| 4 | **Input validation 없음** | 중간 | 클라이언트 측 유효성 검사 없음 (예: 빈 URL, 0이하 batch size 등) |
| 5 | **서버 측 validation 없음** | 중간 | 백엔드 `save_*_settings` 함수들이 단순히 값을 교체하며, 유효성 검증이 없음 |
| 6 | **Response 코드 무시** | 낮음 | fetch 응답의 HTTP status를 체크하지 않음 |

### ❌ SDD 대비 누락 기능

| # | SDD 요구사항 | 현재 상태 | 설명 |
|---|---|---|---|
| 1 | **§8.4 `POST /calibration/{channel_id}`** | 별도 Southbound MU 상세 페이지에 구현 | Northbound와 별개 — 정상 |
| 2 | **§8.4 `GET /health`, `/channels`, `/metrics`** | Dashboard에서 정적 표시 | API endpoint 자체는 미구현, 하드코딩된 mock 데이터 표시 |

> [!NOTE]
> L0~L3 Northbound 어댑터 자체는 SDD에 명시되지 않은 **확장 구현**이므로 SDD 위반이 아니다. SDD §8.2의 Subscriber API (C ABI + UNIX domain socket)를 L0 Shared Memory RingBuffer로 구현한 것은 설계 의도와 부합한다.

---

## 4. 결론

**"Write & Commit Configuration" 기능은 전체적으로 잘 구현되어 있다.**

- 4개 어댑터 모두 동일한 UX 패턴(설정 편집 → progress bar → 저장 → toast)을 따르며, 각 프로토콜에 맞는 적절한 경고 메시지를 제공한다
- 백엔드는 thread-safe한 Mutex/AtomicBool 기반 상태 관리를 사용한다
- SDD §8.2의 Subscriber API를 L0 SHM으로 잘 매핑했고, L1~L3는 의도적인 확장 기능이다

**주요 개선 권장사항**:
1. `fetch()` 호출에 `.catch()` 에러 핸들링 추가
2. L1 OPC UA의 SCADA 세션 재시작 같은 중대 작업에 `confirm()` dialog 추가
3. 기본 입력 유효성 검사 추가 (빈 값, 범위 체크)

이 개선사항들은 즉시 구현이 필요한 수준은 아니며, production 배포 전 polish 단계에서 적용하면 충분하다.
