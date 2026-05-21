import os
import re

def resolve(filepath, prefer='theirs'):
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # Simple regex to replace git conflict markers.
    pattern = re.compile(r'<<<<<<< HEAD\n(.*?)\n=======\n(.*?)\n>>>>>>> [^\n]+', re.DOTALL)
    
    def replacer(match):
        ours = match.group(1)
        theirs = match.group(2)
        if prefer == 'ours': return ours
        elif prefer == 'theirs': return theirs
        else: return ours + "\n" + theirs # keep both
        
    resolved = pattern.sub(replacer, content)
    with open(filepath, 'w', encoding='utf-8') as f:
        f.write(resolved)

# Apply resolutions
resolve('crates/svdc-console/src/assets/styles.css', 'both')
resolve('crates/svdc-console/src/templates/base.rs', 'both')
resolve('crates/svdc-console/src/lib.rs', 'both')
resolve('crates/svdc-console/Cargo.toml', 'both')
resolve('Cargo.toml', 'both')
resolve('crates/svdc-bin/Cargo.toml', 'both')
resolve('crates/svdc-bin/src/main.rs', 'both')
resolve('crates/svdc-console/src/sse/mod.rs', 'both')
resolve('crates/svdc-console/src/sse/emitter.rs', 'both')
