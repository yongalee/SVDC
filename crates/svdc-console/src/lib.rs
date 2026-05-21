/* SVDC Console Library
   OWNER: claude-code
   Agent: antigravity-subagent-ui-spec
   NFR-10: English-only comments and identifiers
*/

use axum::response::IntoResponse;

pub mod assets;
pub mod sse;
pub mod routes {
    pub mod config;
    pub mod dashboard;
    pub mod monitoring;
    pub mod mus_list;
    pub mod northbound;
}
pub mod templates {
    pub mod base;
    pub mod components;
}

/// Register console routes from all router modules
pub fn register_routes(mut router: axum::Router) -> axum::Router {
    router = routes::dashboard::register(router);
    router = routes::mus_list::register(router);
    router = routes::northbound::register(router);
    router = routes::monitoring::register(router);
    router = routes::config::register(router);
    router
}

/// Serve static assets embedded in the binary via rust-embed
async fn serve_assets(axum::extract::Path(file): axum::extract::Path<String>) -> impl IntoResponse {
    let path = file.as_str();
    match assets::Assets::get(path) {
        Some(content) => {
            let mime_type = if path.ends_with(".css") {
                "text/css"
            } else if path.ends_with(".js") {
                "application/javascript"
            } else if path.ends_with(".woff2") {
                "font/woff2"
            } else {
                "application/octet-stream"
            };

            (
                [
                    (axum::http::header::CONTENT_TYPE, mime_type),
                    (
                        axum::http::header::CACHE_CONTROL,
                        "public, max-age=31536000",
                    ),
                ],
                content.data,
            )
                .into_response()
        }
        None => (axum::http::StatusCode::NOT_FOUND, "Asset not found").into_response(),
    }
}

/// Start console web server on the specified bind address and block the thread
pub fn start_console(bind_addr: &str) {
    let addr = bind_addr.to_string();
    println!("svdc-console: Starting console web server on {}...", addr);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async move {
        let app = register_routes(axum::Router::new())
            .route("/assets/*file", axum::routing::get(serve_assets));

        let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });
}
