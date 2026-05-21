import os
import re

for file in ['mu_detail.rs', 'dataplane.rs', 'calibration.rs', 'audit.rs']:
    p = f'crates/svdc-console/src/routes/{file}'
    if os.path.exists(p):
        with open(p, 'r', encoding='utf-8') as f:
            c = f.read()
        
        c = c.replace('use crate::templates::base::{layout, Section};', 'use crate::templates::base::layout;')
        
        # We find layout(Section::XXX, "Title", html! { ... })
        c = re.sub(r'layout\(\s*Section::(\w+),\s*(.*?),\s*html!', lambda m: f'layout({m.group(2)}, "{m.group(1).lower()}", html!', c, flags=re.DOTALL)
        
        with open(p, 'w', encoding='utf-8') as f:
            f.write(c)
