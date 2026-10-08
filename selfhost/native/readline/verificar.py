#!/usr/bin/env python3
"""Compara la edición de líneas de Titan y la VM dentro de una terminal real."""

from __future__ import annotations

import fcntl
import os
import pty
import re
import select
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
PROGRAM = ROOT / "selfhost/native/readline/prueba_readline.titan"


def attach_controlling_terminal() -> None:
    os.setsid()
    fcntl.ioctl(0, termios.TIOCSCTTY, 0)


def read_available(master: int, timeout: float) -> bytes:
    ready, _, _ = select.select([master], [], [], timeout)
    if not ready:
        return b""
    try:
        return os.read(master, 4096)
    except OSError:
        return b""


def run_pty(command: list[str], marker: bytes, keystrokes: bytes, timeout: int = 12) -> tuple[int, bytes]:
    return run_pty_steps(command, [(marker, keystrokes)], timeout)


def run_pty_steps(
    command: list[str], interactions: list[tuple[bytes, bytes]], timeout: int = 12
) -> tuple[int, bytes]:
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
    process = subprocess.Popen(
        command,
        cwd=ROOT,
        stdin=slave,
        stdout=slave,
        stderr=slave,
        close_fds=True,
        preexec_fn=attach_controlling_terminal,
    )
    os.close(slave)
    output = bytearray()
    deadline = time.monotonic() + timeout
    marker_cursor = 0
    try:
        for marker, keystrokes in interactions:
            while marker not in output[marker_cursor:]:
                if process.poll() is not None:
                    break
                if time.monotonic() >= deadline:
                    raise TimeoutError("el programa no mostró el siguiente prompt")
                output.extend(read_available(master, 0.1))
            relative = bytes(output[marker_cursor:]).find(marker)
            if relative < 0:
                process.wait(timeout=2)
                return process.returncode, bytes(output)
            marker_cursor += relative + len(marker)
            os.write(master, keystrokes)
        try:
            process.wait(timeout=max(0.1, deadline - time.monotonic()))
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=2)
            while True:
                chunk = read_available(master, 0.05)
                if not chunk:
                    break
                output.extend(chunk)
            raise TimeoutError("el lector quedó esperando; terminal: " + output.decode(errors="replace"))
        while True:
            chunk = read_available(master, 0.05)
            if not chunk:
                break
            output.extend(chunk)
        return process.returncode, bytes(output)
    except BaseException:
        if process.poll() is None:
            process.kill()
            process.wait(timeout=2)
        raise
    finally:
        os.close(master)


def result_line(output: bytes) -> str | None:
    found = re.search(rb"RESULT:([^\r\n]*)", output)
    if found is None:
        return None
    return found.group(1).decode("utf-8", errors="replace")


def check_pair(
    label: str,
    zett: str,
    executable: str,
    mode: str,
    path_vm: str,
    path_native: str,
    marker: bytes,
    keys: bytes,
    expected: str,
) -> tuple[bytes, bytes]:
    vm_code, vm_output = run_pty([zett, "run", str(PROGRAM.relative_to(ROOT)), mode, path_vm], marker, keys)
    native_code, native_output = run_pty([executable, mode, path_native], marker, keys)
    if vm_code != 0 or native_code != 0:
        raise AssertionError(
            f"{label}: código VM={vm_code}, nativo={native_code}\n"
            f"--- VM ---\n{vm_output.decode(errors='replace')}\n"
            f"--- nativo ---\n{native_output.decode(errors='replace')}"
        )
    if result_line(vm_output) != expected or result_line(native_output) != expected:
        raise AssertionError(
            f"{label}: se esperaba {expected!r}; VM={result_line(vm_output)!r}, "
            f"nativo={result_line(native_output)!r}\n"
            f"--- VM ---\n{vm_output.decode(errors='replace')}\n"
            f"--- nativo ---\n{native_output.decode(errors='replace')}"
        )
    return vm_output, native_output


def check_memory_history(zett: str, executable: str, path: str) -> None:
    interactions = [(b"FIRST> ", b"saved-line\r"), (b"SECOND> ", b"\x1b[A\r")]
    commands = (
        [zett, "run", str(PROGRAM.relative_to(ROOT)), "with-history-twice", path],
        [executable, "with-history-twice", path],
    )
    results = [run_pty_steps(command, interactions) for command in commands]
    expected = "saved-line|saved-line"
    if any(code != 0 or result_line(output) != expected for code, output in results):
        raise AssertionError(
            "el historial en memoria no pasó a la segunda llamada:\n"
            f"--- VM ({results[0][0]}) ---\n{results[0][1].decode(errors='replace')}\n"
            f"--- nativo ({results[1][0]}) ---\n{results[1][1].decode(errors='replace')}"
        )


def check_noop_history_save(zett: str, executable: str, temporary: Path) -> None:
    cases = (("v2", b"#V2\nsame-entry\n"), ("legacy", b"same-entry\n"))
    for name, initial in cases:
        vm_path = temporary / f"noop-{name}-vm.txt"
        native_path = temporary / f"noop-{name}-native.txt"
        vm_path.write_bytes(initial)
        native_path.write_bytes(initial)
        untouched_ns = 1_600_000_000_000_000_000
        os.utime(vm_path, ns=(untouched_ns, untouched_ns))
        os.utime(native_path, ns=(untouched_ns, untouched_ns))
        commands = (
            [zett, "run", str(PROGRAM.relative_to(ROOT)), "persistent-check", str(vm_path)],
            [executable, "persistent-check", str(native_path)],
        )
        results = [run_pty(command, b"HISTORY> ", b"\x1b[A\r") for command in commands]
        failures = []
        for label, (code, output) in zip(("VM", "nativo"), results):
            result = result_line(output)
            if code != 0 or result != "history-ok":
                failures.append(
                    f"--- {label} (código {code}, RESULT={result!r}) ---\n"
                    f"{output.decode(errors='replace')}"
                )
        if failures:
            raise AssertionError(
                f"historial {name}: falló la lectura repetida sin cambios\n" + "\n".join(failures)
            )
        if any(
            path.read_bytes() != initial or path.stat().st_mtime_ns != untouched_ns
            for path in (vm_path, native_path)
        ):
            raise AssertionError(f"historial {name}: se reescribió el archivo sin una entrada nueva")


def check_v2_history(zett: str, executable: str, temporary: Path) -> None:
    backslash = b"\\" * 2
    newline_escape = b"\\n"
    cases = (
        (
            "persistent-v2-backslash",
            b"#V2\nslash" + backslash + b"entry\n",
            b"#V2\nslash" + backslash + b"entry\nslash" + backslash + b"entryX\n",
        ),
        (
            "persistent-v2-newline",
            b"#V2\nfirst" + newline_escape + b"second\n",
            b"#V2\nfirst" + newline_escape + b"second\nfirst" + newline_escape + b"secondX\n",
        ),
        (
            "persistent-v2-invalid",
            b"#V2\nbad\\qentry\n",
            b"#V2\nbad" + backslash + b"qentry\nbad" + backslash + b"qentryX\n",
        ),
    )
    for mode, initial, expected_file in cases:
        vm_path = temporary / f"{mode}-vm.txt"
        native_path = temporary / f"{mode}-native.txt"
        vm_path.write_bytes(initial)
        native_path.write_bytes(initial)
        commands = (
            [zett, "run", str(PROGRAM.relative_to(ROOT)), mode, str(vm_path)],
            [executable, mode, str(native_path)],
        )
        results = [run_pty(command, b"HISTORY> ", b"\x1b[AX\r") for command in commands]
        if any(code != 0 or result_line(output) != "decoded" for code, output in results):
            raise AssertionError(
                f"{mode}: no se recuperó el contenido V2 correctamente:\n"
                f"--- VM ({results[0][0]}) ---\n{results[0][1].decode(errors='replace')}\n"
                f"--- nativo ({results[1][0]}) ---\n{results[1][1].decode(errors='replace')}"
            )
        if vm_path.read_bytes() != expected_file or native_path.read_bytes() != expected_file:
            raise AssertionError(
                f"{mode}: el archivo no conservó los escapes correctos; "
                f"VM={vm_path.read_bytes()!r}; nativo={native_path.read_bytes()!r}"
            )


def check_error(label: str, zett: str, executable: str, mode: str, keys: bytes, expected: bytes) -> None:
    unused = "/tmp/titan-readline-unused-history"
    command_vm = [zett, "run", str(PROGRAM.relative_to(ROOT)), mode, unused]
    command_native = [executable, mode, unused]
    vm_code, vm_output = run_pty(command_vm, b"LINE> ", keys)
    native_code, native_output = run_pty(command_native, b"LINE> ", keys)
    if vm_code == 0 or native_code == 0 or expected not in vm_output or expected not in native_output:
        raise AssertionError(
            f"{label}: error distinto o no detectado\n"
            f"--- VM ({vm_code}) ---\n{vm_output.decode(errors='replace')}\n"
            f"--- nativo ({native_code}) ---\n{native_output.decode(errors='replace')}"
        )


def run_pipe(command: list[str], input_data: bytes) -> str:
    result = subprocess.run(command, cwd=ROOT, input=input_data, capture_output=True, timeout=10, check=False)
    line = result_line(result.stdout)
    if result.returncode != 0 or line is None:
        raise AssertionError(
            f"entrada sin terminal falló (código {result.returncode}): "
            f"{result.stdout.decode(errors='replace')} {result.stderr.decode(errors='replace')}"
        )
    return line


def check_secret_requires_terminal(zett: str, executable: str, history_path: str) -> None:
    commands = (
        [zett, "run", str(PROGRAM.relative_to(ROOT)), "secret", history_path],
        [executable, "secret", history_path],
    )
    for label, command in zip(("VM", "nativo"), commands):
        result = subprocess.run(
            command,
            cwd=ROOT,
            input=b"mysecret\n",
            capture_output=True,
            timeout=10,
            check=False,
        )
        output = result.stdout + result.stderr
        if result.returncode == 0 or b"mysecret" in output:
            raise AssertionError(
                f"{label}: la entrada secreta sin terminal no se rechazó de forma segura: "
                f"{output.decode(errors='replace')}"
            )


def main() -> None:
    zett = os.environ.get("ZETT") or shutil.which("zett") or str(Path.home() / ".local/bin/zett")
    if not Path(zett).is_file() or not os.access(zett, os.X_OK):
        raise SystemExit("No encuentro el binario precompilado `zett`; instálalo antes de ejecutar esta prueba.")

    compiler = os.environ.get("COMPILER", "")
    if compiler and (not Path(compiler).is_file() or not os.access(compiler, os.X_OK)):
        raise SystemExit(f"No encuentro el compilador Titan nativo ejecutable: {compiler}")

    with tempfile.TemporaryDirectory(prefix="titan-readline-") as temporary:
        exe = str(Path(temporary) / "readline_test")
        if compiler:
            build_command = [compiler, str(PROGRAM.relative_to(ROOT)), exe]
            print("Compilador de la prueba: ejecutable nativo Titan del bootstrap.")
        else:
            build_command = [zett, "run", "selfhost/build.titan", str(PROGRAM.relative_to(ROOT)), exe]
            print("Compilador de la prueba: selfhost/build.titan ejecutado por Zett precompilado.")
        built = subprocess.run(
            build_command,
            cwd=ROOT,
            capture_output=True,
            timeout=600,
            check=False,
        )
        if built.returncode != 0 or not Path(exe).is_file():
            print("Falló la compilación nativa de la prueba:", file=sys.stderr)
            print(built.stdout.decode(errors="replace"), file=sys.stderr)
            print(built.stderr.decode(errors="replace"), file=sys.stderr)
            raise SystemExit(1)

        dummy = str(Path(temporary) / "sin-historial")
        check_pair("edición y retroceso", zett, exe, "plain", dummy, dummy, b"LINE> ", b"hellx\x7fo\r", "hello")
        check_pair("movimiento del cursor", zett, exe, "plain", dummy, dummy, b"LINE> ", b"hellx\x1b[Do\r", "hellox")
        check_pair("caracteres UTF-8", zett, exe, "plain", dummy, dummy, b"LINE> ", "caféx\x7f\r".encode("utf-8"), "café")
        check_pair("historial de una llamada", zett, exe, "with-history", dummy, dummy, b"LINE> ", b"memory\r", "memory")
        check_memory_history(zett, exe, dummy)
        check_noop_history_save(zett, exe, Path(temporary))
        check_v2_history(zett, exe, Path(temporary))

        history_vm = str(Path(temporary) / "history-vm.txt")
        history_native = str(Path(temporary) / "history-native.txt")
        Path(history_vm).write_text("first\nprevious\n", encoding="utf-8")
        Path(history_native).write_text("first\nprevious\n", encoding="utf-8")
        check_pair("recorrido del historial", zett, exe, "persistent", history_vm, history_native, b"HISTORY> ", b"\x1b[A\x1b[A\r", "first")
        if Path(history_vm).read_bytes() != Path(history_native).read_bytes():
            raise AssertionError(
                "el archivo de historial no quedó igual tras elegir una línea: "
                f"VM={Path(history_vm).read_bytes()!r}; nativo={Path(history_native).read_bytes()!r}"
            )

        Path(history_vm).write_text("previous\n", encoding="utf-8")
        Path(history_native).write_text("previous\n", encoding="utf-8")
        check_pair("guardado de una línea nueva", zett, exe, "persistent", history_vm, history_native, b"HISTORY> ", b"next\r", "next")
        if Path(history_vm).read_bytes() != Path(history_native).read_bytes():
            raise AssertionError("el archivo de historial no quedó igual después de guardar")
        check_pair("recarga del historial guardado", zett, exe, "persistent", history_vm, history_native, b"HISTORY> ", b"\x1b[A\r", "next")
        if Path(history_vm).read_bytes() != Path(history_native).read_bytes():
            raise AssertionError("el historial guardado no se pudo recargar igual")

        search_vm = str(Path(temporary) / "search-vm.txt")
        search_native = str(Path(temporary) / "search-native.txt")
        history = "alpha\nchosen phrase\nomega\n"
        Path(search_vm).write_text(history, encoding="utf-8")
        Path(search_native).write_text(history, encoding="utf-8")
        check_pair("búsqueda inversa", zett, exe, "reverse-search", search_vm, search_native, b"SEARCH> ", b"\x12chosen\r", "chosen phrase")

        secret_vm, secret_native = check_pair("entrada secreta", zett, exe, "secret", dummy, dummy, b"SECRET> ", b"mysecrex\x7ft\r", "mysecret")
        secret_unicode_vm, secret_unicode_native = check_pair(
            "entrada secreta UTF-8",
            zett,
            exe,
            "secret",
            dummy,
            dummy,
            b"SECRET> ",
            "claveñx\x7f".encode("utf-8") + b"\r",
            "claveñ",
        )
        for output in (secret_vm, secret_native, secret_unicode_vm, secret_unicode_native):
            result_at = output.find(b"RESULT:")
            if result_at < 0 or b"secret" in output[:result_at] or "claveñ".encode() in output[:result_at]:
                raise AssertionError("la entrada secreta apareció en pantalla antes de devolverse")

        check_error("Ctrl-C", zett, exe, "plain", b"\x03", b"interrupted (Ctrl-C)")
        check_error("Ctrl-D", zett, exe, "plain", b"\x04", b"end of file (Ctrl-D)")

        vm_pipe = run_pipe([zett, "run", str(PROGRAM.relative_to(ROOT)), "plain", dummy], b"pipe-line\n")
        native_pipe = run_pipe([exe, "plain", dummy], b"pipe-line\n")
        if vm_pipe != "pipe-line" or native_pipe != vm_pipe:
            raise AssertionError(f"entrada sin terminal distinta: VM={vm_pipe!r}; nativo={native_pipe!r}")
        check_secret_requires_terminal(zett, exe, dummy)

        print("std::readline: edición real de terminal, VM y Titan nativo comparados.")
        print("Probado: cursor, retroceso, Unicode, historial, búsqueda inversa, clave secreta, Ctrl-C, Ctrl-D, tubería y rechazo seguro de claves sin terminal.")


if __name__ == "__main__":
    main()
