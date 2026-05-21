//! `POST /calibration/{channel_id}` — update one channel's
//! `(gain, offset, unit_scale)` triple.
//!
//! Phase 0 validates the request body and **echoes back** the
//! applied calibration without writing through to the data plane —
//! the write-through path lands when this scaffold is wired into
//! the daemon (separate PR) and the daemon has a handle to
//! `svdc_console::operational::OperationalState`.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use crate::model::{ApiError, CalibrationApplied, CalibrationDto};
use crate::ManagementContext;

/// Apply (Phase 0: validate + echo) a calibration triple for one
/// channel. Returns 200 + [`CalibrationApplied`] on success, 400 +
/// [`ApiError`] on a NaN / infinity in any field.
pub async fn handler(
    State(_ctx): State<Arc<ManagementContext>>,
    Path(channel_id): Path<u16>,
    Json(body): Json<CalibrationDto>,
) -> impl IntoResponse {
    if let Err(msg) = validate(&body) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "bad_calibration".to_string(),
                message: msg,
            }),
        )
            .into_response();
    }

    let applied = CalibrationApplied {
        channel_id,
        calibration: body,
    };
    (StatusCode::OK, Json(applied)).into_response()
}

pub(crate) fn validate(c: &CalibrationDto) -> Result<(), String> {
    for (name, v) in [
        ("gain", c.gain),
        ("offset", c.offset),
        ("unit_scale", c.unit_scale),
    ] {
        if !v.is_finite() {
            return Err(format!("calibration field `{name}` is not finite ({v})"));
        }
    }
    if c.gain == 0.0 {
        return Err("calibration field `gain` must be non-zero".to_string());
    }
    if c.unit_scale == 0.0 {
        return Err("calibration field `unit_scale` must be non-zero".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nan_in_any_field_rejected() {
        for triple in [
            CalibrationDto {
                gain: f32::NAN,
                offset: 0.0,
                unit_scale: 1.0,
            },
            CalibrationDto {
                gain: 1.0,
                offset: f32::NAN,
                unit_scale: 1.0,
            },
            CalibrationDto {
                gain: 1.0,
                offset: 0.0,
                unit_scale: f32::NAN,
            },
        ] {
            let err = validate(&triple).unwrap_err();
            assert!(err.contains("not finite"));
        }
    }

    #[test]
    fn infinity_rejected() {
        let triple = CalibrationDto {
            gain: 1.0,
            offset: f32::INFINITY,
            unit_scale: 1.0,
        };
        let err = validate(&triple).unwrap_err();
        assert!(err.contains("not finite"));
    }

    #[test]
    fn zero_gain_or_unit_scale_rejected() {
        for triple in [
            CalibrationDto {
                gain: 0.0,
                offset: 0.0,
                unit_scale: 1.0,
            },
            CalibrationDto {
                gain: 1.0,
                offset: 0.0,
                unit_scale: 0.0,
            },
        ] {
            let err = validate(&triple).unwrap_err();
            assert!(err.contains("non-zero"));
        }
    }

    #[test]
    fn happy_path_accepted() {
        let triple = CalibrationDto {
            gain: 1.05,
            offset: -50.0,
            unit_scale: 0.01,
        };
        assert!(validate(&triple).is_ok());
    }
}
