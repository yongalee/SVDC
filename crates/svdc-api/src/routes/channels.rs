//! `GET /channels` — channel registry snapshot.
//!
//! Phase 0 returns an empty channel list. Phase 2 will plug in the
//! SCD-derived channel registry (SDD §7.2). The endpoint exists now
//! so monitoring consumers can bind against the URL contract before
//! the registry lands.

use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use crate::model::ChannelsResponse;
use crate::ManagementContext;

/// Returns 200 with an empty channels list in Phase 0.
pub async fn handler(State(_ctx): State<Arc<ManagementContext>>) -> Json<ChannelsResponse> {
    Json(ChannelsResponse {
        channels: Vec::new(),
    })
}
