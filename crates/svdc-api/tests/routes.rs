//! HTTP-level tests against `svdc-api`. Drives each handler via
//! `tower::ServiceExt::oneshot` so the router is exercised end-to-
//! end without binding a real socket.

use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{header, Method, Request, StatusCode};
use svdc_aligner::TickBuffer;
use svdc_api::model::{
    ApiError, CalibrationApplied, CalibrationDto, ChannelsResponse, HealthResponse,
};
use svdc_api::{management_router, ManagementContext};
use svdc_core::{flags, Sample, SampleOrigin, TickRecord};
use tower::ServiceExt;

fn ctx_with_buffer(buffer: Arc<TickBuffer>) -> Arc<ManagementContext> {
    Arc::new(ManagementContext::new(buffer))
}

fn stamped(tick_id: u64) -> TickRecord {
    let mut r = TickRecord::empty(tick_id, tick_id * 1_000_000);
    r.n_channels = 1;
    r.set_flag(flags::COMPLETE);
    r.samples[0] = Sample {
        value_q: 100,
        quality: 0,
        origin: SampleOrigin::Live.as_u8(),
        reserved: 0,
    };
    r.stamp_crc();
    r
}

async fn body_string(resp: axum::response::Response) -> String {
    let bytes = to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn get_health_returns_ok_when_buffer_is_consistent() {
    let buf = Arc::new(TickBuffer::new(8));
    buf.push(stamped(0));
    buf.push(stamped(1));
    let router = management_router(ctx_with_buffer(buf));

    let resp = router
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body: HealthResponse = serde_json::from_str(&body_string(resp).await).unwrap();
    assert_eq!(body.status, "ok");
    assert_eq!(body.data_plane.tick_buffer_len, 2);
    assert_eq!(body.data_plane.tick_buffer_capacity, 8);
    assert_eq!(body.data_plane.integrity_violations, 0);
}

#[tokio::test]
async fn get_health_reports_degraded_when_a_record_is_tampered() {
    let buf = Arc::new(TickBuffer::new(8));
    // Push a stamped record, then mutate its payload to invalidate
    // the stored CRC.
    buf.push(stamped(0));
    let mut bad = stamped(1);
    bad.samples[0].value_q = 999; // post-stamp tamper
    buf.push(bad);

    let router = management_router(ctx_with_buffer(buf));
    let resp = router
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body: HealthResponse = serde_json::from_str(&body_string(resp).await).unwrap();
    assert_eq!(body.status, "degraded");
    assert_eq!(body.data_plane.integrity_violations, 1);
}

#[tokio::test]
async fn get_channels_returns_empty_list_in_phase_0() {
    let buf = Arc::new(TickBuffer::new(4));
    let router = management_router(ctx_with_buffer(buf));
    let resp = router
        .oneshot(
            Request::builder()
                .uri("/channels")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body: ChannelsResponse = serde_json::from_str(&body_string(resp).await).unwrap();
    assert!(body.channels.is_empty());
}

#[tokio::test]
async fn get_metrics_returns_prometheus_text_format() {
    let buf = Arc::new(TickBuffer::new(64));
    buf.push(stamped(0));
    buf.push(stamped(1));
    buf.push(stamped(2));
    let router = management_router(ctx_with_buffer(buf));
    let resp = router
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert!(ct.starts_with("text/plain"));
    let body = body_string(resp).await;

    // Each metric appears as a line + a HELP + a TYPE.
    assert!(body.contains("# HELP svdc_uptime_ms"));
    assert!(body.contains("# TYPE svdc_uptime_ms gauge"));
    assert!(body.contains("svdc_tick_buffer_len 3"));
    assert!(body.contains("svdc_tick_buffer_capacity 64"));
    assert!(body.contains("svdc_integrity_violations 0"));
}

#[tokio::test]
async fn post_calibration_echoes_back_the_applied_triple_on_success() {
    let buf = Arc::new(TickBuffer::new(4));
    let router = management_router(ctx_with_buffer(buf));
    let req_body = serde_json::to_string(&CalibrationDto {
        gain: 1.05,
        offset: -50.0,
        unit_scale: 0.01,
    })
    .unwrap();
    let resp = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/calibration/4")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(req_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body: CalibrationApplied = serde_json::from_str(&body_string(resp).await).unwrap();
    assert_eq!(body.channel_id, 4);
    assert!((body.calibration.gain - 1.05).abs() < 1e-5);
    assert!((body.calibration.unit_scale - 0.01).abs() < 1e-5);
}

#[tokio::test]
async fn post_calibration_rejects_malformed_json_with_4xx() {
    // serde_json serialises NaN as `null` which then fails to
    // deserialize back into f32; the HTTP test for "bad numeric
    // input" therefore lives at validate() (see calibration.rs
    // unit tests). Here we just confirm the router rejects
    // structurally invalid bodies cleanly.
    let buf = Arc::new(TickBuffer::new(4));
    let router = management_router(ctx_with_buffer(buf));
    let resp = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/calibration/0")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{ not json }"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        resp.status().is_client_error(),
        "malformed JSON should produce a 4xx, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn post_calibration_rejects_zero_gain_with_400() {
    let buf = Arc::new(TickBuffer::new(4));
    let router = management_router(ctx_with_buffer(buf));
    let req_body = serde_json::to_string(&CalibrationDto {
        gain: 0.0,
        offset: 0.0,
        unit_scale: 1.0,
    })
    .unwrap();
    let resp = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/calibration/0")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(req_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let err: ApiError = serde_json::from_str(&body_string(resp).await).unwrap();
    assert!(err.message.contains("non-zero"));
}
