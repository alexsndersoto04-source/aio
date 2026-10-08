#!/usr/bin/env python3
# Genera ../std_procfs_arm.titan (pf_arm_impl / pf_arm_part) desde arm_tab.txt, que
# copia las tablas get_arm_implementer / get_arm_part de sysinfo 0.32.1
# (src/unix/linux/cpu.rs). Uso: cd selfhost/native/sysinfo && python3 gen_arm.py
L = open('arm_tab.txt').read().split('--\n')
impl = [l.split(' ', 1) for l in L[0].strip().split('\n')]
parts = [l.split(' ', 2) for l in L[1].strip().split('\n')]
o = ['// GENERADO por native/sysinfo/gen_arm.py: no editar a mano.', '// Tablas de get_arm_implementer / get_arm_part de sysinfo 0.32.1 (copiadas en',
     '// native/sysinfo/arm_tab.txt; ver gen_arm.py).',
     'fn pf_arm_impl(x: int) -> any {']
for h, n in impl:
    o.append(f'    if x == {int(h, 16)} {{\n        return "{n}"\n    }}')
o += ['    nil', '}', '', 'fn pf_arm_part(x: int, p: int) -> any {']
seen = set()
for h, pp, n in parts:
    k = (int(h, 16), int(pp, 16))
    if k in seen:
        continue  # en un match gana el primer brazo
    seen.add(k)
    o.append(f'    if x == {k[0]} && p == {k[1]} {{\n        return "{n}"\n    }}')
o += ['    nil', '}', '']
open('../std_procfs_arm.titan', 'w').write('\n'.join(o))
print(len(impl), 'fabricantes,', len(seen), 'modelos')
