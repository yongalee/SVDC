//! Base layout: sidebar nav, top bar, main content slot.
//!
//! Matches `docs/SVDC_UI_Design_Document_v0.1.html` §3 IA. Five nav
//! sections (Dashboard, South-bound, North-bound, Monitoring,
//! Configuration). Top bar carries the brand, breadcrumb, live local
//! time, and a global health dot.
//!
//! OWNER: claude-code (WBS-9.1a)
//! NFR-10: English-only.

use maud::{html, Markup, DOCTYPE};

/// Section the visitor is currently in. Used for `aria-current` and
/// active-link highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    /// `/`
    Dashboard,
    /// `/south/...`
    Southbound,
    /// `/north/...`
    Northbound,
    /// `/monitoring`
    Monitoring,
    /// `/config`
    Configuration,
}

/// Render a full page with sidebar + top bar + main content.
///
/// `title` is shown both in `<title>` and in the top bar. `section`
/// drives nav highlighting. `body` is the screen-specific markup.
pub fn layout(section: Section, title: &str, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " · SVDC Operator Console" }
                link rel="stylesheet" href="/assets/styles.css";
                script src="/assets/htmx.min.js" {}
                script src="/assets/alpine.min.js" defer {}
            }
            body {
                aside.sidebar aria-label="Primary navigation" {
                    div.sidebar-brand {
                        span.brand-mark { "SVDC" }
                        span.brand-node { "a²SDP node-01" }
                    }
                    nav.sidebar-nav {
                        (nav_link("/", "Dashboard", section == Section::Dashboard))
                        div.nav-group {
                            span.nav-group-label { "South-bound" }
                            (nav_link("/south/mus", "Merging Units", section == Section::Southbound))
                        }
                        div.nav-group {
                            span.nav-group-label { "North-bound" }
                            (nav_link("/north", "Application layers", section == Section::Northbound))
                        }
                        (nav_link("/monitoring", "Monitoring", section == Section::Monitoring))
                        (nav_link("/config", "Configuration", section == Section::Configuration))
                    }
                    div.sidebar-footer {
                        span.foot-tag { "v0.0.1 · Phase 0" }
                    }
                }
                div.page {
                    header.topbar {
                        h1.screen-title { (title) }
                        div.topbar-meta {
                            span.live-clock id="live-clock" { "--:--:--" }
                            span.health-dot.healthy
                                id="health-dot"
                                title="System healthy" {}
                        }
                    }
                    main.main-content { (body) }
                }
                script {
                    // Local 10 Hz clock — purely cosmetic; PTP-disciplined
                    // time arrives via SSE from the daemon when available.
                    (maud::PreEscaped(LIVE_CLOCK_JS))
                }
            }
        }
    }
}

fn nav_link(href: &str, label: &str, active: bool) -> Markup {
    html! {
        @if active {
            a.nav-link.active href=(href) aria-current="page" { (label) }
        } @else {
            a.nav-link href=(href) { (label) }
        }
    }
}

const LIVE_CLOCK_JS: &str = r#"
(function () {
  const el = document.getElementById('live-clock');
  if (!el) return;
  const tick = () => {
    const now = new Date();
    const pad = (n) => String(n).padStart(2, '0');
    el.textContent = pad(now.getHours()) + ':' + pad(now.getMinutes()) + ':' + pad(now.getSeconds());
  };
  tick();
  setInterval(tick, 100);
})();
"#;
