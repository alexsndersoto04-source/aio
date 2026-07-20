import os

path = os.path.expanduser("~/titan/crates/titan_mir/src/lib.rs")
with open(path, "r") as f:
    lines = f.readlines()

out = []
in_match = False
braces = 0
added_item = False

for line in lines:
    # 1. Arreglar mi error de sintaxis
    line = line.replace("expr: operand: src_op", "operand: src_op")
    
    # 2. Borrar el match obsoleto de else_b y reemplazarlo con la llamada directa
    if "let else_res = match else_b" in line:
        in_match = True
        braces = line.count('{') - line.count('}')
        out.append("                let else_res = lower_block(builder, else_b);\n")
        continue
        
    if in_match:
        braces += line.count('{') - line.count('}')
        if braces <= 0:
            in_match = False
        continue
        
    # 3. Arreglar el Option<MirOperand> desempaquetando correctamente
    if "if else_res != MirOperand::Undef" in line:
        line = line.replace("if else_res != MirOperand::Undef", "if else_res != Some(MirOperand::Undef) && else_res.is_some()")
        
    if "src: else_res" in line:
        line = line.replace("src: else_res", "src: else_res.unwrap()")
        
    out.append(line)
    
    # 4. Agregar el Stmt::Item que falta en el enum de tu AST
    if not added_item and "Stmt::Expr(" in line and "=>" in line:
        out.append("        Stmt::Item(_) => None,\n")
        added_item = True

with open(path, "w") as f:
    f.writelines(out)
