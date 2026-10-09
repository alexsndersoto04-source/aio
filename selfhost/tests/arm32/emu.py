#!/usr/bin/env python3
"""Emulador mínimo de Linux ARM de 32 bits (modo usuario) para probar ejecutables ARM32 de Titan sin
hardware ARM32. Carga un ELF estático, simula el kernel (las llamadas al sistema más comunes, con
sus números y estructuras reales de ARM EABI) y ejecuta la CPU con Unicorn (Cortex-A9: VFPv3, sin udiv).

Uso: emu.py [--trace] [--max-insns N] programa [args...]

No es un Linux completo: una llamada que no conoce detiene la emulación con un mensaje (no inventa
resultados). Sirve para el desarrollo; la prueba definitiva es qemu-user en CI y un teléfono real."""
import sys, os, struct, time
from unicorn import *
from unicorn.arm_const import *

PAGE = 4096
def up(x, a=PAGE): return (x + a - 1) & ~(a - 1)

class Emu:
    def __init__(self, path, argv, envp, trace=False, max_insns=0):
        self.trace = trace
        self.max_insns = max_insns
        self.uc = Uc(UC_ARCH_ARM, UC_MODE_ARM)
        self.uc.ctl_set_cpu_model(UC_CPU_ARM_CORTEX_A9)
        self.exit_code = None
        self.brk_base = 0
        self.mmap_next = 0x40000000
        self.out = {1: sys.stdout.buffer, 2: sys.stderr.buffer}
        self.fds = {0: 0, 1: 1, 2: 2}
        self.insns = 0
        self.syscalls = {}
        self.load(path, argv, envp)
        # VFP/NEON activados
        v = self.uc.reg_read(UC_ARM_REG_C1_C0_2)
        self.uc.reg_write(UC_ARM_REG_C1_C0_2, v | (0xF << 20))
        self.uc.reg_write(UC_ARM_REG_FPEXC, 0x40000000)
        self.uc.hook_add(UC_HOOK_INTR, self.on_intr)
        if trace:
            self.uc.hook_add(UC_HOOK_CODE, self.on_code)

    def map(self, addr, size, perms=UC_PROT_ALL):
        a = addr & ~(PAGE - 1)
        e = up(addr + size)
        # no remapear lo ya mapeado
        for r in list(self.uc.mem_regions()):
            pass
        try:
            self.uc.mem_map(a, e - a, perms)
        except UcError:
            pass

    def load(self, path, argv, envp):
        data = open(path, 'rb').read()
        assert data[:4] == b'\x7fELF' and data[4] == 1 and data[5] == 1, "no es un ELF de 32 bits little-endian"
        (e_type, e_machine, _, e_entry, e_phoff, _, _, _, e_phentsize, e_phnum) = struct.unpack_from('<HHIIIIIHHH', data, 16)
        assert e_machine == 40, "no es ARM (EM_ARM=40)"
        top = 0
        for i in range(e_phnum):
            (p_type, p_off, p_vaddr, p_paddr, p_filesz, p_memsz, p_flags, p_align) = struct.unpack_from('<IIIIIIII', data, e_phoff + i * e_phentsize)
            if p_type != 1: continue
            self.map(p_vaddr, p_memsz)
            self.uc.mem_write(p_vaddr, data[p_off:p_off + p_filesz])
            top = max(top, p_vaddr + p_memsz)
        self.brk_base = up(top) + 0x100000
        self.brk = self.brk_base
        self.map(self.brk_base, 0x10000)
        # pila
        stack_top = 0xC0000000
        stack_size = 8 * 1024 * 1024
        self.map(stack_top - stack_size, stack_size)
        sp = stack_top - 0x1000
        def push_str(s):
            nonlocal sp
            b = s.encode() + b'\0'
            sp -= len(b)
            self.uc.mem_write(sp, b)
            return sp
        argp = [push_str(a) for a in argv]
        envpp = [push_str(e) for e in envp]
        sp &= ~15
        words = [len(argv)] + argp + [0] + envpp + [0] + [6, PAGE, 0, 0]  # AT_PAGESZ, AT_NULL
        if len(words) % 4: words += [0] * (4 - len(words) % 4)
        sp -= 4 * len(words)
        sp &= ~15
        self.uc.mem_write(sp, struct.pack('<%dI' % len(words), *words))
        self.uc.reg_write(UC_ARM_REG_SP, sp)
        self.entry = e_entry

    def on_code(self, uc, addr, size, user):
        self.insns += 1
        if self.max_insns and self.insns > self.max_insns:
            uc.emu_stop()

    def r(self, n): return self.uc.reg_read(UC_ARM_REG_R0 + n)
    def rd(self, a, n): return bytes(self.uc.mem_read(a, n))
    def rstr(self, a):
        out = b''
        while True:
            c = self.rd(a, 1)
            if c == b'\0': return out
            out += c; a += 1

    def on_intr(self, uc, intno, user):
        if intno != 2:
            raise RuntimeError("excepción %d en pc=%#x" % (intno, uc.reg_read(UC_ARM_REG_PC)))
        nr = self.r(7)
        a = [self.r(i) for i in range(6)]
        self.syscalls[nr] = self.syscalls.get(nr, 0) + 1
        res = self.sys(nr, a)
        if res is None:
            return
        uc.reg_write(UC_ARM_REG_R0, res & 0xFFFFFFFF)

    def sys(self, nr, a):
        uc = self.uc
        if nr in (1, 248):
            self.exit_code = a[0] & 255
            uc.emu_stop(); return None
        if nr == 4:  # write
            fd, buf, n = a[0], a[1], a[2]
            if fd in (1, 2):
                self.out[fd].write(self.rd(buf, n)); self.out[fd].flush()
                return n
            return -9
        if nr == 3:  # read
            fd, buf, n = a[0], a[1], a[2]
            if fd == 0:
                d = os.read(0, n); uc.mem_write(buf, d); return len(d)
            return -9
        if nr == 192:  # mmap2
            addr, length, prot, flags, fd, off = a
            length = up(length)
            if not (flags & 0x20): return -19  # solo anónimo
            if flags & 0x10 and addr:  # MAP_FIXED
                base = addr
            else:
                base = self.mmap_next
                self.mmap_next += length + PAGE
            try:
                uc.mem_map(base, length, UC_PROT_ALL)
            except UcError as e:
                return -12
            return base
        if nr == 91:  # munmap
            try: uc.mem_unmap(a[0], up(a[1]))
            except UcError: pass
            return 0
        if nr == 125: return 0  # mprotect
        if nr == 220: return 0  # madvise
        if nr == 263:  # clock_gettime (timespec de 32 bits)
            t = time.time(); s = int(t); ns = int((t - s) * 1e9)
            uc.mem_write(a[1], struct.pack('<ii', s, ns)); return 0
        if nr == 20: return 4242  # getpid
        if nr == 199: return 1000  # getuid32
        if nr == 54:  # ioctl
            return -25  # ENOTTY
        if nr == 175 or nr == 174: return 0  # sigprocmask / sigaction
        if nr == 197:  # fstat64: pretend regular
            return -9
        if nr == 384:  # getrandom
            uc.mem_write(a[0], os.urandom(a[1])); return a[1]
        if nr == 183:  # getcwd
            c = os.getcwd().encode() + b'\0'; uc.mem_write(a[0], c); return len(c)
        if nr == 122:  # uname
            b = b''.join(x.ljust(65, b'\0') for x in (b'Linux', b'emu', b'6.0', b'#1', b'armv7l', b''))
            uc.mem_write(a[0], b); return 0
        raise RuntimeError("llamada al sistema sin emular: %d (args %s) pc=%#x" % (nr, [hex(x) for x in a], uc.reg_read(UC_ARM_REG_PC)))

    def run(self):
        try:
            self.uc.emu_start(self.entry, 0xFFFFFFFF)
        except UcError as e:
            pc = self.uc.reg_read(UC_ARM_REG_PC)
            sys.stderr.write("\n[emu] error de CPU: %s en pc=%#x lr=%#x sp=%#x\n" % (e, pc, self.uc.reg_read(UC_ARM_REG_LR), self.uc.reg_read(UC_ARM_REG_SP)))
            return 139
        return self.exit_code if self.exit_code is not None else 1

if __name__ == '__main__':
    args = sys.argv[1:]
    trace = False; mx = 0
    while args and args[0].startswith('--'):
        if args[0] == '--trace': trace = True; args = args[1:]
        elif args[0] == '--max-insns': mx = int(args[1]); trace = True; args = args[2:]
        else: break
    if not args:
        sys.exit(__doc__)
    e = Emu(args[0], args, ['PATH=/usr/bin:/bin', 'HOME=/tmp'], trace, mx)
    t0 = time.time()
    code = e.run()
    if os.environ.get('EMU_STATS'):
        sys.stderr.write("[emu] %.2fs, syscalls: %s\n" % (time.time() - t0, e.syscalls))
    sys.exit(code)
