import re
with open('crates/svdc-console/src/templates/base.rs', 'r', encoding='utf-8') as f:
    s = f.read()

nav = """                            a href="/dataplane" class=(if active_nav == "dataplane" { "nav-link active" } else { "nav-link" }) {
                                span class="nav-icon" {
                                    svg fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                                        path stroke-linecap="round" stroke-linejoin="round" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" {}
                                    }
                                }
                                span class="nav-text" { "Data plane" }
                            }
"""

if 'href="/dataplane"' not in s:
    # Insert before configuration
    s = s.replace('                            a href="/config"', nav + '                            a href="/config"')
    
with open('crates/svdc-console/src/templates/base.rs', 'w', encoding='utf-8') as f:
    f.write(s)
