import re
import os

with open('crates/svdc-console/src/lib.rs', 'r', encoding='utf-8') as f:
    s = f.read()
s = s.replace('.merge(routes::sse::register(Router::new()))', '.merge(routes::sse::router())')
with open('crates/svdc-console/src/lib.rs', 'w', encoding='utf-8') as f:
    f.write(s)

def replace_section(path):
    if not os.path.exists(path): return
    with open(path, 'r', encoding='utf-8') as f:
        s = f.read()
    
    s = s.replace('use crate::templates::base::{layout, Section};', 'use crate::templates::base::layout;')
    s = s.replace('Section::Southbound,', '')
    s = s.replace('Section::Dataplane,', '')
    s = s.replace('Section::Configuration,', '')
    s = s.replace('Section::Audit,', '')
    s = s.replace('Section::Monitoring,', '')
    s = s.replace('Section::Northbound,', '')
    
    # Actually, the original was `layout(Section::XXX, "Title", html! { ... })`
    # My layout signature is `pub fn layout(title: &str, active_nav: &str, content: Markup)`
    # So I need to replace `layout(Section::XXX, "Title", body)` -> `layout("Title", "xxx", body)`
    # Let's just do a regex replace for the whole `layout(...)` call if possible.
    # Since regex is tricky, I'll just write a specific replacer:
    
    # mu_detail: layout(Section::Southbound, &title, body)
    s = re.sub(r'layout\(\s*Section::(\w+),\s*(&?title),\s*(body)\s*\)', lambda m: f'layout({m.group(2)}, "{m.group(1).lower()}", {m.group(3)})', s)
    
    # dataplane: layout(Section::Dataplane, "Data plane (diagnostics)", html! {
    s = re.sub(r'layout\(\s*Section::(\w+),\s*("(?:[^"\\]|\\.)*"),\s*html!', lambda m: f'layout({m.group(2)}, "{m.group(1).lower()}", html!', s)

    # calibration.rs
    s = re.sub(r'layout\(\s*Section::(\w+),\s*("(?:[^"\\]|\\.)*"),\s*html!', lambda m: f'layout({m.group(2)}, "{m.group(1).lower()}", html!', s)
    
    # audit.rs
    s = re.sub(r'layout\(\s*Section::(\w+),\s*("(?:[^"\\]|\\.)*"),\s*html!', lambda m: f'layout({m.group(2)}, "{m.group(1).lower()}", html!', s)

    # Also handle string literals passed as title instead of format! or &title
    # wait, I already did that with `"(?:[^"\\]|\\.)*"`

    # Let's catch any remaining layout(Section::XXX, title_expr, 
    s = re.sub(r'layout\(\s*Section::(\w+),\s*(.*?),\s*(html!|body)', lambda m: f'layout({m.group(2)}, "{m.group(1).lower()}", {m.group(3)}', s)
    
    with open(path, 'w', encoding='utf-8') as f:
        f.write(s)

for f in ['mu_detail.rs', 'dataplane.rs', 'calibration.rs', 'audit.rs']:
    replace_section(f'crates/svdc-console/src/routes/{f}')
