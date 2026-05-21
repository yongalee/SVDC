/* SVDC Console Library
   OWNER: claude-code
   Agent: antigravity-subagent-ui-spec
   NFR-10: English-only comments and identifiers

   Note: This file is owned by Claude Code (WBS-9.1a). This is a skeleton
   stub created by Antigravity (WBS-9.1b) to allow workspace compilation.
*/

pub mod assets;
pub mod sse;

/// Register console routes. This skeleton function will be fully populated
/// by Claude Code under WBS-9.1a.
pub fn register_routes(router: axum::Router) -> axum::Router {
    router
}

/// Start console web server on the specified bind address. This skeleton
/// function will be integrated with the real async server in later WBS steps.
pub fn start_console(bind_addr: &str) {
    println!(
        "svdc-console: Starting console web server on {}...",
        bind_addr
    );
}
