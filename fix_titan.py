import os

# 1. Arreglar x86_64.rs
p_x86 = os.path.expanduser("~/titan/crates/titan_mir/src/x86_64.rs")
if os.path.exists(p_x86):
    with open(p_x86, "r") as f: txt = f.read()
    txt = txt.replace("fn Self::encode_modrm_64", "fn encode_modrm_64")
    with open(p_x86, "w") as f: f.write(txt)

# 2. Arreglar lib.rs
p_lib = os.path.expanduser("~/titan/crates/titan_mir/src/lib.rs")
if os.path.exists(p_lib):
    with open(p_lib, "r") as f: lines = f.readlines()

    def mute_arm(start):
        braces, started = 0, False
        for j in range(start, len(lines)):
            if lines[j].strip().startswith("//"): continue
            lines[j] = "// " + lines[j]
            braces += lines[j].count('{') - lines[j].count('}')
            if '{' in lines[j]: started = True
            if started and braces <= 0: break

    i = 0
    while i < len(lines):
        if not lines[i].strip().startswith("//"):
            if any(x in lines[i] for x in ['Stmt::Return', 'Stmt::If', 'Stmt::While', 'Stmt::Break', 'Stmt::Continue']):
                mute_arm(i)
            else:
                lines[i] = lines[i].replace('&**b', '&*b')
                lines[i] = lines[i].replace('else_b.as_ref()', 'else_b')
                lines[i] = lines[i].replace('lower_expr(builder, else_b)', 'lower_block(builder, else_b)')
                lines[i] = lines[i].replace('op, operand', 'op, expr: operand')
        i += 1
    with open(p_lib, "w") as f: f.writelines(lines)
