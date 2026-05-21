/* Assets Embedder Module
   OWNER: antigravity
   Agent: antigravity-subagent-ui-spec
   NFR-10: English-only comments and identifiers
*/

use rust_embed::RustEmbed;

/// rust-embed marker struct: all files under `src/assets/` are compiled
/// into the binary at build time and served from the `/assets/*` route.
#[derive(RustEmbed)]
#[folder = "src/assets/"]
pub struct Assets;
