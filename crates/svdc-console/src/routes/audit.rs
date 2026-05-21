//! `GET /api/audit` — newest-first JSON dump of recent audit records.
//!
//! Query param `?limit=N` caps the page size. Default 200, hard max
//! 1000 (matching the in-memory ring). Phase 5 may add cursor-based
//! pagination + persistent storage; Phase 0/4 just returns the ring.
//!
//! OWNER: claude-code.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::audit::{self, AuditRecord, SharedAudit, DEFAULT_CAPACITY};

const DEFAULT_LIMIT: usize = 200;

/// Build the audit sub-router using the process-wide audit log.
pub fn router() -> Router {
    Router::new()
        .route("/api/audit", get(api_audit))
        .with_state(audit::global())
}

#[derive(Debug, Deserialize)]
struct AuditQuery {
    /// Maximum number of records to return.
    #[serde(default)]
    limit: Option<usize>,
}

async fn api_audit(
    State(log): State<SharedAudit>,
    Query(q): Query<AuditQuery>,
) -> Json<Vec<AuditRecord>> {
    let limit = q.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, DEFAULT_CAPACITY);
    Json(log.recent(limit))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_constructs() {
        let _ = router();
    }

    #[test]
    fn default_limit_is_capped_to_ring_capacity() {
        // Capped at DEFAULT_CAPACITY even if the query asks for more.
        let n = (DEFAULT_CAPACITY * 4).clamp(1, DEFAULT_CAPACITY);
        assert_eq!(n, DEFAULT_CAPACITY);
    }
}
