/* Assets Embedder Module
   OWNER: antigravity
   Agent: antigravity-subagent-ui-spec
   NFR-10: English-only comments and identifiers
*/

use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "src/assets/"]
pub struct Assets;
