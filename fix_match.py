import os

path = os.path.expanduser("~/titan/crates/titan_mir/src/lib.rs")
with open(path, "r") as f:
    lines = f.readlines()

start_idx = -1
for i, l in enumerate(lines):
    if "if let Some(else_b) = else_branch {" in l:
        start_idx = i
        break

if start_idx != -1:
    braces = 0
    end_idx = -1
    for i in range(start_idx, len(lines)):
        braces += lines[i].count('{') - lines[i].count('}')
        if braces == 0:
            end_idx = i
            break
            
    new_block = """    if let Some(else_b) = else_branch {
        let else_res = lower_block(builder, else_b);
        if let Some(res) = else_res {
            if res != MirOperand::Undef {
                builder.emit(MirInst::Move { dst, src: res });
            }
        }
    }\n"""
    
    lines = lines[:start_idx] + [new_block] + lines[end_idx+1:]
    
    with open(path, "w") as f:
        f.writelines(lines)
