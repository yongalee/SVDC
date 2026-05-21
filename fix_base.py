import sys

def modify_base():
    with open('crates/svdc-console/src/templates/base.rs', 'r', encoding='utf-8') as f:
        content = f.read()

    # Append Dataplane and Northbound to the sidebar menu
    target = r'span class="nav-text" { "Southbound MUs" }' + '\n                            }'
    replacement = target + """
                            a href="/north" class=(if active_nav == "northbound" { "nav-link active" } else { "nav-link" }) {
                                span class="nav-icon" {
                                    svg fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                                        path stroke-linecap="round" stroke-linejoin="round" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" {}
                                    }
                                }
                                span class="nav-text" { "Northbound Controls" }
                            }
                            a href="/dataplane" class=(if active_nav == "dataplane" { "nav-link active" } else { "nav-link" }) {
                                span class="nav-icon" {
                                    svg fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                                        path stroke-linecap="round" stroke-linejoin="round" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" {}
                                    }
                                }
                                span class="nav-text" { "Data plane" }
                            }"""

    content = content.replace(target, replacement)
    
    # Update cache busters
    content = content.replace('?v=1', '?v=2').replace('?v=8', '?v=9')
    if 'styles.css"' in content:
        content = content.replace('styles.css"', 'styles.css?v=2"')
        
    # Also we need to make sure the enum `Section` is correctly not used anymore or we add it?
    # Actually my base.rs uses `active_nav: &str`, so no enum needed!
    
    with open('crates/svdc-console/src/templates/base.rs', 'w', encoding='utf-8') as f:
        f.write(content)

if __name__ == "__main__":
    modify_base()
