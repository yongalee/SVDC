import re
import os

with open('crates/svdc-console/src/lib.rs', 'r', encoding='utf-8') as f:
    s = f.read()
s = s.replace('routes::sse::register(Router::new())', 'routes::sse::router()')
with open('crates/svdc-console/src/lib.rs', 'w', encoding='utf-8') as f:
    f.write(s)

def replace_section(path):
    if not os.path.exists(path): return
    with open(path, 'r', encoding='utf-8') as f:
        s = f.read()
    
    # regex replace for use crate::templates::base::{layout, Section};
    s = re.sub(r'use crate::templates::base::\{\s*layout\s*,\s*Section\s*\}', 'use crate::templates::base::layout', s)
    s = re.sub(r'use crate::templates::base::\{\s*Section\s*,\s*layout\s*\}', 'use crate::templates::base::layout', s)
    s = s.replace('use crate::templates::base::{layout, Section};', 'use crate::templates::base::layout;')
    
    s = re.sub(r'layout\(\s*Section::(\w+),\s*(&?title),\s*(body)\s*\)', lambda m: f'layout({m.group(2)}, "{m.group(1).lower()}", {m.group(3)})', s)
    s = re.sub(r'layout\(\s*Section::(\w+),\s*("(?:[^"\\]|\\.)*"),\s*html!', lambda m: f'layout({m.group(2)}, "{m.group(1).lower()}", html!', s)
    s = re.sub(r'layout\(\s*Section::(\w+),\s*(.*?),\s*(html!|body)', lambda m: f'layout({m.group(2)}, "{m.group(1).lower()}", {m.group(3)}', s)
    
    with open(path, 'w', encoding='utf-8') as f:
        f.write(s)

for f in ['mu_detail.rs', 'dataplane.rs', 'calibration.rs', 'audit.rs']:
    replace_section(f'crates/svdc-console/src/routes/{f}')
